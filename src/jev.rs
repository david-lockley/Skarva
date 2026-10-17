use std::{collections::BTreeMap, thread, time::Duration};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::credentials;

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";

#[derive(Clone, Debug)]
pub struct OptionDescription {
    pub id: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct RouteRequest {
    pub test_message: String,
    pub current_block: String,
    pub path: Vec<String>,
    pub options: Vec<OptionDescription>,
}

#[derive(Clone, Debug)]
pub struct RouteAnswer {
    pub choice: String,
    pub confidence: f32,
    pub probability: f32,
}

#[derive(Serialize)]
struct ApiRequest<'a> {
    state: serde_json::Value,
    model: &'a str,
    questions: BTreeMap<&'a str, ChoiceQuestion>,
}

#[derive(Serialize)]
struct ChoiceQuestion {
    #[serde(rename = "type")]
    kind: &'static str,
    instructions: &'static str,
    criteria: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct ApiResponse {
    answers: BTreeMap<String, ChoiceAnswer>,
}

#[derive(Deserialize)]
struct ChoiceAnswer {
    #[serde(rename = "type")]
    kind: String,
    choice: String,
    confidence: f32,
    probabilities: BTreeMap<String, f32>,
}

fn build_request(request: &RouteRequest) -> Result<ApiRequest<'static>, String> {
    if !(2..=255).contains(&request.options.len()) {
        return Err("Jev routing needs between 2 and 255 connected choices".into());
    }
    let criteria = request
        .options
        .iter()
        .map(|option| (option.id.clone(), option.description.clone()))
        .collect::<BTreeMap<_, _>>();
    if criteria.len() != request.options.len() {
        return Err("Connected choices must have unique IDs".into());
    }
    Ok(ApiRequest {
        state: serde_json::json!({
            "test_message": request.test_message,
            "current_block": request.current_block,
            "visited_path": request.path,
        }),
        model: "jev-latest",
        questions: BTreeMap::from([(
            "next_path",
            ChoiceQuestion {
                kind: "choice",
                instructions: "Given the test message and current block, which connected path best handles the request? Choose exactly one of the available paths.",
                criteria,
            },
        )]),
    })
}

fn parse_answer(
    response: ApiResponse,
    options: &[OptionDescription],
) -> Result<RouteAnswer, String> {
    let answer = response
        .answers
        .get("next_path")
        .ok_or("Jev returned no routing answer")?;
    if answer.kind != "choice" || !options.iter().any(|option| option.id == answer.choice) {
        return Err("Jev returned an unexpected path".into());
    }
    let probability = *answer
        .probabilities
        .get(&answer.choice)
        .ok_or("Jev returned no probability for its choice")?;
    if !(0.0..=1.0).contains(&answer.confidence) || !(0.0..=1.0).contains(&probability) {
        return Err("Jev returned invalid confidence or probability".into());
    }
    Ok(RouteAnswer {
        choice: answer.choice.clone(),
        confidence: answer.confidence,
        probability,
    })
}

pub fn route(request: RouteRequest) -> Result<RouteAnswer, String> {
    let body = build_request(&request)?;
    let key = Zeroizing::new(
        credentials::load()?.ok_or("No TypeSafe API key saved. Open Jev settings to add one")?,
    );
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    for attempt in 0..3 {
        let result = agent
            .post(ENDPOINT)
            .header("Authorization", &format!("Bearer {}", key.as_str()))
            .header("Content-Type", "application/json")
            .send_json(&body);
        match result {
            Ok(mut response) => {
                let parsed: ApiResponse = response
                    .body_mut()
                    .read_json()
                    .map_err(|_| "Could not read Jev's response".to_owned())?;
                return parse_answer(parsed, &request.options);
            }
            Err(ureq::Error::StatusCode(429 | 529)) if attempt < 2 => {
                thread::sleep(Duration::from_millis(300 * (1 << attempt)));
            }
            Err(ureq::Error::StatusCode(401)) => {
                return Err("TypeSafe rejected the API key. Check Jev settings".into());
            }
            Err(ureq::Error::StatusCode(status)) => {
                return Err(format!("Jev request failed (HTTP {status})"));
            }
            Err(error) => return Err(format!("Could not reach Jev: {error}")),
        }
    }
    Err("Jev is rate-limited or overloaded. Try again later".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RouteRequest {
        RouteRequest {
            test_message: "Please prepare a budget spreadsheet".into(),
            current_block: "Teams".into(),
            path: vec!["Start".into(), "Teams".into()],
            options: vec![
                OptionDescription {
                    id: "edge_1".into(),
                    description: "Excel spreadsheet".into(),
                },
                OptionDescription {
                    id: "edge_2".into(),
                    description: "Word document".into(),
                },
            ],
        }
    }

    #[test]
    fn creates_official_choice_shape() {
        let value = serde_json::to_value(build_request(&request()).unwrap()).unwrap();
        assert_eq!(value["model"], "jev-latest");
        assert_eq!(value["questions"]["next_path"]["type"], "choice");
        assert_eq!(
            value["questions"]["next_path"]["criteria"]["edge_1"],
            "Excel spreadsheet"
        );
    }

    #[test]
    fn rejects_unknown_choice() {
        let response: ApiResponse = serde_json::from_value(serde_json::json!({
            "answers": {"next_path": {"type":"choice", "choice":"bad", "confidence":0.9,
                "probabilities":{"bad":0.95,"edge_1":0.05}}}
        }))
        .unwrap();
        assert!(parse_answer(response, &request().options).is_err());
    }
}
