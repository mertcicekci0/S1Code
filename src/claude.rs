//! Native generation via the public Anthropic Messages API. No subscription tokens.
use crate::{
    domain::Usage,
    generation::{GenerationResult, Generator, INSTRUCTIONS, Sse, proposal_schema, validate},
};
use anyhow::{Context, Result, bail, ensure};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_MODEL: &str = "claude-opus-5";

pub struct Claude {
    client: reqwest::Client,
    key: String,
    model: String,
    endpoint: String,
}
impl Claude {
    pub fn from_env(model: &str) -> Result<Self> {
        Self::new(std::env::var("ANTHROPIC_API_KEY").context(
            "ANTHROPIC_API_KEY missing; native Claude uses the public API, not Claude subscription credentials"
        )?, model, "https://api.anthropic.com/v1/messages")
    }
    pub fn new(key: String, model: &str, endpoint: &str) -> Result<Self> {
        ensure!(
            !key.trim().is_empty() && !model.trim().is_empty(),
            "Claude key and model must be configured"
        );
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            key,
            model: model.into(),
            endpoint: endpoint.into(),
        })
    }
}

fn schema() -> Value {
    fn visit(value: &mut Value) {
        match value {
            Value::Object(object) => {
                // Numeric minimum is unsupported by this provider's JSON grammar.
                // Runtime tool validation still enforces the original range.
                if object.remove("minimum").is_some() {
                    object.insert("description".into(), "Positive integer, at least 1".into());
                }
                for child in object.values_mut() {
                    visit(child);
                }
            }
            Value::Array(array) => {
                for child in array {
                    visit(child);
                }
            }
            _ => {}
        }
    }
    let mut result = proposal_schema();
    visit(&mut result);
    result
}

#[derive(Default)]
struct Message {
    started: bool,
    ended: bool,
    stopped: bool,
    block: Option<(u64, bool)>,
    next_index: u64,
    text: String,
    model: String,
    usage: Usage,
}
impl Message {
    fn consume(&mut self, event: Value, deltas: &mpsc::UnboundedSender<String>) -> Result<()> {
        let kind = event["type"]
            .as_str()
            .context("Claude event type missing")?;
        if kind == "ping" {
            return Ok(());
        }
        ensure!(!self.stopped, "Claude event after message_stop");
        match kind {
            "error" => bail!("Claude stream error; no action accepted"),
            "message_start" => {
                ensure!(!self.started, "duplicate Claude message_start");
                ensure!(
                    event["message"]["content"]
                        .as_array()
                        .is_some_and(Vec::is_empty),
                    "unexpected initial Claude content"
                );
                self.started = true;
                self.model = event["message"]["model"]
                    .as_str()
                    .context("Claude model missing")?
                    .into();
                let u = &event["message"]["usage"];
                self.usage = Usage {
                    input_tokens: u["input_tokens"].as_u64(),
                    output_tokens: u["output_tokens"].as_u64(),
                    cached_input_tokens: u["cache_read_input_tokens"].as_u64(),
                    cache_creation_input_tokens: u["cache_creation_input_tokens"].as_u64(),
                };
            }
            "content_block_start" => {
                ensure!(
                    self.started && !self.ended && self.block.is_none(),
                    "invalid Claude block start"
                );
                let index = event["index"]
                    .as_u64()
                    .context("Claude block index missing")?;
                ensure!(index == self.next_index, "out-of-order Claude block");
                let block = &event["content_block"];
                let text = match block["type"].as_str() {
                    Some("text") => true,
                    Some("thinking" | "redacted_thinking") => false,
                    _ => bail!(
                        "unsupported Claude content; native generation cannot execute upstream tools"
                    ),
                };
                self.block = Some((index, text));
                if text {
                    self.append(
                        block["text"]
                            .as_str()
                            .context("Claude initial text missing")?,
                        deltas,
                    )?;
                }
            }
            "content_block_delta" => {
                let (index, text) = self.block.context("orphan Claude content delta")?;
                ensure!(
                    event["index"].as_u64() == Some(index),
                    "Claude delta index mismatch"
                );
                if text {
                    ensure!(
                        event["delta"]["type"] == "text_delta",
                        "unexpected Claude text delta type"
                    );
                    self.append(
                        event["delta"]["text"]
                            .as_str()
                            .context("Claude text delta missing")?,
                        deltas,
                    )?;
                } // Thinking and signatures are never displayed or persisted.
            }
            "content_block_stop" => {
                let (index, _) = self.block.take().context("orphan Claude block stop")?;
                ensure!(
                    event["index"].as_u64() == Some(index),
                    "Claude stop index mismatch"
                );
                self.next_index += 1;
            }
            "message_delta" => {
                ensure!(
                    self.started && self.block.is_none() && !self.ended,
                    "invalid Claude message delta"
                );
                if !event["delta"]["stop_reason"].is_null() {
                    ensure!(
                        event["delta"]["stop_reason"] == "end_turn",
                        "Claude response refused or incomplete; no action accepted"
                    );
                    self.ended = true;
                }
                if let Some(n) = event["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = Some(n);
                }
            }
            "message_stop" => {
                ensure!(
                    self.started && self.ended && self.block.is_none(),
                    "Claude message ended before completion"
                );
                self.stopped = true;
            }
            _ => {} // Future informational events do not create executable content.
        }
        Ok(())
    }
    fn append(&mut self, text: &str, deltas: &mpsc::UnboundedSender<String>) -> Result<()> {
        ensure!(
            self.text.len() + text.len() <= 512 * 1024,
            "Claude proposal too large"
        );
        self.text.push_str(text);
        let _ = deltas.send(text.into());
        Ok(())
    }
    fn finish(self) -> Result<GenerationResult> {
        ensure!(self.stopped, "Claude stream ended without message_stop");
        let proposal =
            serde_json::from_str(&self.text).context("invalid structured Claude proposal")?;
        validate(&proposal)?;
        Ok(GenerationResult {
            proposal,
            usage: self.usage,
            model: self.model,
        })
    }
}

