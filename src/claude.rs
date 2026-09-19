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

/// Stable task/evidence prefixes precede changing workspace and verification
/// metadata. Capture revisions remain attached to every historical snapshot.
fn content_blocks(input: Value) -> Vec<Value> {
    let Value::Object(mut fields) = input else {
        return vec![json!({"type":"text","text":input.to_string()})];
    };
    let mut blocks = vec![];
    let mut stable = serde_json::Map::new();
    for key in ["task", "constraints"] {
        if let Some(value) = fields.remove(key) {
            stable.insert(key.into(), value);
        }
    }
    if !stable.is_empty() {
        blocks.push(json!({"type":"text","text":Value::Object(stable).to_string()}));
    }
    if fields.get("evidence").is_some_and(Value::is_array) {
        let evidence = fields.remove("evidence").unwrap();
        let mut freshness = vec![];
        for (index, mut item) in evidence.as_array().unwrap().iter().cloned().enumerate() {
            if let Some(object) = item.as_object_mut()
                && let Some(historical) = object.remove("historical")
            {
                freshness.push(json!({"evidence_index":index,"historical":historical}));
            }
            blocks.push(json!({"type":"text","text":json!({"evidence":[item]}).to_string()}));
        }
        fields.insert("evidence_freshness".into(), json!(freshness));
    }
    // One explicit five-minute breakpoint; mutable state is outside the cached
    // prefix. Provider usage is authoritative: a marker does not imply a hit.
    if let Some(last) = blocks.last_mut() {
        last["cache_control"] = json!({"type":"ephemeral"});
    }
    blocks.push(json!({"type":"text","text":Value::Object(fields).to_string()}));
    blocks
}

pub struct Claude {
    client: reqwest::Client,
    key: String,
    model: String,
    endpoint: String,
    max_output_tokens: u32,
    effort: Option<String>,
}
impl Claude {
    pub fn from_env(model: &str) -> Result<Self> {
        Self::new(crate::credentials::get("ANTHROPIC_API_KEY").context(
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
            max_output_tokens: crate::domain::default_output_limit(),
            effort: if model.starts_with("claude-opus-5") || model.starts_with("claude-sonnet-5") {
                Some("medium".into())
            } else {
                None
            },
        })
    }
    pub fn configure(mut self, max_output_tokens: u32, effort: Option<String>) -> Result<Self> {
        ensure!(
            (1024..=65536).contains(&max_output_tokens),
            "output token limit must be 1024..65536"
        );
        if let Some(effort) = effort {
            ensure!(
                ["low", "medium", "high", "xhigh", "max"].contains(&effort.as_str()),
                "invalid Claude effort"
            );
            self.effort = Some(effort);
        }
        self.max_output_tokens = max_output_tokens;
        Ok(self)
    }
    fn request(&self, input: Value) -> Value {
        let mut body = json!({"model":self.model,"system":format!("{} Use the provided client tools to propose the next action. These tools do not execute on the provider: S1Code validates, selects and executes locally. Emit exactly one tool call for the next step and brief ordinary text if helpful. Do not wrap calls in JSON prose or fill unrelated arguments. Tool names select the action type; input contains only arguments declared in that tool input_schema.", INSTRUCTIONS),"messages":[{"role":"user","content":content_blocks(input)}],"stream":true,"max_tokens":self.max_output_tokens,"tools":action_tools(),"tool_choice":{"type":"auto","disable_parallel_tool_use":true}});
        if let Some(effort) = &self.effort {
            body["output_config"] = json!({"effort":effort});
        }
        body
    }
}

#[cfg(test)]
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
    // A single object avoids committing to a no-argument union branch before
    // decoding the discriminator. Domain validation still enforces each action.
    let variants = result["properties"]["actions"]["items"]["anyOf"]
        .as_array()
        .unwrap();
    let mut properties = serde_json::Map::new();
    let mut kinds = Vec::new();
    for variant in variants {
        let kind = variant["properties"]["type"]["enum"][0].as_str().unwrap();
        kinds.push(json!(kind));
        let mut fields = variant["properties"].as_object().unwrap().clone();
        fields.remove("type");
        if !fields.is_empty() {
            let required: Vec<_> = fields.keys().cloned().collect();
            properties.insert(kind.into(),json!({"anyOf":[{"type":"object","properties":fields,"required":required,"additionalProperties":false},{"type":"null"}]}));
        }
    }
    properties.insert("type".into(), json!({"type":"string","enum":kinds}));
    let required: Vec<_> = properties.keys().cloned().collect();
    result["properties"]["actions"]["items"] = json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    result
}

