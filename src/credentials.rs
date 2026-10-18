use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use keyring::{Entry, Error};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const MASTER_SERVICE: &str = "dev.skarva.app.vault";
const MASTER_ACCOUNT: &str = "master-key-v1";
const JEV_KEY: &str = "typesafe-api-key";
const AAD: &[u8] = b"Skarva credential vault v2";
// Version-one vaults retain their original keychain item and authentication
// bytes. Renaming either would strand saved connections or add unlock prompts.
const LEGACY_MASTER_SERVICE: &str = "dev.jevstudio.app.vault";
const LEGACY_AAD: &[u8] = b"JEV Studio credential vault v1";
const MAX_VAULT_BYTES: u64 = 1_048_576;

// Cache success AND failure: a rejected OS prompt must never be retried for each key.
static VAULT: OnceLock<Result<Mutex<Vault>, String>> = OnceLock::new();

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    nonce: String,
    ciphertext: String,
}

struct Vault {
    path: PathBuf,
    key: Zeroizing<Vec<u8>>,
    values: BTreeMap<String, Zeroizing<String>>,
    version: u32,
}

impl Vault {
    fn open() -> Result<Self, String> {
        let directory = crate::brand::data_directory()
            .ok_or("Could not locate the application data folder")?
            .join("credentials");
        let path = directory.join("vault.json");
        let version = Self::read_envelope(&path)?.map_or(2, |envelope| envelope.version);
        let service = if version == 1 {
            LEGACY_MASTER_SERVICE
        } else {
            MASTER_SERVICE
        };
        let entry = Entry::new(service, MASTER_ACCOUNT).map_err(|e| e.to_string())?;
        // Read the master key once per launch. A new, empty vault creates its key once.
        let key = match entry.get_password() {
            Ok(encoded) => STANDARD.decode(Zeroizing::new(encoded).as_bytes())
                .map(Zeroizing::new).map_err(|_| "The saved vault key is invalid".to_owned())?,
            Err(Error::NoEntry) if !path.exists() => {
                let mut key = Zeroizing::new(vec![0; 32]);
                SystemRandom::new().fill(&mut key).map_err(|_| "Could not create a secure vault key")?;
                let encoded = Zeroizing::new(STANDARD.encode(key.as_slice()));
                entry.set_password(&encoded).map_err(|e| format!("Could not protect the vault key: {e}"))?;
                key
            }
            Err(Error::NoEntry) => return Err("The credential vault exists but its OS key is missing. Restore the vault key before reconnecting services.".into()),
            Err(error) => return Err(format!("Could not unlock saved connections: {error}. Reopen the app to try again.")),
        };
        Self::with_key(path, key)
    }

    fn read_envelope(path: &Path) -> Result<Option<Envelope>, String> {
        match fs::metadata(path) {
            Ok(metadata) => {
                if metadata.len() > MAX_VAULT_BYTES {
                    return Err("The credential vault is too large".into());
                }
                let envelope: Envelope =
                    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
                        .map_err(|_| "Could not read the credential vault")?;
                if !matches!(envelope.version, 1 | 2) {
                    return Err("This credential vault version is not supported".into());
                }
                Ok(Some(envelope))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Could not open saved connections: {error}")),
        }
    }

    fn associated_data(version: u32) -> &'static [u8] {
        if version == 1 { LEGACY_AAD } else { AAD }
    }

    fn with_key(path: PathBuf, key: Zeroizing<Vec<u8>>) -> Result<Self, String> {
        if key.len() != 32 {
            return Err("The saved vault key is invalid".into());
        }
        let envelope = Self::read_envelope(&path)?;
        let version = envelope.as_ref().map_or(2, |envelope| envelope.version);
        let values = match envelope {
            Some(envelope) => {
                let nonce: [u8; 12] = STANDARD
                    .decode(envelope.nonce)
                    .map_err(|_| "Invalid vault nonce")?
                    .try_into()
                    .map_err(|_| "Invalid vault nonce")?;
                let mut ciphertext = Zeroizing::new(
                    STANDARD
                        .decode(envelope.ciphertext)
                        .map_err(|_| "Invalid vault data")?,
                );
                let cipher = aead::LessSafeKey::new(
                    aead::UnboundKey::new(&aead::AES_256_GCM, &key)
                        .map_err(|_| "Invalid vault key")?,
                );
                let plaintext = cipher
                    .open_in_place(
                        aead::Nonce::assume_unique_for_key(nonce),
                        aead::Aad::from(Self::associated_data(version)),
                        &mut ciphertext,
                    )
                    .map_err(
                        |_| "Could not decrypt saved connections; the vault may have been altered",
                    )?;
                let values: BTreeMap<String, String> =
                    serde_json::from_slice(plaintext).map_err(|_| "Invalid credential data")?;
                values
                    .into_iter()
                    .map(|(name, value)| (name, Zeroizing::new(value)))
                    .collect()
            }
            None => BTreeMap::new(),
        };
        Ok(Self {
            path,
            key,
            values,
            version,
        })
    }