#[async_trait]
impl Generator for Claude {
    async fn generate(
        &self,
        input: Value,
        cancel: &CancellationToken,
        deltas: mpsc::UnboundedSender<String>,
    ) -> Result<GenerationResult> {
        ensure!(!cancel.is_cancelled(), "generation cancelled");
        let body = json!({"model":self.model,"system":INSTRUCTIONS,"messages":[{"role":"user","content":input.to_string()}],"stream":true,"max_tokens":8192,"output_config":{"format":{"type":"json_schema","schema":schema()}}});
        let response = tokio::select! {biased; _=cancel.cancelled()=>bail!("generation cancelled"), r=self.client.post(&self.endpoint).header("x-api-key", &self.key).header("anthropic-version", "2023-06-01").json(&body).send()=>r.context("Claude transport failed")?};
        if !response.status().is_success() {
            let status = response.status();
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            loop {
                let chunk = tokio::select! {biased; _=cancel.cancelled()=>bail!("generation cancelled"), next=stream.next()=>next};
                let Some(Ok(chunk)) = chunk else { break };
                if bytes.len() + chunk.len() > 16 * 1024 {
                    bytes.clear();
                    break;
                }
                bytes.extend_from_slice(&chunk);
            }
            bail!("{}", http_error(status.as_u16(), &bytes, &self.key));
        }
        let mut stream = response.bytes_stream();
        let mut parser = Sse::default();
        let mut message = Message::default();
        let mut captured = 0;
        loop {
            let chunk = tokio::select! {biased; _=cancel.cancelled()=>bail!("generation cancelled"), next=stream.next()=>next};
            let Some(chunk) = chunk else { break };
            let chunk = chunk.context("Claude stream interrupted")?;
            captured += chunk.len();
            ensure!(
                captured <= 4 * 1024 * 1024,
                "Claude stream exceeded capture budget"
            );
            for event in parser.push(&chunk)? {
                message.consume(event, &deltas)?;
            }
        }
        parser.finish()?;
        message.finish()
    }
}

// Only selected error fields are displayed, never headers or the complete response.
fn http_error(status: u16, body: &[u8], key: &str) -> String {
    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let safe = |text: &str| {
        let text = crate::privacy::Redactor::with_secrets(vec![key.to_owned()]).text(text);
        let text = crate::privacy::Redactor::environment("").text(&text);
        text.split_whitespace()
            .map(|word| {
                if word.contains("sk-") || word.contains("apikey_") {
                    "[REDACTED]"
                } else {
                    word
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(1500)
            .collect::<String>()
    };
    let kind = safe(value["error"]["type"].as_str().unwrap_or("unknown_error"));
    let message = safe(
        value["error"]["message"]
            .as_str()
            .unwrap_or("Provider returned no readable JSON error details."),
    );
    let request = safe(value["request_id"].as_str().unwrap_or("unavailable"));
    format!(
        "Claude HTTP {status} ({kind}): {message}\nRequest ID: {request}. No automatic retry or provider fallback."
    )
}

#[cfg(test)]
mod error_tests {
    use super::*;
    #[test]
    fn http_errors_preserve_cause_without_keys_headers_or_terminal_controls() {
        let body = json!({"error":{"type":"invalid_request_error","message":"credit balance too low. fixture-private-key sk-ant-api-fixture \u{1b}[31m"},"request_id":"req_fixture","headers":{"authorization":"never-display-this"}});
        let text = http_error(
            400,
            &serde_json::to_vec(&body).unwrap(),
            "fixture-private-key",
        );
        assert!(text.contains("credit balance too low") && text.contains("req_fixture"));
        assert!(
            !text.contains("fixture-private-key")
                && !text.contains("sk-ant-api")
                && !text.contains("never-display-this")
                && !text.contains('\u{1b}')
        );
        assert!(
            http_error(502, b"<html>private upstream dump</html>", "key")
                .contains("no readable JSON")
        );
        assert!(
            !http_error(502, b"<html>private upstream dump</html>", "key")
                .contains("private upstream")
        );
    }
}
