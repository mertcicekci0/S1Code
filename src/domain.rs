use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    AwaitingApproval,
    Blocked,
    Cancelled,
    Failed,
    BudgetExhausted,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Native,
    Codex,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyClass {
    Allow,
    Ask,
    Deny,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub path: String,
    /// None means create a new file, never overwrite an existing one.
    pub before_hash: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    List,
    Search {
        query: String,
    },
    Read {
        path: String,
        start: usize,
        lines: usize,
    },
    Patch {
        edits: Vec<Edit>,
    },
    Run {
        argv: Vec<String>,
        verification: bool,
    },
    Git,
    Rehydrate {
        artifact: String,
    },
    AskGenerator,
    Finish {
        summary: String,
    },
    Blocked {
        reason: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateAction {
    pub id: String,
    pub action: Action,
    pub provenance: String,
    pub evidence: Vec<String>,
    pub revision: String,
    pub policy_version: String,
    pub class: PolicyClass,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// Visible, brief plan. Never request or persist hidden reasoning.
    pub message: String,
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub generative_calls: u64,
    pub decision_requests: u64,
    pub decision_questions: u64,
    pub tool_calls: u64,
    pub delegations: u64,
    pub retries: u64,
    pub rehydrations: u64,
    pub cache_hits: u64,
    pub context_invalidations: u64,
    pub simulated_turns: u64,
    pub usage: Vec<Usage>,
    pub elapsed_ms: u64,
    pub cost: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub hash: String,
    pub bytes: usize,
    pub origin: String,
    pub call_id: String,
    pub revision: String,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextItem {
    pub artifact: ArtifactRef,
    pub action: Action,
    pub pinned: bool,
    pub evicted: bool,
    pub diagnostic: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Verification {
    pub argv: Vec<String>,
    pub revision: String,
    pub artifact: String,
    pub exit_code: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    pub mode: Mode,
    pub decision: String,
    pub generation_model: String,
    pub jev_model: String,
    pub max_steps: usize,
    pub max_generations: u64,
    pub context_bytes: usize,
    pub exclusions: Vec<String>,
    pub jev_fallback_rules: bool,
    pub jev_confidence: f64,
    pub jev_request_limit: usize,
    pub jev_state_limit: usize,
    pub eviction: String,
    pub offline_demo: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            mode: Mode::Native,
            decision: "rules".into(),
            generation_model: "gpt-4.1-2025-04-14".into(),
            jev_model: "jev-1.13.0".into(),
            max_steps: 40,
            max_generations: 12,
            context_bytes: 96_000,
            exclusions: vec![],
            jev_fallback_rules: false,
            jev_confidence: 0.5,
            jev_request_limit: 64_000,
            jev_state_limit: 32_000,
            eviction: "conservative".into(),
            offline_demo: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub version: u32,
    pub id: String,
    pub workspace: String,
    pub task: String,
    pub config: RunConfig,
    pub status: RunStatus,
    pub context: Vec<ContextItem>,
    pub pending: Option<CandidateAction>,
    pub inflight: Option<CandidateAction>,
    pub verified: Option<Verification>,
    pub metrics: Metrics,
    pub steps: usize,
    pub seen: BTreeMap<String, usize>,
    pub proposals: Vec<Action>,
    pub bridge_thread: Option<String>,
    pub bridge_turn: Option<String>,
    pub event_seq: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunEvent {
    pub seq: u64,
    pub session: String,
    pub kind: String,
    pub data: Value,
}

#[derive(Clone, Debug)]
pub enum UiInput {
    Approve(String),
    Deny(String),
}
