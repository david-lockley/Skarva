use std::{
    io::{Cursor, Read},
    path::Path,
};

use quick_xml::{Reader, events::Event};
use zip::ZipArchive;

const MAX_EXTRACTED_CHARS: usize = 16_000;
const MAX_ARCHIVE_ENTRY: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Attachment {
    pub filename: String,
    pub text: Option<String>,
    pub note: Option<String>,
}

impl Attachment {
    pub fn context(&self) -> String {
        match &self.text {
            Some(text) => format!("Attachment: {}\n{}", self.filename, text),
            None => format!(
                "Attachment: {} (content unavailable: {})",
                self.filename,
                self.note.as_deref().unwrap_or("unsupported format")
            ),
        }
    }
}

pub fn extract(filename: &str, bytes: &[u8]) -> Attachment {
    let extension = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let result = match extension.as_str() {
        "txt" | "md" | "csv" | "tsv" | "json" => std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| "not UTF-8 text".to_owned()),
        "pdf" => pdf_extract::extract_text_from_mem(bytes)
            .map_err(|error| format!("PDF text extraction failed: {error}")),
        "docx" => office_xml(bytes, "word/document.xml"),
        "pptx" => presentation_text(bytes),
        "xlsx" => spreadsheet_text(bytes),
        _ => Err("unsupported format".into()),
    };
    match result {
        Ok(text) if !text.trim().is_empty() => Attachment {
            filename: filename.to_owned(),
            text: Some(text.chars().take(MAX_EXTRACTED_CHARS).collect()),
            note: None,
        },
        Ok(_) => Attachment {
            filename: filename.to_owned(),
            text: None,
            note: Some("no selectable text found (scanned images need OCR)".into()),
        },
        Err(note) => Attachment {
            filename: filename.to_owned(),
            text: None,
            note: Some(note),
        },
    }
}

pub fn load(path: &Path) -> Result<Attachment, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("Could not read file: {e}"))?
        .len();
    if size > 5 * 1024 * 1024 {
        return Err("Local file is over the 5 MB read limit".into());
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Local file has no usable name")?;
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read file: {e}"))?;
    let attachment = extract(filename, &bytes);
    if attachment.text.is_none() {
        return Err(format!(
            "{} could not be read: {}",
            filename,
            attachment.note.as_deref().unwrap_or("no text found")
        ));
    }
    Ok(attachment)
}

