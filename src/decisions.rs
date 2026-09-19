//! TypeSafe public HTTP contract. Policy enforcement never depends on these scores.
use crate::{
    brand,
    domain::{Metrics, Usage},
    session::hash,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    Choice {
        instructions: String,
        criteria: BTreeMap<String, Option<String>>,
    },
    Noul {
        instructions: String,
    },
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Noul {
        noul: f64,
    },
    Score {
        score: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legend: Option<BTreeMap<String, String>>,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionResponse {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    pub usage: DecisionUsage,
}
#[derive(Clone, Debug, Serialize)]
pub struct DecisionRequest {
    pub model: String,
    pub state: Value,
    pub questions: BTreeMap<String, Question>,
}

pub fn validate(request: &DecisionRequest, response: &DecisionResponse) -> Result<()> {
    validate_contract(request, response, true)
}
fn validate_contract(
    request: &DecisionRequest,
    response: &DecisionResponse,
    require_legend: bool,
) -> Result<()> {
    ensure!(
        response.model == request.model,
        "resolved Jev model changed: expected {}, received {}; inspect and update the pin explicitly",
        request.model,
        response.model
    );
    ensure!(
        request.questions.keys().eq(response.answers.keys()),
        "answer IDs do not match question IDs"
    );
    for (id, q) in &request.questions {
        match (q, &response.answers[id]) {
            (Question::Noul { .. }, Answer::Noul { noul }) => unit(*noul)?,
            (
                Question::Choice { criteria, .. },
                Answer::Choice {
                    choice,
                    probabilities,
                    confidence,
                },
            ) => {
                ensure!(criteria.len() >= 2, "choice needs at least two options");
                ensure!(
                    criteria.keys().eq(probabilities.keys()),
                    "choice distribution keys differ"
                );
                distribution(probabilities)?;
                unit(*confidence)?;
                let selected = *probabilities
                    .get(choice)
                    .context("unknown selected choice")?;
                ensure!(
                    probabilities.values().all(|p| *p <= selected + 1e-6),
                    "selected choice is not maximum probability"
                );
            }
            (
                Question::Score { criteria, .. },
                Answer::Score {
                    score,
                    legend,
                    probabilities,
                    confidence,
                },
            ) => {
                ensure!(
                    criteria.len() >= 2,
                    "score needs at least two rubric levels"
                );
                let expected: BTreeMap<_, _> = criteria
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (i.to_string(), s.clone()))
                    .collect();
                ensure!(
                    (!require_legend || legend.is_some())
                        && legend.as_ref().is_none_or(|l| *l == expected)
                        && expected.keys().eq(probabilities.keys()),
                    "score legend/distribution mismatch"
                );
                distribution(probabilities)?;
                unit(*confidence)?;
                let weighted: f64 = criteria
                    .iter()
                    .enumerate()
                    .map(|(i, _)| i as f64 * probabilities[&i.to_string()])
                    .sum();
                ensure!(
                    score.is_finite()
                        && *score >= 0.0
                        && *score <= (criteria.len() - 1) as f64
                        && (*score - weighted).abs() <= 0.01,
                    "score outside rubric or inconsistent distribution"
                );
            }
            _ => bail!("answer type does not match question"),
        }
    }
    ensure!(
        response
            .usage
            .cost
            .is_none_or(|cost| cost.is_finite() && cost >= 0.0),
        "invalid reported cost"
    );
    Ok(())
}
fn unit(n: f64) -> Result<()> {
    ensure!(
        n.is_finite() && (0.0..=1.0).contains(&n),
        "nonfinite or out-of-range value"
    );
    Ok(())
}
fn distribution(p: &BTreeMap<String, f64>) -> Result<()> {
    for n in p.values() {
        unit(*n)?;
    }
    ensure!(
        (p.values().sum::<f64>() - 1.0).abs() < 0.002,
        "probabilities must sum to one"
    );
    Ok(())
}

/// Conservative byte-count estimates, not an official tokenizer.
pub fn check_budget(r: &DecisionRequest, total: usize, state_longest: usize) -> Result<()> {
    let state = serde_json::to_vec(&r.state)?.len();
    let longest = r
        .questions
        .values()
        .map(serde_json::to_vec)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    ensure!(!r.questions.is_empty(), "no questions");
    ensure!(
        serde_json::to_vec(r)?.len() + 1024 <= total,
        "Jev combined request estimate exceeds token budget"
    );
    ensure!(
        state + longest + 1024 <= state_longest,
        "Jev state plus longest question estimate exceeds token budget"
    );
    Ok(())
}

pub fn retryable(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504 | 529)
}
pub fn retry_delay(attempt: u32, retry_after: Option<&str>) -> Duration {
    let exponential =
        Duration::from_millis(250 * 2u64.pow(attempt.min(5)) + rand::random_range(0..=150));
    let hinted = retry_after
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .or_else(|| {
            retry_after
                .and_then(|s| httpdate::parse_http_date(s).ok())
                .and_then(|t| t.duration_since(std::time::SystemTime::now()).ok())
        });
    hinted.map_or(exponential, |d| d.max(exponential))
}