fn action_tools() -> Vec<Value> {
    proposal_schema()["properties"]["actions"]["items"]["anyOf"]
        .as_array().unwrap().iter().filter_map(|variant| {
            let kind = variant["properties"]["type"]["enum"][0].as_str().unwrap();
            if kind == "ask_generator" { return None; }
            let mut input = variant.clone();
            input["properties"].as_object_mut().unwrap().remove("type");
            input["required"].as_array_mut().unwrap().retain(|v| v != "type");
            let purpose = match kind {
                "read" => "Read existing UTF-8 file lines and its exact hash before editing. At most 300 lines.",
                "replace" => "Replace one unique exact snippet in a previously read file. Supply the observed full-file before_hash plus old and new text. Runtime materializes a complete patch for review; no fuzzy matching.",
                "patch" => "Propose complete replacement content for one file. Use the exact observed before_hash; null creates a new file. This does not apply immediately: runtime policy and consent are enforced.",
                "run" => "Propose a supported test command with exact argv and verification=true. Runtime controls execution and permissions.",
                "finish" => "Finish only after real current verification passes and all requested files/features are present. Never finish a partial task.",
                "blocked" => "Explain a concrete limitation only when evidence gathering or a supported action cannot resolve it.",
                "search" => "Find a literal identifier in repository text, with bounded results.",
                "rehydrate" => "Recover an evicted artifact's exact bytes without rerunning the historical action.",
                "list" => "List bounded repository files.",
                _ => "Inspect repository git status and diff without changing git state.",
            };
            Some(json!({"name":kind,"description":purpose,"input_schema":input}))
        }).collect()
}

struct ToolInput {
    id: String,
    name: String,
    initial: Value,
    json: String,
}
impl ToolInput {
    fn action(self) -> Result<crate::domain::Action> {
        let mut value = if self.json.is_empty() {
            self.initial
        } else {
            ensure!(
                self.initial.as_object().is_some_and(|o| o.is_empty()),
                "mixed initial and streamed tool input"
            );
            serde_json::from_str(&self.json).context("incomplete Claude tool input")?
        };
        let object = value
            .as_object_mut()
            .context("Claude tool input must be an object")?;
        ensure!(
            !object.contains_key("type"),
            "tool input cannot override its action type"
        );
        object.insert("type".into(), json!(self.name));
        serde_json::from_value(value).context("invalid Claude tool arguments")
    }
}

fn decode_proposal(text: &str) -> Result<crate::domain::Proposal> {
    let mut value: Value =
        serde_json::from_str(text).context("invalid structured Claude proposal")?;
    if let Some(actions) = value["actions"].as_array_mut() {
        for action in actions {
            if let Some(fields) = action.as_object_mut() {
                let kind = fields
                    .get("type")
                    .and_then(Value::as_str)
                    .context("Claude action type missing")?
                    .to_owned();
                let names = [
                    "read",
                    "search",
                    "patch",
                    "replace",
                    "run",
                    "rehydrate",
                    "blocked",
                    "finish",
                ];
                if names.iter().any(|name| fields.contains_key(*name)) {
                    ensure!(
                        fields
                            .keys()
                            .all(|name| name == "type" || names.contains(&name.as_str())),
                        "unknown Claude wire argument"
                    );
                    let mut canonical = serde_json::Map::new();
                    canonical.insert("type".into(), json!(kind));
                    for name in names {
                        if let Some(payload) = fields.get(name) {
                            if name == kind {
                                let payload = payload
                                    .as_object()
                                    .context("selected Claude action payload missing")?;
                                ensure!(
                                    !payload.contains_key("type"),
                                    "nested action type forbidden"
                                );
                                canonical.extend(payload.clone());
                            } else {
                                ensure!(payload.is_null(), "multiple Claude action payloads");
                            }
                        }
                    }
                    *fields = canonical;
                }
            }
        }
    }
    let domain_schema = proposal_schema();
    if let Some(actions) = value["actions"].as_array() {
        for action in actions {
            let variant = domain_schema["properties"]["actions"]["items"]["anyOf"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["properties"]["type"]["enum"][0] == action["type"])
                .context("unknown Claude action type")?;
            let allowed = variant["properties"].as_object().unwrap();
            ensure!(
                action
                    .as_object()
                    .is_some_and(|fields| fields.keys().all(|k| allowed.contains_key(k))),
                "Claude supplied arguments for another action type"
            );
        }
    }
    let proposal = serde_json::from_value(value).context("invalid Claude action arguments")?;
    validate(&proposal)?;
    Ok(proposal)
}

