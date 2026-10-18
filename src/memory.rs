use std::{collections::BTreeMap, fs, path::Path};

const MAX_RECORDS: usize = 1_000;
const MAX_VALUE: usize = 12_000;

fn records(path: &Path) -> Result<BTreeMap<String, String>, String> {
    match fs::read(path) {
        Ok(data) => {
            serde_json::from_slice(&data).map_err(|e| format!("Could not read saved memory: {e}"))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(format!("Could not open saved memory: {error}")),
    }
}

pub fn recall(path: &Path, key: &str) -> Result<Option<String>, String> {
    Ok(records(path)?.get(key).cloned())
}

pub fn remember(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let mut data = records(path)?;
    if !data.contains_key(key) && data.len() >= MAX_RECORDS {
        return Err("Project memory is full (1,000 records)".into());
    }
    data.insert(key.to_owned(), value.chars().take(MAX_VALUE).collect());
    let parent = path.parent().ok_or("Invalid memory location")?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create memory directory: {e}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Could not save memory: {e}"))?;
    fs::rename(temporary, path).map_err(|e| format!("Could not finish saving memory: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_recalls_value() {
        let path =
            std::env::temp_dir().join(format!("skarva-memory-test-{}.json", std::process::id()));
        assert_eq!(recall(&path, "unknown").unwrap(), None);
        remember(&path, "task", "first").unwrap();
        remember(&path, "task", "second").unwrap();
        assert_eq!(recall(&path, "task").unwrap().as_deref(), Some("second"));
        let _ = fs::remove_file(path);
    }
}
