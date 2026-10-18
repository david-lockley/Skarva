use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{llm, model::NodeKind};

#[derive(Clone)]
pub struct Artifact {
    pub title: String,
    pub filename: String,
    pub mime: &'static str,
    pub summary: String,
    pub bytes: Vec<u8>,
    pub path: PathBuf,
}

#[derive(Deserialize)]
struct WordSpec {
    title: String,
    sections: Vec<WordSection>,
}

#[derive(Deserialize)]
struct WordSection {
    heading: String,
    paragraphs: Vec<String>,
}

#[derive(Deserialize)]
struct SheetSpec {
    title: String,
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct SlidesSpec {
    title: String,
    slides: Vec<SlideSpec>,
}

#[derive(Deserialize)]
struct SlideSpec {
    title: String,
    bullets: Vec<String>,
}

pub fn generate(
    kind: NodeKind,
    provider: llm::Provider,
    model: &str,
    instructions: &str,
    email: &str,
    directory: &Path,
) -> Result<Artifact, String> {
    let format_instructions = match kind {
        NodeKind::Word => {
            "Create a practical, editable Word guide. Return only JSON: {\"title\":\"...\",\"sections\":[{\"heading\":\"...\",\"paragraphs\":[\"...\"]}]}. Write a useful introduction and 2-6 sections with specific, accurate guidance. No Markdown."
        }
        NodeKind::Excel => {
            "Create a useful editable spreadsheet. Return only JSON: {\"title\":\"...\",\"columns\":[\"...\"],\"rows\":[[\"...\",123]]}. Use 2-8 clear columns and up to 60 rows. Use JSON numbers for numeric cells; no formulas or Markdown."
        }
        NodeKind::PowerPoint => {
            "Create a concise editable slide deck. Return only JSON: {\"title\":\"...\",\"slides\":[{\"title\":\"...\",\"bullets\":[\"...\"]}]}. Use 2-8 slides and at most 5 bullets per slide. Keep slide titles under 100 characters and bullets under 120 characters. No Markdown."
        }
        _ => return Err("This is not an Office block".into()),
    };
    let system = format!(
        "{format_instructions} Follow these block instructions: {}. Base the result on the incoming request. Avoid unsupported facts and invented data.",
        if instructions.trim().is_empty() {
            "Create the requested file"
        } else {
            instructions.trim()
        }
    );
    let answer = llm::complete(
        provider,
        model,
        &system,
        &format!("Incoming email:\n{email}"),
    )?;
    let (title, summary, bytes, extension, mime) = match render_document(kind, &answer) {
        Ok(document) => document,
        Err(first_error) => {
            let retry_system = format!(
                "{system} Your previous attempt could not be used: {first_error}. Regenerate the complete document from scratch as valid JSON. Escape newlines and special characters inside JSON strings. Follow the requested field names and types exactly."
            );
            let retry = llm::complete(
                provider,
                model,
                &retry_system,
                &format!("Incoming email:\n{email}"),
            )?;
            render_document(kind, &retry).map_err(|error| {
                format!("The model returned unusable document data after one retry: {error}")
            })?
        }
    };
    fs::create_dir_all(directory)
        .map_err(|e| format!("Could not create generated files directory: {e}"))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let base = safe_filename(&title);
    let filename = format!("{base}-{now}.{extension}");
    let path = directory.join(&filename);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("Could not save generated file: {e}"))?;
    file.write_all(&bytes)
        .map_err(|e| format!("Could not save generated file: {e}"))?;
    Ok(Artifact {
        title,
        filename,
        mime,
        summary: summary.chars().take(12_000).collect(),
        bytes,
        path,
    })
}

type RenderedDocument = (String, String, Vec<u8>, &'static str, &'static str);

