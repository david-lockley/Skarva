use std::path::PathBuf;
use std::sync::OnceLock;

pub const NAME: &str = "Skarva";
pub const TAGLINE: &str = "An agentic whiteboard";
pub const APP_ID: &str = "dev.skarva.app";
static DATA_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

pub fn set_data_directory(path: PathBuf) {
    let _ = DATA_DIRECTORY.set(path);
}

pub fn data_directory() -> Option<PathBuf> {
    if let Some(path) = DATA_DIRECTORY.get() {
        return Some(path.clone());
    }
    // Keep existing installs on their original folder. Moving the vault or
    // project-relative attachments during a rename would break saved workflows.
    let previous = directories::ProjectDirs::from("dev", "JEV Studio", "JEV Studio")?;
    if previous.data_local_dir().is_dir() {
        return Some(previous.data_local_dir().to_owned());
    }
    directories::ProjectDirs::from("dev", "Skarva", "Skarva")
        .map(|dirs| dirs.data_local_dir().to_owned())
}
