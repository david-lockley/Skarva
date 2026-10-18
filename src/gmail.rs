use std::{
    collections::HashSet,
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use base64::{
    Engine as _,
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde_json::{Value, json};
use url::Url;
use zeroize::Zeroizing;

use crate::{attachments, credentials, office};

const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const CLIENT_ID: &str = "gmail-client-id";
const CLIENT_SECRET: &str = "gmail-client-secret";
const REFRESH_TOKEN: &str = "gmail-refresh-token";

#[derive(Clone)]
pub struct Mail {
    pub id: String,
    pub thread_id: String,
    pub sender: String,
    pub recipient: String,
    pub subject: String,
    pub message_id: String,
    pub body: String,
    pub attachments: Vec<attachments::Attachment>,
}

impl Mail {
    pub fn message_context(&self) -> String {
        format!(
            "From: {}\nSubject: {}\n\n{}",
            self.sender, self.subject, self.body
        )
    }
}

pub fn connected() -> Result<bool, String> {
    Ok(credentials::load_named(REFRESH_TOKEN)?
        .map(Zeroizing::new)
        .is_some())
}

pub fn disconnect() -> Result<(), String> {
    credentials::remove_named(REFRESH_TOKEN)
}

pub fn save_client(id: &str, secret: &str) -> Result<(), String> {
    if !id.trim().ends_with(".apps.googleusercontent.com") {
        return Err("Enter a Desktop app OAuth client ID from Google Cloud".into());
    }
    if secret.trim().is_empty() {
        let (saved_id, saved_secret) = load_client()?;
        if saved_id.as_deref() == Some(id.trim()) && saved_secret.is_some() {
            return Ok(());
        }
        return Err("Enter the Desktop OAuth client secret from Google Cloud".into());
    }
    credentials::save_named(CLIENT_ID, id)?;
    credentials::save_named(CLIENT_SECRET, secret)
}

pub fn load_client() -> Result<(Option<String>, Option<String>), String> {
    Ok((
        credentials::load_named(CLIENT_ID)?,
        credentials::load_named(CLIENT_SECRET)?,
    ))
}

fn random_url_string() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "Could not generate OAuth random value")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn form(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().copied())
        .finish()
}

pub fn connect() -> Result<(), String> {
    let (id, secret) = load_client()?;
    let id = id.ok_or("Save a Google Desktop OAuth client first")?;
    let secret = secret.ok_or("Save a Google Desktop OAuth client secret first")?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Could not open local OAuth callback: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let redirect = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().map_err(|e| e.to_string())?.port()
    );
    let verifier = random_url_string()?;
    let challenge =
        URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()).as_ref());
    let state = random_url_string()?;
    let scopes =
        "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.send";
    let authorization = format!(
        "{AUTH}?{}",
        form(&[
            ("client_id", &id),
            ("redirect_uri", &redirect),
            ("response_type", "code"),
            ("scope", scopes),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("state", &state),
        ])
    );
    webbrowser::open(&authorization).map_err(|e| format!("Could not open browser: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if Instant::now() >= deadline {
            return Err("Gmail sign-in timed out. Try Connect Gmail again".into());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
                let mut buffer = [0_u8; 8192];
                let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
                let request = String::from_utf8_lossy(&buffer[..n]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .ok_or("Invalid OAuth callback")?;
                let callback =
                    Url::parse(&format!("{redirect}{path}")).map_err(|e| e.to_string())?;
                let params = callback
                    .query_pairs()
                    .collect::<std::collections::HashMap<_, _>>();
                if params.get("state").map(|value| value.as_ref()) != Some(state.as_str()) {
                    let _ =
                        stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
                    continue;
                }
                let code = params.get("code").map(|value| value.as_ref());
                let Some(code) = code else {
                    let page = format!(
                        "Gmail connection was cancelled. You can return to {}.",
                        crate::brand::NAME
                    );
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        page.len(),
                        page
                    );
                    let _ = stream.write_all(response.as_bytes());
                    return Err("Gmail authorization was cancelled".into());
                };
                let body = form(&[
                    ("code", code),
                    ("client_id", &id),
                    ("client_secret", &secret),
                    ("redirect_uri", &redirect),
                    ("grant_type", "authorization_code"),
                    ("code_verifier", &verifier),
                ]);
                let mut response = agent()
                    .post(TOKEN)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .send(body.as_bytes())
                    .map_err(|e| format!("Could not finish Gmail sign-in: {e}"))?;
                let value: Value = response.body_mut().read_json().map_err(|e| e.to_string())?;
                let refresh = value["refresh_token"]
                    .as_str()
                    .ok_or("Google returned no refresh token")?;
                credentials::save_named(REFRESH_TOKEN, refresh)?;
                let page = format!("Gmail connected. You can return to {}.", crate::brand::NAME);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    page.len(),
                    page
                );
                let _ = stream.write_all(response.as_bytes());
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100))
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into()
}