fn render_document(kind: NodeKind, answer: &str) -> Result<RenderedDocument, String> {
    let json = extract_json(&answer)?;
    Ok(match kind {
        NodeKind::Word => {
            let spec: WordSpec = serde_json::from_str(json)
                .map_err(|e| format!("Word content was not valid JSON: {e}"))?;
            validate_word(&spec)?;
            let summary = format!(
                "{}\n{}",
                spec.title,
                spec.sections
                    .iter()
                    .map(|s| format!("{}\n{}", s.heading, s.paragraphs.join("\n")))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            );
            let bytes = make_docx(&spec)?;
            (
                spec.title,
                summary,
                bytes,
                "docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            )
        }
        NodeKind::Excel => {
            let spec: SheetSpec = serde_json::from_str(json)
                .map_err(|e| format!("Excel content was not valid JSON: {e}"))?;
            validate_sheet(&spec)?;
            let summary = format!(
                "{}\n{}\n{}",
                spec.title,
                spec.columns.join(" | "),
                spec.rows
                    .iter()
                    .take(30)
                    .map(|row| row.iter().map(cell_summary).collect::<Vec<_>>().join(" | "))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            let bytes = make_xlsx(&spec)?;
            (
                spec.title,
                summary,
                bytes,
                "xlsx",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            )
        }
        NodeKind::PowerPoint => {
            let spec: SlidesSpec = serde_json::from_str(json)
                .map_err(|e| format!("PowerPoint content was not valid JSON: {e}"))?;
            validate_slides(&spec)?;
            let summary = format!(
                "{}\n{}",
                spec.title,
                spec.slides
                    .iter()
                    .map(|slide| format!("{}\n{}", slide.title, slide.bullets.join("\n")))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            );
            let bytes = make_pptx(&spec)?;
            (
                spec.title,
                summary,
                bytes,
                "pptx",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            )
        }
        _ => unreachable!(),
    })
}

fn extract_json(answer: &str) -> Result<&str, String> {
    let start = answer
        .find('{')
        .ok_or("The model returned no JSON object")?;
    let end = answer
        .rfind('}')
        .ok_or("The model returned incomplete JSON")?;
    if end <= start {
        return Err("The model returned incomplete JSON".into());
    }
    Ok(&answer[start..=end])
}

fn validate_text(value: &str, label: &str, max: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > max {
        return Err(format!("Generated {label} is empty or too long"));
    }
    Ok(())
}

fn validate_word(spec: &WordSpec) -> Result<(), String> {
    validate_text(&spec.title, "title", 180)?;
    if spec.sections.is_empty() || spec.sections.len() > 10 {
        return Err("Word guide needs 1-10 sections".into());
    }
    for section in &spec.sections {
        validate_text(&section.heading, "section heading", 180)?;
        if section.paragraphs.is_empty() {
            return Err("Word section needs at least one paragraph".into());
        }
        for paragraph in &section.paragraphs {
            validate_text(paragraph, "paragraph", 2500)?;
        }
    }
    if spec
        .sections
        .iter()
        .flat_map(|section| &section.paragraphs)
        .map(String::len)
        .sum::<usize>()
        > 100_000
    {
        return Err("Word content is too long".into());
    }
    Ok(())
}

fn validate_sheet(spec: &SheetSpec) -> Result<(), String> {
    validate_text(&spec.title, "title", 180)?;
    if !(2..=8).contains(&spec.columns.len()) || spec.rows.is_empty() || spec.rows.len() > 100 {
        return Err("Spreadsheet needs 2-8 columns and 1-100 rows".into());
    }
    for column in &spec.columns {
        validate_text(column, "column heading", 100)?;
    }
    for row in &spec.rows {
        if row.len() != spec.columns.len() {
            return Err("Spreadsheet row has the wrong number of cells".into());
        }
        for value in row {
            if !value.is_string() && !value.is_number() && !value.is_boolean() && !value.is_null() {
                return Err("Spreadsheet cells must be text, numbers, booleans, or blank".into());
            }
            if value.to_string().len() > 1000 {
                return Err("Spreadsheet cell is too long".into());
            }
        }
    }
    Ok(())
}

fn validate_slides(spec: &SlidesSpec) -> Result<(), String> {
    validate_text(&spec.title, "title", 180)?;
    if spec.slides.is_empty() || spec.slides.len() > 12 {
        return Err("Presentation needs 1-12 slides".into());
    }
    for slide in &spec.slides {
        validate_text(&slide.title, "slide title", 100)?;
        if slide.bullets.len() > 6 {
            return Err("Slide has too many bullets".into());
        }
        for bullet in &slide.bullets {
            validate_text(bullet, "slide bullet", 120)?;
        }
    }
    Ok(())
}

fn cell_summary(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn safe_filename(title: &str) -> String {
    let slug = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "document".into()
    } else {
        slug
    }
}

fn xml(value: &str) -> String {
    value
        .chars()
        .filter(|&c| {
            matches!(c, '\t' | '\n' | '\r')
                || matches!(c as u32, 0x20..=0x7E | 0xA0..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn package(parts: Vec<(String, String)>) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in parts {
        writer
            .start_file(name, options)
            .map_err(|e| e.to_string())?;
        writer
            .write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    Ok(writer.finish().map_err(|e| e.to_string())?.into_inner())
}

fn make_docx(spec: &WordSpec) -> Result<Vec<u8>, String> {
    let mut body = String::new();
    body.push_str(&word_paragraph(&spec.title, "Title"));
    for section in &spec.sections {
        body.push_str(&word_paragraph(&section.heading, "Heading1"));
        for paragraph in &section.paragraphs {
            body.push_str(&word_paragraph(paragraph, "Normal"));
        }
    }
    body.push_str("<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1080\" w:right=\"1170\" w:bottom=\"1080\" w:left=\"1170\"/></w:sectPr>");
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>"
    );
    let styles = r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:pPr><w:spacing w:after="180" w:line="310" w:lineRule="auto"/></w:pPr><w:rPr><w:rFonts w:ascii="Aptos" w:hAnsi="Aptos"/><w:sz w:val="22"/><w:color w:val="202427"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="360"/></w:pPr><w:rPr><w:b/><w:sz w:val="42"/><w:color w:val="161A1C"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="300" w:after="140"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/><w:color w:val="202427"/></w:rPr></w:style></w:styles>"#;
    package(vec![
        ("[Content_Types].xml".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#.into()),
        ("_rels/.rels".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.into()),
        ("word/_rels/document.xml.rels".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#.into()),
        ("word/document.xml".into(), document),
        ("word/styles.xml".into(), styles.into()),
    ])
}

fn word_paragraph(value: &str, style: &str) -> String {
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml(value)
    )
}

fn make_xlsx(spec: &SheetSpec) -> Result<Vec<u8>, String> {
    let mut cols = String::new();
    for (index, heading) in spec.columns.iter().enumerate() {
        let width = heading.chars().count().max(18).min(38);
        cols.push_str(&format!(
            "<col min=\"{}\" max=\"{}\" width=\"{}\" customWidth=\"1\"/>",
            index + 1,
            index + 1,
            width + 6
        ));
    }
    let mut rows = format!(
        "<row r=\"1\" ht=\"34\" customHeight=\"1\"><c r=\"A1\" s=\"2\" t=\"inlineStr\"><is><t>{}</t></is></c></row>",
        xml(&spec.title)
    );
    rows.push_str("<row r=\"2\"/>");
    rows.push_str("<row r=\"3\" ht=\"26\" customHeight=\"1\">");
    for (index, heading) in spec.columns.iter().enumerate() {
        rows.push_str(&format!(
            "<c r=\"{}3\" s=\"1\" t=\"inlineStr\"><is><t>{}</t></is></c>",
            column_name(index),
            xml(heading)
        ));
    }
    rows.push_str("</row>");
    for (row_index, row) in spec.rows.iter().enumerate() {
        let number = row_index + 4;
        rows.push_str(&format!("<row r=\"{number}\">"));
        for (column_index, cell) in row.iter().enumerate() {
            let address = format!("{}{number}", column_name(column_index));
            match cell {
                serde_json::Value::Number(number) => rows.push_str(&format!("<c r=\"{address}\" s=\"4\"><v>{number}</v></c>")),
                serde_json::Value::Bool(value) => rows.push_str(&format!("<c r=\"{address}\" s=\"4\" t=\"b\"><v>{}</v></c>", if *value { 1 } else { 0 })),
                serde_json::Value::Null => {},
                other => rows.push_str(&format!("<c r=\"{address}\" s=\"3\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", xml(other.as_str().unwrap_or("")))),
            }
        }
        rows.push_str("</row>");
    }
    let last_col = column_name(spec.columns.len() - 1);
    let last_row = spec.rows.len() + 3;
    let worksheet = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"3\" topLeftCell=\"A4\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight=\"20\"/><cols>{cols}</cols><sheetData>{rows}</sheetData><autoFilter ref=\"A3:{last_col}{last_row}\"/></worksheet>"
    );
    let styles = r#"<?xml version="1.0" encoding="UTF-8"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="3"><font><sz val="11"/><name val="Aptos"/></font><font><b/><sz val="11"/><color rgb="FFFFFFFF"/><name val="Aptos"/></font><font><b/><sz val="18"/><color rgb="FF202427"/><name val="Aptos"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FF27343B"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="5"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFill="1"/><xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0" applyAlignment="1"><alignment horizontal="left" indent="1"/></xf><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0" applyAlignment="1"><alignment horizontal="right" indent="1"/></xf></cellXfs></styleSheet>"#;
    package(vec![
        ("[Content_Types].xml".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#.into()),
        ("_rels/.rels".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.into()),
        ("xl/workbook.xml".into(), r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#.into()),
        ("xl/_rels/workbook.xml.rels".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#.into()),
        ("xl/worksheets/sheet1.xml".into(), worksheet),
        ("xl/styles.xml".into(), styles.into()),
    ])
}

fn column_name(mut index: usize) -> String {
    let mut result = String::new();
    loop {
        result.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    result
}

fn make_pptx(spec: &SlidesSpec) -> Result<Vec<u8>, String> {
    let mut types = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/><Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/><Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>",
    );
    let mut ids = String::new();
    let mut presentation_rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"slideMasters/slideMaster1.xml\"/>",
    );
    let mut parts = Vec::new();
    for (index, slide) in spec.slides.iter().enumerate() {
        let number = index + 1;
        types.push_str(&format!("<Override PartName=\"/ppt/slides/slide{number}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"));
        ids.push_str(&format!(
            "<p:sldId id=\"{}\" r:id=\"rId{}\"/>",
            256 + index,
            number + 1
        ));
        presentation_rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{number}.xml\"/>", number + 1));
        parts.push((
            format!("ppt/slides/slide{number}.xml"),
            ppt_slide(slide, number),
        ));
        parts.push((format!("ppt/slides/_rels/slide{number}.xml.rels"), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#.into()));
    }
    types.push_str("</Types>");
    presentation_rels.push_str("</Relationships>");
    let presentation = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx=\"12192000\" cy=\"6858000\" type=\"screen16x9\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>"
    );
    parts.extend([
        ("[Content_Types].xml".into(), types),
        ("_rels/.rels".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#.into()),
        ("ppt/presentation.xml".into(), presentation),
        ("ppt/_rels/presentation.xml.rels".into(), presentation_rels),
        ("ppt/slideMasters/slideMaster1.xml".into(), format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldMaster xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:cSld><p:spTree>{}</p:spTree></p:cSld><p:clrMap accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" bg1=\"lt1\" bg2=\"lt2\" folHlink=\"folHlink\" hlink=\"hlink\" tx1=\"dk1\" tx2=\"dk2\"/><p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst></p:sldMaster>", ppt_group())),
        ("ppt/slideMasters/_rels/slideMaster1.xml.rels".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#.into()),
        ("ppt/slideLayouts/slideLayout1.xml".into(), format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldLayout xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" type=\"blank\" preserve=\"1\"><p:cSld><p:spTree>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>", ppt_group())),
        ("ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#.into()),
        ("ppt/theme/theme1.xml".into(), ppt_theme()),
    ]);
    package(parts)
}

fn ppt_group() -> &'static str {
    r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#
}

fn ppt_slide(slide: &SlideSpec, number: usize) -> String {
    let mut shapes = ppt_text_shape(
        2,
        &slide.title,
        640000,
        480000,
        10900000,
        900000,
        3200,
        true,
    );
    for (index, bullet) in slide.bullets.iter().enumerate() {
        shapes.push_str(&ppt_text_shape(
            index + 3,
            &format!("• {bullet}"),
            800000,
            1550000 + index as i64 * 790000,
            10500000,
            650000,
            2200,
            false,
        ));
    }
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"FAFAF8\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree>{}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>", ppt_group()).replace("name=\"\"", &format!("name=\"Slide {number}\""))
}

fn ppt_text_shape(
    id: usize,
    value: &str,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    size: usize,
    bold: bool,
) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"Text {id}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{width}\" cy=\"{height}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap=\"square\"/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" sz=\"{size}\" b=\"{}\"><a:solidFill><a:srgbClr val=\"202427\"/></a:solidFill><a:latin typeface=\"Aptos\"/></a:rPr><a:t>{}</a:t></a:r><a:endParaRPr lang=\"en-US\"/></a:p></p:txBody></p:sp>",
        if bold { 1 } else { 0 },
        xml(value)
    )
}

fn ppt_theme() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Skarva"><a:themeElements><a:clrScheme name="Neutral"><a:dk1><a:srgbClr val="202427"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="27343B"/></a:dk2><a:lt2><a:srgbClr val="FAFAF8"/></a:lt2><a:accent1><a:srgbClr val="327A5B"/></a:accent1><a:accent2><a:srgbClr val="557A9A"/></a:accent2><a:accent3><a:srgbClr val="B68558"/></a:accent3><a:accent4><a:srgbClr val="8C75A8"/></a:accent4><a:accent5><a:srgbClr val="789B85"/></a:accent5><a:accent6><a:srgbClr val="C87867"/></a:accent6><a:hlink><a:srgbClr val="327A5B"/></a:hlink><a:folHlink><a:srgbClr val="557A9A"/></a:folHlink></a:clrScheme><a:fontScheme name="Aptos"><a:majorFont><a:latin typeface="Aptos Display"/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/></a:minorFont></a:fontScheme><a:fmtScheme name="Simple"><a:fillStyleLst><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="lt1"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn generates_editable_office_packages() {
        let word = make_docx(&WordSpec {
            title: "Health & wellbeing".into(),
            sections: vec![WordSection {
                heading: "Daily habits".into(),
                paragraphs: vec!["Sleep, move, and eat well.".into()],
            }],
        })
        .unwrap();
        let sheet = make_xlsx(&SheetSpec {
            title: "Habit tracker".into(),
            columns: vec!["Habit".into(), "Days".into()],
            rows: vec![vec!["Sleep".into(), 7.into()]],
        })
        .unwrap();
        let slides = make_pptx(&SlidesSpec {
            title: "Healthy routines".into(),
            slides: vec![SlideSpec {
                title: "Sleep".into(),
                bullets: vec!["Keep a regular schedule".into()],
            }],
        })
        .unwrap();
        for (bytes, required) in [
            (&word, "word/document.xml"),
            (&sheet, "xl/worksheets/sheet1.xml"),
            (&slides, "ppt/slides/slide1.xml"),
        ] {
            let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut text = String::new();
            archive
                .by_name(required)
                .unwrap()
                .read_to_string(&mut text)
                .unwrap();
            assert!(text.contains("Health") || text.contains("Sleep") || text.contains("Habit"));
        }
        let word_text = crate::attachments::extract("guide.docx", &word)
            .text
            .unwrap();
        let sheet_text = crate::attachments::extract("tracker.xlsx", &sheet)
            .text
            .unwrap();
        let slide_text = crate::attachments::extract("slides.pptx", &slides)
            .text
            .unwrap();
        assert!(word_text.contains("Sleep, move, and eat well."));
        assert!(sheet_text.contains("Habit") && sheet_text.contains("Sleep"));
        assert!(slide_text.contains("Keep a regular schedule"));
    }

    #[test]
    fn xml_escapes_user_content() {
        assert_eq!(xml("A & <B>"), "A &amp; &lt;B&gt;");
        assert_eq!(xml("A\u{FFFE}B\u{0}C\u{85}D"), "ABCD");
        assert_eq!(safe_filename("../../Secret report"), "secret-report");
    }

    #[test]
    fn word_accepts_more_than_eight_paragraphs() {
        let paragraphs = (0..9).map(|index| format!("Paragraph {index}")).collect();
        let spec = WordSpec {
            title: "Workshop briefing".into(),
            sections: vec![WordSection {
                heading: "Agenda".into(),
                paragraphs,
            }],
        };
        validate_word(&spec).unwrap();
        let bytes = make_docx(&spec).unwrap();
        let text = crate::attachments::extract("briefing.docx", &bytes)
            .text
            .unwrap();
        assert!(text.contains("Paragraph 8"));
    }

    #[test]
    #[ignore = "writes sample files for visual Office compatibility checks"]
    fn write_sample_files() {
        let directory = std::env::temp_dir().join("skarva-office-qa");
        fs::create_dir_all(&directory).unwrap();
        let word = WordSpec {
            title: "Healthy living guide".into(),
            sections: vec![
                WordSection {
                    heading: "Everyday habits".into(),
                    paragraphs: vec![
                        "Keep a consistent sleep schedule and make time for regular activity."
                            .into(),
                    ],
                },
                WordSection {
                    heading: "Food and hydration".into(),
                    paragraphs: vec![
                        "Choose a variety of foods and drink water throughout the day.".into(),
                    ],
                },
            ],
        };
        let sheet = SheetSpec {
            title: "Weekly habit tracker".into(),
            columns: vec!["Habit".into(), "Target days".into(), "Notes".into()],
            rows: vec![
                vec![
                    "Sleep routine".into(),
                    7.into(),
                    "Set a regular bedtime".into(),
                ],
                vec![
                    "Movement".into(),
                    5.into(),
                    "Choose an activity you enjoy".into(),
                ],
            ],
        };
        let slides = SlidesSpec {
            title: "Healthy routines".into(),
            slides: vec![
                SlideSpec {
                    title: "Sleep".into(),
                    bullets: vec![
                        "Keep a consistent schedule".into(),
                        "Make the room comfortable".into(),
                    ],
                },
                SlideSpec {
                    title: "Movement".into(),
                    bullets: vec!["Start with a manageable routine".into()],
                },
            ],
        };
        fs::write(directory.join("sample.docx"), make_docx(&word).unwrap()).unwrap();
        fs::write(directory.join("sample.xlsx"), make_xlsx(&sheet).unwrap()).unwrap();
        fs::write(directory.join("sample.pptx"), make_pptx(&slides).unwrap()).unwrap();
        println!("{}", directory.display());
    }
}