/// Terminal metadata only: never retains partial code or hidden reasoning.
#[derive(Debug)]
pub struct IncompleteResponse {
    pub reason: String,
    pub usage: Usage,
}
impl std::fmt::Display for IncompleteResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let explanation = match self.reason.as_str() {
            "max_tokens" => "output token limit reached; partial actions discarded",
            "refusal" => "provider refused the response; no action accepted",
            "model_context_window_exceeded" => {
                "provider context window exceeded; no action accepted"
            }
            _ => "unsupported completion reason; no action accepted",
        };
        write!(f, "Claude stop_reason={}: {explanation}", self.reason)
    }
}
impl std::error::Error for IncompleteResponse {}

#[derive(Default)]
struct Message {
    started: bool,
    ended: bool,
    stopped: bool,
    stop_reason: Option<String>,
    block: Option<(u64, bool)>,
    next_index: u64,
    tool: Option<ToolInput>,
    tools: Vec<ToolInput>,
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
                    reasoning_tokens: None,
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
                    Some("tool_use") => {
                        let name = block["name"].as_str().context("tool name missing")?;
                        ensure!(
                            action_tools().iter().any(|t| t["name"] == name),
                            "unknown native tool proposal"
                        );
                        let id = block["id"].as_str().context("tool ID missing")?;
                        ensure!(
                            !id.is_empty() && !self.tools.iter().any(|t| t.id == id),
                            "duplicate or empty tool ID"
                        );
                        ensure!(self.tools.len() < 4, "too many proposed tools");
                        self.tool = Some(ToolInput {
                            id: id.into(),
                            name: name.into(),
                            initial: block["input"].clone(),
                            json: String::new(),
                        });
                        false
                    }
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
                } else if let Some(tool) = &mut self.tool {
                    ensure!(
                        event["delta"]["type"] == "input_json_delta",
                        "unexpected tool input delta"
                    );
                    let part = event["delta"]["partial_json"]
                        .as_str()
                        .context("tool input fragment missing")?;
                    ensure!(
                        tool.json.len() + part.len() <= 512 * 1024,
                        "tool input exceeds byte limit"
                    );
                    tool.json.push_str(part);
                } // Thinking and signatures are never displayed or persisted.
            }
            "content_block_stop" => {
                let (index, _) = self.block.take().context("orphan Claude block stop")?;
                ensure!(
                    event["index"].as_u64() == Some(index),
                    "Claude stop index mismatch"
                );
                if let Some(tool) = self.tool.take() {
                    self.tools.push(tool);
                }
                self.next_index += 1;
            }
            "message_delta" => {
                ensure!(
                    self.started && self.block.is_none() && !self.ended,
                    "invalid Claude message delta"
                );
                if !event["delta"]["stop_reason"].is_null() {
                    let reason = event["delta"]["stop_reason"]
                        .as_str()
                        .context("invalid Claude stop_reason")?;
                    // Only known protocol constants enter logs; no arbitrary provider text.
                    self.stop_reason = Some(
                        match reason {
                            "end_turn"
                            | "max_tokens"
                            | "refusal"
                            | "model_context_window_exceeded"
                            | "tool_use"
                            | "pause_turn"
                            | "stop_sequence" => reason,
                            _ => "unknown",
                        }
                        .into(),
                    );
                    self.ended = true;
                }
                if let Some(n) = event["usage"]["output_tokens_details"]["thinking_tokens"].as_u64()
                {
                    self.usage.reasoning_tokens = Some(n);
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
        if !matches!(self.stop_reason.as_deref(), Some("end_turn" | "tool_use")) {
            return Err(IncompleteResponse {
                reason: self.stop_reason.unwrap_or_else(|| "unknown".into()),
                usage: self.usage,
            }
            .into());
        }
        let proposal = if self.stop_reason.as_deref() == Some("tool_use") {
            ensure!(
                !self.tools.is_empty(),
                "tool_use completion without tool proposals"
            );
            crate::domain::Proposal {
                message: self.text,
                actions: self
                    .tools
                    .into_iter()
                    .map(ToolInput::action)
                    .collect::<Result<Vec<_>>>()?,
            }
        } else {
            ensure!(
                self.tools.is_empty(),
                "tool proposals without tool_use completion"
            );
            // Explicit legacy JSON responses remain accepted, never prose-parsed actions.
            decode_proposal(&self.text)?
        };
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
    fn streams_plain_text(&self) -> bool {
        true
    }
    async fn generate(
        &self,
        input: Value,
        cancel: &CancellationToken,
        deltas: mpsc::UnboundedSender<String>,
    ) -> Result<GenerationResult> {
        ensure!(!cancel.is_cancelled(), "generation cancelled");
        let body = self.request(input);
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
    fn growing_evidence_keeps_cacheable_prefix_and_all_dynamic_fields() {
        let original = json!({"task":"fix parser","constraints":"never run without approval",
            "current_workspace_revision":"old", "verification":null,
            "evidence":[{"artifact":{"hash":"a","revision":"old"},"content":"exact\nbytes", "historical":false}],
            "candidates":[{"id":"choice-a"}], "selection_contract":"choose one"});
        let first = content_blocks(original.clone());
        let mut changed = original;
        changed["current_workspace_revision"] = json!("new");
        changed["verification"] = json!({"exit_code":0});
        changed["evidence"][0]["historical"] = json!(true);
        changed["evidence"].as_array_mut().unwrap().push(json!({"artifact":{"hash":"b","revision":"new"},"content":"more evidence","historical":false}));
        let next = content_blocks(changed);
        assert_eq!(first[0]["text"], next[0]["text"]);
        assert_eq!(first[1]["text"], next[1]["text"]);
        assert_eq!(next[2]["cache_control"]["type"], "ephemeral");
        assert!(next.last().unwrap().get("cache_control").is_none());
        assert_eq!(
            next.iter()
                .filter(|b| b.get("cache_control").is_some())
                .count(),
            1
        );
        let tail: Value =
            serde_json::from_str(next.last().unwrap()["text"].as_str().unwrap()).unwrap();
        assert_eq!(tail["current_workspace_revision"], "new");
        assert_eq!(tail["verification"]["exit_code"], 0);
        assert_eq!(tail["evidence_freshness"][0]["historical"], true);
        assert_eq!(tail["candidates"][0]["id"], "choice-a");
        assert_eq!(tail["selection_contract"], "choose one");
        let saved: Value = serde_json::from_str(next[1]["text"].as_str().unwrap()).unwrap();
        assert_eq!(saved["evidence"][0]["content"], "exact\nbytes");
        assert_eq!(saved["evidence"][0]["artifact"]["revision"], "old");
        let mut evicted: Value = saved;
        evicted["evidence"][0]["content"] = json!("[EVICTED]");
        assert_ne!(content_blocks(evicted)[0]["text"], next[1]["text"]);
    }

    #[test]
    fn named_payload_actions_recover_exact_arguments_and_reject_cross_type_fields() {
        let wire = schema();
        let item = &wire["properties"]["actions"]["items"];
        assert!(item.get("anyOf").is_none());
        for action in [
            json!({"type":"read","path":"parser.py","start":1,"lines":120}),
            json!({"type":"search","query":"parse_count"}),
            json!({"type":"patch","edits":[{"path":"new.py","before_hash":null,"content":"pass\n"}]}),
            json!({"type":"run","argv":["python3","-m","unittest"],"verification":true}),
            json!({"type":"list"}),
        ] {
            let mut padded = json!({"type":action["type"]});
            let kind = action["type"].as_str().unwrap();
            if kind != "list" {
                let mut args = action.as_object().unwrap().clone();
                args.remove("type");
                padded[kind] = json!(args);
            }
            for name in item["properties"].as_object().unwrap().keys() {
                padded
                    .as_object_mut()
                    .unwrap()
                    .entry(name)
                    .or_insert(Value::Null);
            }
            let decoded =
                decode_proposal(&json!({"message":"Inspect","actions":[padded]}).to_string())
                    .unwrap();
            assert_eq!(serde_json::to_value(&decoded.actions[0]).unwrap(), action);
        }
        for action in [
            json!({"type":"read","path":"parser.py","start":null,"lines":10}),
            json!({"type":"list","path":"parser.py"}),
            json!({"type":"list","invented":null}),
            json!({"type":"read","read":{"path":"parser.py","start":1,"lines":40},"run":{"argv":["python3","-m","unittest"],"verification":true}}),
            json!({"type":"read","read":{"path":"parser.py","start":1,"lines":40,"reason":"extra"}}),
            json!({"type":"read","read":null}),
        ] {
            assert!(
                decode_proposal(&json!({"message":"Inspect","actions":[action]}).to_string())
                    .is_err()
            );
        }
    }
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
