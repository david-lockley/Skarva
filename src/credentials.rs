use keyring::{Entry, Error};

const SERVICE: &str = "dev.jevstudio.app.typesafe";
const USER: &str = "api-key";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, USER).map_err(|error| error.to_string())
}

pub fn load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub fn save(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("Enter a TypeSafe API key first".into());
    }
    entry()?
        .set_password(key.trim())
        .map_err(|error| error.to_string())
}

pub fn remove() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