pub struct Jev {
    client: reqwest::Client,
    key: String,
    endpoint: String,
    pub model: String,
    pub total: usize,
    pub state_limit: usize,
    pub max_requests: u64,
    cache: BTreeMap<String, DecisionResponse>,
    openrouter_resolved: Option<String>,
}
pub const OPENROUTER_MODEL: &str = "typesafe/jev-1.13";
pub const OPENROUTER_RESOLVED: &str = "typesafe/jev-1.13-20260917";
impl Jev {
    pub fn from_config(config: &crate::domain::RunConfig) -> Result<Self> {
        match config.jev_provider.as_str() {
            "typesafe" => Self::from_env(
                &config.jev_model,
                config.jev_request_limit,
                config.jev_state_limit,
            ),
            "openrouter" => Self::new_openrouter(
                crate::credentials::get("OPENROUTER_API_KEY").context(
                    "OPENROUTER_API_KEY missing; OpenRouter and TypeSafe keys are separate",
                )?,
                &config.jev_model,
                config
                    .jev_resolved_model
                    .as_deref()
                    .unwrap_or(OPENROUTER_RESOLVED),
                "https://openrouter.ai/api/alpha/decisions",
                config.jev_request_limit,
                config.jev_state_limit,
            ),
            _ => bail!("unsupported Jev provider"),
        }
    }
    pub fn from_env(model: &str, total: usize, state_limit: usize) -> Result<Self> {
        let key = crate::credentials::get("TYPESAFE_API_KEY").context("TYPESAFE_API_KEY missing; for OpenRouter use OPENROUTER_API_KEY and --jev-provider openrouter")?;
        ensure!(
            !key.starts_with("sk-or-"),
            "OpenRouter keys do not authenticate TypeSafe directly; use OPENROUTER_API_KEY and --jev-provider openrouter"
        );
        Self::new(
            key,
            model,
            "https://api.typesafe.ai/v1/systemone",
            total,
            state_limit,
        )
    }
    pub fn new_openrouter(
        key: String,
        model: &str,
        resolved: &str,
        endpoint: &str,
        total: usize,
        state_limit: usize,
    ) -> Result<Self> {
        ensure!(
            model == OPENROUTER_MODEL,
            "supported OpenRouter decision model is typesafe/jev-1.13; aliases are not reproducible"
        );
        let date = resolved.strip_prefix("typesafe/jev-1.13-").unwrap_or("");
        ensure!(
            date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()),
            "pin the exact dated OpenRouter serving model with --jev-resolved-model"
        );
        let mut adapter = Self::new(
            key,
            "jev-1.13.0",
            endpoint,
            total.min(32_000),
            state_limit.min(32_000),
        )?;
        adapter.model = model.into();
        adapter.openrouter_resolved = Some(resolved.into());
        Ok(adapter)
    }
    pub fn new(
        key: String,
        model: &str,
        endpoint: &str,
        total: usize,
        state_limit: usize,
    ) -> Result<Self> {
        let version = model.strip_prefix("jev-").unwrap_or("");
        let components: Vec<_> = version.split('.').collect();
        ensure!(
            components.len() == 3
                && components
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())),
            "pin a resolved Jev model version such as jev-1.13.0 for reproducibility"
        );
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            key,
            model: model.into(),
            endpoint: endpoint.into(),
            total,
            state_limit,
            max_requests: 24,
            cache: Default::default(),
            openrouter_resolved: None,
        })
    }
    pub async fn ask(
        &mut self,
        state: Value,
        questions: BTreeMap<String, Question>,
        cancel: &CancellationToken,
        metrics: &mut Metrics,
    ) -> Result<DecisionResponse> {
        let request = DecisionRequest {
            model: self.model.clone(),
            state,
            questions,
        };
        check_budget(&request, self.total, self.state_limit)?;
        let cache_key = hash(&serde_json::to_vec(&(
            brand::POLICY_VERSION,
            "decision-rubric-1",
            &request,
            &self.endpoint,
            &self.openrouter_resolved,
        ))?);
        let mut wire = serde_json::to_value(&request)?;
        if self.openrouter_resolved.is_some() {
            wire["provider"] = serde_json::json!({"only":["typesafe"],"allow_fallbacks":false});
            // The gateway requires string descriptions; TypeSafe also permits null.
            for (id, question) in &request.questions {
                if let Question::Choice { criteria, .. } = question {
                    for (option, meaning) in criteria {
                        wire["questions"][id]["criteria"][option] =
                            meaning.as_deref().unwrap_or(option).into();
                    }
                }
            }
        }
        if let Some(cached) = self.cache.get(&cache_key) {
            metrics.cache_hits += 1;
            return Ok(cached.clone());
        }
        for attempt in 0..3 {
            ensure!(
                metrics.decision_requests + metrics.generative_calls < self.max_requests,
                "provider request budget exhausted"
            );
            ensure!(!cancel.is_cancelled(), "decision cancelled");
            metrics.decision_requests += 1;
            metrics.decision_questions += request.questions.len() as u64;
            if attempt > 0 {
                metrics.retries += 1;
            }
            let response = tokio::select! {biased;_=cancel.cancelled()=>bail!("decision cancelled"),r=self.client.post(&self.endpoint).bearer_auth(&self.key).json(&wire).send()=>r};
            match response {
                Ok(mut response) => {
                    let status = response.status().as_u16();
                    if response.status().is_success() {
                        let mut body = vec![];
                        loop {
                            let chunk = tokio::select! {biased;_=cancel.cancelled()=>bail!("decision cancelled"),v=response.chunk()=>v?};
                            let Some(chunk) = chunk else { break };
                            body.extend_from_slice(&chunk);
                            ensure!(body.len() <= 1024 * 1024, "decision response exceeds limit");
                        }
                        let parsed: DecisionResponse =
                            serde_json::from_slice(&body).context("invalid typed Jev response")?;
                        metrics.usage.push(Usage {
                            input_tokens: Some(parsed.usage.input_tokens),
                            output_tokens: Some(parsed.usage.output_tokens),
                            cached_input_tokens: None,
                            cache_creation_input_tokens: None,
                        });
                        if let Some(expected) = &self.openrouter_resolved {
                            let validation_request = DecisionRequest {
                                model: expected.clone(),
                                state: request.state.clone(),
                                questions: request.questions.clone(),
                            };
                            validate_contract(&validation_request, &parsed, false)?;
                        } else {
                            validate(&request, &parsed)?;
                        }
                        if self.cache.len() >= 128 {
                            self.cache.clear();
                        }
                        self.cache.insert(cache_key, parsed.clone());
                        return Ok(parsed);
                    }
                    if !retryable(status) {
                        bail!(
                            "Jev HTTP {status}; authentication/validation or permanent failure; no retry or fabricated answer"
                        );
                    }
                    if attempt == 2 {
                        bail!("Jev retry budget exhausted (HTTP {status})");
                    }
                    let delay = retry_delay(
                        attempt,
                        response
                            .headers()
                            .get("retry-after")
                            .and_then(|s| s.to_str().ok()),
                    );
                    ensure!(
                        delay <= Duration::from_secs(60),
                        "Jev Retry-After exceeds wait budget; try again later"
                    );
                    tokio::select! {biased;_=cancel.cancelled()=>bail!("decision cancelled"),_=tokio::time::sleep(delay)=>{}}
                }
                Err(e) => {
                    if attempt == 2 || (!e.is_timeout() && !e.is_connect()) {
                        bail!("Jev transport failed; request exhausted or unretryable");
                    }
                    let delay = retry_delay(attempt, None);
                    tokio::select! {biased;_=cancel.cancelled()=>bail!("decision cancelled"),_=tokio::time::sleep(delay)=>{}}
                }
            }
        }
        bail!("Jev retry budget exhausted")
    }
}
