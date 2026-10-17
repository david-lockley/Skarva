use std::{fs, path::PathBuf};

use crate::model::Project;

pub struct Storage {
    pub directory: PathBuf,
}

impl Storage {
    pub fn new() -> Self {
        let directory = directories::ProjectDirs::from("dev", "JEV Studio", "JEV Studio")
            .map(|dirs| dirs.data_local_dir().join("projects"))
            .unwrap_or_else(|| PathBuf::from("projects"));
        Self { directory }
    }

    pub fn list(&self) -> Vec<Project> {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        let mut projects = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    return None;
                }
                let contents = fs::read_to_string(path).ok()?;
                serde_json::from_str::<Project>(&contents).ok()
            })
            .collect::<Vec<_>>();
        projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        projects
    }

    pub fn save(&self, project: &Project) -> Result<(), String> {
        fs::create_dir_all(&self.directory).map_err(|e| e.to_string())?;
        let path = self.directory.join(format!("{}.json", project.id));
        let temp = self.directory.join(format!("{}.json.tmp", project.id));
        let bytes = serde_json::to_vec_pretty(project).map_err(|e| e.to_string())?;
        fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        fs::rename(temp, path).map_err(|e| e.to_string())
    }

    pub fn delete(&self, id: u64) -> Result<(), String> {
        let path = self.directory.join(format!("{id}.json"));
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}