fn access_token() -> Result<Zeroizing<String>, String> {
    let (id, secret) = load_client()?;
    let id = id.ok_or("Gmail OAuth client missing. Open Integrations")?;
    let secret = secret.ok_or("Gmail OAuth client secret missing. Open Integrations")?;
    let refresh = Zeroizing::new(
        credentials::load_named(REFRESH_TOKEN)?
            .ok_or("Gmail is disconnected. Open Integrations")?,
    );
    let body = form(&[
        ("client_id", &id),
        ("client_secret", &secret),
        ("refresh_token", refresh.as_str()),
        ("grant_type", "refresh_token"),
    ]);
    let mut response = agent()
        .post(TOKEN)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body.as_bytes())
        .map_err(|e| format!("Gmail token refresh failed: {e}. Reconnect Gmail if needed"))?;
    let value: Value = response.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(Zeroizing::new(
        value["access_token"]
            .as_str()
            .ok_or("Google returned no access token")?
            .to_owned(),
    ))
}

fn get_json(path: &str, token: &str) -> Result<Value, String> {
    let mut response = agent()
        .get(format!("{API}{path}"))
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| format!("Gmail request failed: {e}"))?;
    response.body_mut().read_json().map_err(|e| e.to_string())
}

fn header(value: &Value, name: &str) -> String {
    value["payload"]["headers"]
        .as_array()
        .and_then(|headers| {
            headers.iter().find_map(|entry| {
                if entry["name"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
                {
                    entry["value"].as_str()
                } else {
                    None
                }
            })
        })
        .unwrap_or("")
        .to_owned()
}

fn text_body(payload: &Value) -> Option<String> {
    if payload["mimeType"].as_str() == Some("text/plain")
        && !payload["filename"]
            .as_str()
            .is_some_and(|name| !name.is_empty())
        && let Some(data) = payload["body"]["data"].as_str()
    {
        let bytes = URL_SAFE_NO_PAD
            .decode(data)
            .or_else(|_| URL_SAFE.decode(data))
            .ok()?;
        return String::from_utf8(bytes).ok();
    }
    payload["parts"].as_array()?.iter().find_map(text_body)
}

fn attachment_parts<'a>(payload: &'a Value, found: &mut Vec<&'a Value>) {
    if payload["filename"]
        .as_str()
        .is_some_and(|name| !name.is_empty())
    {
        found.push(payload);
    }
    if let Some(parts) = payload["parts"].as_array() {
        for part in parts {
            attachment_parts(part, found);
        }
    }
}

fn read_attachments(
    message_id: &str,
    payload: &Value,
    token: &str,
) -> Vec<attachments::Attachment> {
    let mut parts = Vec::new();
    attachment_parts(payload, &mut parts);
    parts
        .into_iter()
        .take(5)
        .map(|part| {
            let filename = part["filename"].as_str().unwrap_or("attachment");
            let size = part["body"]["size"].as_u64().unwrap_or(0);
            if size > 5 * 1024 * 1024 {
                return attachments::Attachment {
                    filename: filename.into(),
                    text: None,
                    note: Some("over the 5 MB read limit".into()),
                };
            }
            let result = (|| {
                let data = if let Some(id) = part["body"]["attachmentId"].as_str() {
                    let value =
                        get_json(&format!("/messages/{message_id}/attachments/{id}"), token)?;
                    value["data"]
                        .as_str()
                        .ok_or("Gmail returned no attachment data")?
                        .to_owned()
                } else {
                    part["body"]["data"]
                        .as_str()
                        .ok_or("Gmail returned no attachment data")?
                        .to_owned()
                };
                URL_SAFE_NO_PAD
                    .decode(&data)
                    .or_else(|_| URL_SAFE.decode(&data))
                    .map_err(|_| "Invalid Gmail attachment encoding".to_owned())
            })();
            match result {
                Ok(bytes) => attachments::extract(filename, &bytes),
                Err(note) => attachments::Attachment {
                    filename: filename.into(),
                    text: None,
                    note: Some(note),
                },
            }
        })
        .collect()
}

pub fn next_unread(seen: &HashSet<String>) -> Result<Option<Mail>, String> {
    let token = access_token()?;
    let listing = get_json(
        "/messages?labelIds=INBOX&labelIds=UNREAD&maxResults=10&q=newer_than%3A1d",
        &token,
    )?;
    let Some(id) = listing["messages"].as_array().and_then(|messages| {
        messages
            .iter()
            .filter_map(|message| message["id"].as_str())
            .find(|id| !seen.contains(*id))
    }) else {
        return Ok(None);
    };
    let value = get_json(&format!("/messages/{id}?format=full"), &token)?;
    let sender = header(&value, "From");
    let reply_to = header(&value, "Reply-To");
    let recipient = reply_address(if reply_to.is_empty() {
        &sender
    } else {
        &reply_to
    })?;
    let subject = header(&value, "Subject");
    let body = text_body(&value["payload"])
        .or_else(|| value["snippet"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let attachments = read_attachments(id, &value["payload"], &token);
    Ok(Some(Mail {
        id: id.into(),
        thread_id: value["threadId"].as_str().unwrap_or_default().into(),
        sender,
        recipient,
        subject,
        message_id: header(&value, "Message-ID"),
        body: body.chars().take(12000).collect(),
        attachments,
    }))
}

fn reply_address(from: &str) -> Result<String, String> {
    let address = if let Some((_, tail)) = from.rsplit_once('<') {
        tail.trim_end_matches('>').trim()
    } else {
        from.trim()
    };
    if address.matches('@').count() != 1
        || address
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '<' | '>' | ',' | ';' | '\r' | '\n'))
    {
        return Err("Incoming email has an invalid reply address".into());
    }
    Ok(address.to_owned())
}