fn archive(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, String> {
    ZipArchive::new(Cursor::new(bytes)).map_err(|_| "invalid Office file".into())
}

fn entry(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, String> {
    let mut file = zip.by_name(name).map_err(|_| format!("missing {name}"))?;
    if file.size() > MAX_ARCHIVE_ENTRY {
        return Err("Office document text is too large".into());
    }
    let mut xml = String::new();
    file.read_to_string(&mut xml)
        .map_err(|_| "invalid Office XML".to_owned())?;
    Ok(xml)
}

fn xml_text(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    let mut result = String::new();
    loop {
        match reader.read_event().map_err(|_| "invalid Office XML")? {
            Event::Text(value) => {
                let decoded = value.decode().map_err(|_| "invalid Office text")?;
                let unescaped =
                    quick_xml::escape::unescape(&decoded).map_err(|_| "invalid Office text")?;
                result.push_str(&unescaped);
                if result.chars().count() > MAX_EXTRACTED_CHARS {
                    break;
                }
            }
            Event::End(tag)
                if matches!(tag.local_name().as_ref(), b"p" | b"row" | b"tr" | b"si") =>
            {
                result.push('\n');
            }
            Event::End(tag) if matches!(tag.local_name().as_ref(), b"tc" | b"c") => {
                result.push_str(" | ");
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(result)
}

fn office_xml(bytes: &[u8], path: &str) -> Result<String, String> {
    let mut zip = archive(bytes)?;
    xml_text(&entry(&mut zip, path)?)
}

fn presentation_text(bytes: &[u8]) -> Result<String, String> {
    let mut zip = archive(bytes)?;
    let mut slides = (0..zip.len())
        .filter_map(|index| zip.name_for_index(index).map(str::to_owned))
        .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
        .collect::<Vec<_>>();
    slides.sort();
    let mut result = String::new();
    for slide in slides.into_iter().take(30) {
        result.push_str(&format!("\n{slide}\n"));
        result.push_str(&xml_text(&entry(&mut zip, &slide)?)?);
        if result.chars().count() >= MAX_EXTRACTED_CHARS {
            break;
        }
    }
    Ok(result)
}

fn spreadsheet_text(bytes: &[u8]) -> Result<String, String> {
    let mut zip = archive(bytes)?;
    let shared = entry(&mut zip, "xl/sharedStrings.xml")
        .ok()
        .map(|xml| shared_strings(&xml))
        .transpose()?
        .unwrap_or_default();
    let mut sheets = (0..zip.len())
        .filter_map(|index| zip.name_for_index(index).map(str::to_owned))
        .filter(|name| name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml"))
        .collect::<Vec<_>>();
    sheets.sort();
    let mut result = String::new();
    for sheet in sheets.into_iter().take(10) {
        result.push_str(&format!("\n{sheet}\n"));
        result.push_str(&sheet_text(&entry(&mut zip, &sheet)?, &shared)?);
        if result.chars().count() >= MAX_EXTRACTED_CHARS {
            break;
        }
    }
    Ok(result)
}

fn shared_strings(xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event().map_err(|_| "invalid spreadsheet XML")? {
            Event::Start(tag) if tag.local_name().as_ref() == b"t" => in_text = true,
            Event::End(tag) if tag.local_name().as_ref() == b"t" => in_text = false,
            Event::End(tag) if tag.local_name().as_ref() == b"si" => {
                strings.push(std::mem::take(&mut current));
                if strings.len() > 10_000 {
                    break;
                }
            }
            Event::Text(value) if in_text => {
                let decoded = value.decode().map_err(|_| "invalid spreadsheet text")?;
                current.push_str(
                    &quick_xml::escape::unescape(&decoded)
                        .map_err(|_| "invalid spreadsheet text")?,
                );
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(strings)
}

fn sheet_text(xml: &str, shared: &[String]) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    let mut result = String::new();
    let mut shared_cell = false;
    let mut in_value = false;
    let mut in_inline = false;
    loop {
        match reader.read_event().map_err(|_| "invalid spreadsheet XML")? {
            Event::Start(tag) if tag.local_name().as_ref() == b"c" => {
                shared_cell = tag
                    .attributes()
                    .flatten()
                    .any(|attr| attr.key.as_ref() == b"t" && attr.value.as_ref() == b"s");
            }
            Event::Start(tag) if tag.local_name().as_ref() == b"v" => in_value = true,
            Event::Start(tag) if tag.local_name().as_ref() == b"t" => in_inline = true,
            Event::End(tag) if tag.local_name().as_ref() == b"v" => in_value = false,
            Event::End(tag) if tag.local_name().as_ref() == b"t" => in_inline = false,
            Event::Text(value) if in_value || in_inline => {
                let decoded = value.decode().map_err(|_| "invalid spreadsheet text")?;
                let value = quick_xml::escape::unescape(&decoded)
                    .map_err(|_| "invalid spreadsheet text")?;
                if shared_cell && in_value {
                    if let Ok(index) = value.parse::<usize>()
                        && let Some(text) = shared.get(index)
                    {
                        result.push_str(text);
                    }
                } else {
                    result.push_str(&value);
                }
            }
            Event::End(tag) if tag.local_name().as_ref() == b"c" => result.push_str(" | "),
            Event::End(tag) if tag.local_name().as_ref() == b"row" => result.push('\n'),
            Event::Eof => break,
            _ => {}
        }
        if result.chars().count() > MAX_EXTRACTED_CHARS {
            break;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_utf8_text() {
        let file = extract("notes.txt", b"hello world");
        assert_eq!(file.text.as_deref(), Some("hello world"));
    }

    #[test]
    fn reports_unsupported_file() {
        let file = extract("photo.png", b"xyz");
        assert!(file.text.is_none());
        assert_eq!(file.note.as_deref(), Some("unsupported format"));
    }
}
