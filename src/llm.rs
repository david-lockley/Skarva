use std::time::Duration;

use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::credentials;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Provider {
    #[default]
    OpenRouter,
    OpenAI,
    Anthropic,
}

impl Provider {
    pub const ALL: [Self; 3] = [Self::OpenRouter, Self::OpenAI, Self::Anthropic];

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenRouter => "OpenRouter",
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
        }
    }

    pub fn key_name(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter-api-key",
            Self::OpenAI => "openai-api-key",
            Self::Anthropic => "anthropic-api-key",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter/free",
            Self::OpenAI => "gpt-4.1-mini",
            Self::Anthropic => "claude-sonnet-4-5",
        }
    }

    pub fn from_label(label: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|provider| provider.label() == label)
            .unwrap_or_default()
    }
}

pub fn current_provider() -> Result<Provider, String> {
    Ok(Provider::from_label(
        &credentials::load_named("llm-provider")?.unwrap_or_default(),
    ))
}

pub fn current_model(provider: Provider) -> Result<String, String> {
    Ok(credentials::load_named("llm-model")?.unwrap_or_else(|| provider.default_model().into()))
}

pub fn generate(
    provider: Provider,
    model: &str,
    instructions: &str,
    email: &str,
) -> Result<String, String> {
    complete(
        provider,
        model,
        instructions,
        &format!("Incoming email:\n{email}\n\nWrite only the email reply body."),
    )
}

pub fn complete(
    provider: Provider,
    model: &str,
    instructions: &str,
    content: &str,
) -> Result<String, String> {
    let key = Zeroizing::new(
        credentials::load_named(provider.key_name())?
            .ok_or_else(|| format!("Add an {} API key in Integrations", provider.label()))?,
    );
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(75)))
        .build()
        .into();
    let response = match provider {
        Provider::OpenRouter | Provider::OpenAI => {
            let endpoint = if provider == Provider::OpenRouter {
                "https://openrouter.ai/api/v1/chat/completions"
            } else {
                "https://api.openai.com/v1/chat/completions"
            };
            agent
                .post(endpoint)
                .header("Authorization", &format!("Bearer {}", key.as_str()))
                .send_json(json!({"model": model, "messages": [
                    {"role":"system", "content": instructions},
                    {"role":"user", "content": content}
                ]}))
        }
        Provider::Anthropic => agent
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", key.as_str())
            .header("anthropic-version", "2023-06-01")
            .send_json(json!({"model": model, "max_tokens": 4096,
                "system": instructions,
                "messages": [{"role":"user", "content": content}]})),
    };
    let mut response =
        response.map_err(|error| format!("{} request failed: {error}", provider.label()))?;
    let value: Value = response
        .body_mut()
        .read_json()
        .map_err(|error| error.to_string())?;
    let result = match provider {
        Provider::Anthropic => value["content"]
            .as_array()
            .and_then(|parts| parts.iter().find_map(|part| part["text"].as_str())),
        _ => value["choices"][0]["message"]["content"].as_str(),
    };
    let result = result.ok_or("The model returned no text")?.trim();
    if result.is_empty() {
        return Err("The model returned an empty draft".into());
    }
    Ok(result.to_owned())
}