    fn persist(&self, values: &BTreeMap<&str, &str>) -> Result<(), String> {
        let mut plaintext = Zeroizing::new(serde_json::to_vec(values).map_err(|e| e.to_string())?);
        let mut nonce = [0u8; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| "Could not encrypt saved connections")?;
        let cipher = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, &self.key)
                .map_err(|_| "Invalid vault key")?,
        );
        cipher
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(Self::associated_data(self.version)),
                &mut *plaintext,
            )
            .map_err(|_| "Could not encrypt saved connections")?;
        let data = serde_json::to_vec(&Envelope {
            version: self.version,
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(&*plaintext),
        })
        .map_err(|e| e.to_string())?;
        let parent = self.path.parent().ok_or("Invalid vault location")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let temporary = parent.join(format!(
            "vault-{}.tmp",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
            file.write_all(&data).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn save(&mut self, name: &str, value: &str) -> Result<(), String> {
        let mut values: BTreeMap<&str, &str> = self
            .values
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        values.insert(name, value);
        self.persist(&values)?;
        self.values
            .insert(name.into(), Zeroizing::new(value.into()));
        Ok(())
    }

    fn remove(&mut self, name: &str) -> Result<(), String> {
        if !self.values.contains_key(name) {
            return Ok(());
        }
        let values = self
            .values
            .iter()
            .filter(|(k, _)| k.as_str() != name)
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        self.persist(&values)?;
        self.values.remove(name);
        Ok(())
    }
}

fn with_vault<T>(operation: impl FnOnce(&mut Vault) -> Result<T, String>) -> Result<T, String> {
    let vault = VAULT.get_or_init(|| Vault::open().map(Mutex::new));
    match vault {
        Ok(vault) => {
            let mut guard = vault
                .lock()
                .map_err(|_| "The credential vault is unavailable; reopen the app")?;
            operation(&mut guard)
        }
        Err(error) => Err(error.clone()),
    }
}

pub fn load_named(name: &str) -> Result<Option<String>, String> {
    with_vault(|vault| Ok(vault.values.get(name).map(|v| v.to_string())))
}

pub fn save_named(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("Enter a value first".into());
    }
    if value.len() > 65_536 {
        return Err("This credential is too long".into());
    }
    with_vault(|vault| vault.save(name, value.trim()))
}

pub fn remove_named(name: &str) -> Result<(), String> {
    with_vault(|vault| vault.remove(name))
}

pub fn load() -> Result<Option<String>, String> {
    load_named(JEV_KEY)
}
pub fn save(key: &str) -> Result<(), String> {
    save_named(JEV_KEY, key)
}
pub fn remove() -> Result<(), String> {
    remove_named(JEV_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(label: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("skarva-vault-{label}-{}", std::process::id()))
            .join("vault.json")
    }

    #[test]
    fn vault_encrypts_credentials_and_roundtrips_removal() {
        let path = location("roundtrip");
        let key = Zeroizing::new(vec![7; 32]);
        let mut vault = Vault::with_key(path.clone(), key.clone()).unwrap();
        vault.save("test", "secret-api-token").unwrap();
        assert!(
            !fs::read_to_string(&path)
                .unwrap()
                .contains("secret-api-token")
        );
        let mut reopened = Vault::with_key(path.clone(), key.clone()).unwrap();
        assert_eq!(reopened.values["test"].as_str(), "secret-api-token");
        reopened.remove("test").unwrap();
        assert!(
            Vault::with_key(path.clone(), key)
                .unwrap()
                .values
                .is_empty()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn vault_rejects_wrong_key_and_altered_data() {
        let path = location("integrity");
        let mut vault = Vault::with_key(path.clone(), Zeroizing::new(vec![9; 32])).unwrap();
        vault.save("token", "example").unwrap();
        assert!(Vault::with_key(path.clone(), Zeroizing::new(vec![8; 32])).is_err());
        let mut envelope: Envelope = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let mut ciphertext = STANDARD.decode(&envelope.ciphertext).unwrap();
        ciphertext[0] ^= 1;
        envelope.ciphertext = STANDARD.encode(ciphertext);
        fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
        assert!(Vault::with_key(path.clone(), Zeroizing::new(vec![9; 32])).is_err());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn rebrand_preserves_legacy_vaults_and_authenticates_the_format_version() {
        for version in [1, 2] {
            let path = location(&format!("format-{version}"));
            let key = Zeroizing::new(vec![11; 32]);
            let mut vault = Vault::with_key(path.clone(), key.clone()).unwrap();
            assert_eq!(vault.version, 2);
            vault.version = version;
            vault.save("connection", "existing-secret").unwrap();
            let mut reopened = Vault::with_key(path.clone(), key.clone()).unwrap();
            assert_eq!(reopened.version, version);
            assert_eq!(reopened.values["connection"].as_str(), "existing-secret");
            reopened.save("second", "new-secret").unwrap();
            let mut envelope = Vault::read_envelope(&path).unwrap().unwrap();
            assert_eq!(envelope.version, version);
            envelope.version = if version == 1 { 2 } else { 1 };
            fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
            assert!(Vault::with_key(path.clone(), key).is_err());
            let _ = fs::remove_dir_all(path.parent().unwrap());
        }
    }
}
