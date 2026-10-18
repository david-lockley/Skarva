use std::time::Duration;

use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::{credentials, llm};

const KEY_NAME: &str = "tavily-search-api-key";

pub fn key_present() -> Result<bool, String> {
    Ok(credentials::load_named(KEY_NAME)?
        .map(Zeroizing::new)
        .is_some())
}

pub fn save_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("Enter a Tavily API key".into());
    }
    credentials::save_named(KEY_NAME, key.trim())
}

pub fn remove_key() -> Result<(), String> {
    credentials::remove_named(KEY_NAME)
}

pub fn research(
    provider: llm::Provider,
    model: &str,
    instructions: &str,
    context: &str,
) -> Result<String, String> {
    let key = Zeroizing::new(
        credentials::load_named(KEY_NAME)?
            .ok_or("Add a Tavily API key in Integrations to use Web research")?,
    );
    let query = llm::complete(
        provider,
        model,
        "Create one concise web search query. Return only the query, without quotes, URLs, or commentary. Do not include personal names, email addresses, or confidential details.",
        &format!(
            "Task: {instructions}\nIncoming request: {}",
            context.chars().take(3000).collect::<String>()
        ),
    )?;
    let query = query.lines().next().unwrap_or("").trim().trim_matches('"');
    let query = query
        .split_whitespace()
        .filter(|word| !word.contains('@'))
        .take(35)
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        return Err("AI produced an empty web search query".into());
    }
    let query: String = query.chars().take(300).collect();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
    let mut response = agent
        .post("https://api.tavily.com/search")
        .header("Authorization", &format!("Bearer {}", key.as_str()))
        .send_json(json!({
            "query": query,
            "search_depth": "basic",
            "max_results": 5,
            "chunks_per_source": 1,
            "include_answer": false,
            "include_raw_content": false
        }))
        .map_err(|e| format!("Tavily Search request failed: {e}"))?;
    let value: Value = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("Could not read search results: {e}"))?;
    format_results(&query, &value)
}

fn format_results(query: &str, value: &Value) -> Result<String, String> {
    let mut sources = Vec::new();
    if let Some(results) = value["results"].as_array() {
        for result in results.iter().take(5) {
            let title = result["title"].as_str().unwrap_or("Untitled");
            let link = result["url"].as_str().unwrap_or("");
            let snippet = result["content"].as_str().unwrap_or("");
            if link.starts_with("https://") {
                sources.push(format!(
                    "- {title}\n  URL: {link}\n  Extract: {}",
                    snippet.chars().take(1000).collect::<String>()
                ));
            }
        }
    }
    if sources.is_empty() {
        return Err("Search returned no usable sources".into());
    }
    Ok(format!(
        "Web research query: {query}\nUntrusted source extracts (verify before relying on them):\n{}",
        sources.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::format_results;
    use serde_json::json;

    #[test]
    fn formats_tavily_sources_for_downstream_blocks() {
        let response = json!({"results": [
            {"title": "Guide", "url": "https://example.com/guide", "content": "Helpful facts"},
            {"title": "Bad link", "url": "javascript:alert(1)", "content": "Ignore me"}
        ]});
        let output = format_results("example query", &response).unwrap();
        assert!(output.contains("Web research query: example query"));
        assert!(output.contains("URL: https://example.com/guide"));
        assert!(output.contains("Extract: Helpful facts"));
        assert!(!output.contains("javascript:"));
    }
}