fn reply_message(
    mail: &Mail,
    draft: &str,
    attachment: Option<&office::Artifact>,
) -> Result<String, String> {
    let to = &mail.recipient;
    if draft.trim().is_empty() {
        return Err("Reply draft is empty".into());
    }
    let subject = if mail.subject.to_lowercase().starts_with("re:") {
        mail.subject.clone()
    } else {
        format!("Re: {}", mail.subject)
    };
    let subject = subject.replace(['\r', '\n'], " ");
    let subject = if subject.is_ascii() {
        subject
    } else {
        format!(
            "=?UTF-8?B?{}?=",
            base64::engine::general_purpose::STANDARD.encode(subject.as_bytes())
        )
    };
    let message_id = mail.message_id.replace(['\r', '\n'], "");
    let mut raw = format!("To: {to}\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\n");
    if !message_id.is_empty() {
        raw.push_str(&format!(
            "In-Reply-To: {message_id}\r\nReferences: {message_id}\r\n"
        ));
    }
    if let Some(file) = attachment {
        if file.bytes.len() > 15 * 1024 * 1024 {
            return Err("Generated attachment is too large to send".into());
        }
        let boundary = "skarva-attachment-boundary";
        raw.push_str(&format!(
            "Content-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\n"
        ));
        raw.push_str(&format!("--{boundary}\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n"));
        for chunk in base64::engine::general_purpose::STANDARD
            .encode(draft.trim().as_bytes())
            .as_bytes()
            .chunks(76)
        {
            raw.push_str(std::str::from_utf8(chunk).map_err(|e| e.to_string())?);
            raw.push_str("\r\n");
        }
        raw.push_str(&format!("--{boundary}\r\nContent-Type: {}; name=\"{}\"\r\nContent-Disposition: attachment; filename=\"{}\"\r\nContent-Transfer-Encoding: base64\r\n\r\n", file.mime, file.filename, file.filename));
        for chunk in base64::engine::general_purpose::STANDARD
            .encode(&file.bytes)
            .as_bytes()
            .chunks(76)
        {
            raw.push_str(std::str::from_utf8(chunk).map_err(|e| e.to_string())?);
            raw.push_str("\r\n");
        }
        raw.push_str(&format!("--{boundary}--\r\n"));
    } else {
        raw.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
        raw.push_str(draft.trim());
    }
    Ok(raw)
}

pub fn send_reply(
    mail: &Mail,
    draft: &str,
    attachment: Option<&office::Artifact>,
) -> Result<String, String> {
    let raw = reply_message(mail, draft, attachment)?;
    let token = access_token()?;
    let mut response = agent()
        .post(format!("{API}/messages/send"))
        .header("Authorization", &format!("Bearer {}", token.as_str()))
        .send_json(
            json!({"raw": URL_SAFE_NO_PAD.encode(raw.as_bytes()), "threadId": mail.thread_id}),
        )
        .map_err(|e| format!("Gmail send failed: {e}"))?;
    let value: Value = response.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(value["id"].as_str().unwrap_or("sent").to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reply_address() {
        assert_eq!(
            reply_address("Person <person@example.com>").unwrap(),
            "person@example.com"
        );
        assert!(reply_address("bad\nBcc: other@example.com").is_err());
    }

    #[test]
    fn reply_includes_attachment_mime_part() {
        let mail = Mail {
            id: "1".into(),
            thread_id: "2".into(),
            sender: "a@example.com".into(),
            recipient: "a@example.com".into(),
            subject: "Guide".into(),
            message_id: "<m@example.com>".into(),
            body: String::new(),
            attachments: Vec::new(),
        };
        let attachment = office::Artifact {
            title: "Guide".into(),
            filename: "guide.docx".into(),
            mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            summary: String::new(),
            bytes: b"test".to_vec(),
            path: std::path::PathBuf::new(),
        };
        let raw = reply_message(&mail, "Here is the guide", Some(&attachment)).unwrap();
        assert!(raw.contains("multipart/mixed"));
        assert!(raw.contains("filename=\"guide.docx\""));
        assert!(raw.contains("dGVzdA=="));
        assert!(raw.contains("SGVyZSBpcyB0aGUgZ3VpZGU="));
    }

    #[test]
    fn finds_inline_attachment_parts() {
        let payload = json!({
            "mimeType": "multipart/mixed",
            "parts": [{
                "filename": "brief.txt",
                "body": {"size": 5, "data": URL_SAFE_NO_PAD.encode(b"hello")}
            }]
        });
        let files = read_attachments("message-id", &payload, "unused-token");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].text.as_deref(), Some("hello"));
    }
}
