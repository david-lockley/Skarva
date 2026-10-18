use std::{fs, path::PathBuf};

use crate::model::Project;

pub struct Storage {
    pub directory: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Node, NodeKind};

    #[test]
    fn migration_keeps_original_project_backup() {
        let directory =
            std::env::temp_dir().join(format!("skarva-storage-migration-{}", std::process::id()));
        let storage = Storage {
            directory: directory.clone(),
        };
        let mut project = Project::new(10, "Saved workflow");
        project.canvases[0]
            .nodes
            .push(Node::new(2, NodeKind::Recall, 0.0, 0.0));
        project.next_id = 3;
        storage.save(&project).unwrap();
        let loaded = storage.list();
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].canvases[0].nodes.is_empty());
        let backup: Project = serde_json::from_slice(
            &fs::read(directory.join("10.json.before-automatic-memory")).unwrap(),
        )
        .unwrap();
        assert_eq!(backup.canvases[0].nodes.len(), 1);
        let _ = fs::remove_dir_all(directory);
    }
}

impl Storage {
    pub fn new() -> Self {
        let directory = crate::brand::data_directory()
            .map(|directory| directory.join("projects"))
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
                let contents = fs::read_to_string(&path).ok()?;
                let mut project = serde_json::from_str::<Project>(&contents).ok()?;
                let original = project.clone();
                if project.remove_memory_blocks() {
                    let backup = path.with_extension("json.before-automatic-memory");
                    if !backup.exists() {
                        if fs::copy(&path, backup).is_err() {
                            return Some(original);
                        }
                    }
                    if self.save(&project).is_err() {
                        return Some(original);
                    }
                }
                Some(project)
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
