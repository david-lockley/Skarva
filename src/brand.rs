use std::path::PathBuf;

pub const NAME: &str = "Skarva";
pub const TAGLINE: &str = "An agentic whiteboard";
pub const APP_ID: &str = "dev.skarva.app";

pub fn data_directory() -> Option<PathBuf> {
    // Keep existing installs on their original folder. Moving the vault or
    // project-relative attachments during a rename would break saved workflows.
    let previous = directories::ProjectDirs::from("dev", "JEV Studio", "JEV Studio")?;
    if previous.data_local_dir().is_dir() {
        return Some(previous.data_local_dir().to_owned());
    }
    directories::ProjectDirs::from("dev", "Skarva", "Skarva")
        .map(|dirs| dirs.data_local_dir().to_owned())
}
