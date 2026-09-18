//! Explicit integration checks; ignored by default. Never print provider responses.
use nerve::{
    bridge,
    decisions::{Jev, Question},
    domain::Metrics,
};
use serde_json::json;
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "starts official local App Server and creates an ephemeral disposable thread; no inference"]
async fn codex_local_protocol() {
    let root = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let mut rpc = bridge::Rpc::start(root.path(), &cancel).await.unwrap();
    let account = rpc
        .call("account/read", json!({"refreshToken":false}), &cancel)
        .await
        .unwrap();
    assert!(account.get("requiresOpenaiAuth").is_some());
    let cwd = root
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let result=rpc.call("thread/start",json!({"cwd":cwd,"ephemeral":true,"sandbox":"read-only","approvalPolicy":bridge::approval_policy(),"config":{"approvals_reviewer":"user","analytics.enabled":false,"mcp_servers":{},"apps._default.enabled":false}}),&cancel).await.unwrap();
    bridge::validate_permissions(&result, &cwd).unwrap();
}

#[tokio::test]
#[ignore = "billed Jev request; requires TYPESAFE_API_KEY and explicit NERVE_LIVE_BUDGET_REQUESTS"]
async fn jev_live_contract() {
    let budget = std::env::var("NERVE_LIVE_BUDGET_REQUESTS")
        .expect("explicit request spend consent required")
        .parse::<u64>()
        .unwrap();
    assert!(budget > 0);
    let mut jev = Jev::from_env("jev-1.13.0", 64000, 32000).unwrap();
    jev.max_requests = budget.min(3);
    let questions = BTreeMap::from([
        (
            "next".into(),
            Question::Choice {
                instructions: "Which action directly provides evidence about the failing test?"
                    .into(),
                criteria: BTreeMap::from([
                    ("read".into(), Some("Read the named test source".into())),
                    ("blocked".into(), Some("Cannot gather evidence".into())),
                ]),
            },
        ),
        (
            "retain".into(),
            Question::Noul {
                instructions: "Is this failure relevant to the parser task?".into(),
            },
        ),
        (
            "relevance".into(),
            Question::Score {
                instructions: "Rate relevance to the parser bug".into(),
                criteria: vec!["unrelated".into(), "directly relevant".into()],
            },
        ),
    ]);
    let mut metrics = Metrics::default();
    jev.ask(
        json!({"task":"Fix parser test","evidence":"parse_count('42') returned 4; expected 42"}),
        questions,
        &CancellationToken::new(),
        &mut metrics,
    )
    .await
    .unwrap();
    // Results stay in memory; do not publish provider measurements.
}
