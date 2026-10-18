use std::{fs, path::PathBuf};

use crate::model::Project;

pub struct Storage {
    pub directory: PathBuf,
}

impl Storage {
    pub fn import(&self, path: &std::path::Path, id: u64) -> Result<Project, String> {
        if fs::metadata(path).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
            return Err("Project files must be smaller than 4 MB".into());
        }
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let mut project: Project = serde_json::from_slice(&bytes)
            .map_err(|_| "This is not a valid Skarva project file")?;
        project.validate()?;
        project.remove_memory_blocks();
        project.id = id;
        self.save(&project)?;
        Ok(project)
    }

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
                    if !backup.exists() && fs::copy(&path, backup).is_err() {
                        return Some(original);
                    }
                    if self.save(&project).is_err() {
                        return Some(original);
                    }
                }
                Some(project)
            })
            .collect::<Vec<_>>();
        projects.sort_by_key(|a| a.name.to_lowercase());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Node, NodeKind};

    #[test]
    fn imported_projects_get_a_fresh_id_and_invalid_files_do_not_replace_saved_work() {
        let directory = std::env::temp_dir().join(format!("skarva-import-{}", std::process::id()));
        let storage = Storage {
            directory: directory.join("projects"),
        };
        fs::create_dir_all(&directory).unwrap();
        let input = directory.join("input.json");
        fs::write(
            &input,
            serde_json::to_vec(&Project::text_workflow(1, "Shared writing")).unwrap(),
        )
        .unwrap();
        let imported = storage.import(&input, 20).unwrap();
        assert_eq!(imported.id, 20);
        assert_eq!(storage.list().len(), 1);
        fs::write(
            &input,
            br#"{"version":999,"id":1,"name":"Bad","canvases":[],"next_id":2}"#,
        )
        .unwrap();
        assert!(storage.import(&input, 20).is_err());
        assert_eq!(storage.list()[0].name, "Shared writing");
        fs::remove_dir_all(directory).unwrap();
    }

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
