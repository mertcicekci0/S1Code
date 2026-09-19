//! Explicit integration checks; ignored by default. Never print provider responses.
use s1code::{
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
#[ignore = "billed Jev request; requires TYPESAFE_API_KEY and explicit S1CODE_LIVE_BUDGET_REQUESTS"]
async fn jev_live_contract() {
    let budget = std::env::var("S1CODE_LIVE_BUDGET_REQUESTS")
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

#[tokio::test]
#[ignore = "billed OpenRouter decision request; needs OPENROUTER_API_KEY and explicit S1CODE_LIVE_BUDGET_REQUESTS"]
async fn openrouter_live_contract() {
    let budget = std::env::var("S1CODE_LIVE_BUDGET_REQUESTS")
        .expect("explicit request spend consent required")
        .parse::<u64>()
        .unwrap();
    assert!(budget > 0);
    let config = s1code::domain::RunConfig {
        jev_provider: "openrouter".into(),
        jev_model: s1code::decisions::OPENROUTER_MODEL.into(),
        ..Default::default()
    };
    let mut adapter = Jev::from_config(&config).unwrap();
    adapter.max_requests = budget.min(3);
    adapter
        .ask(
            json!({"task":"Fix parser", "evidence":"parse_count('42') returned 4"}),
            BTreeMap::from([(
                "relevant".into(),
                Question::Noul {
                    instructions: "Does the observed failure concern the parser task?".into(),
                },
            )]),
            &CancellationToken::new(),
            &mut Metrics::default(),
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "one billed Claude generation request; needs ANTHROPIC_API_KEY and explicit S1CODE_LIVE_BUDGET_REQUESTS"]
async fn claude_live_contract() -> anyhow::Result<()> {
    use s1code::generation::Generator;
    let budget: u64 = std::env::var("S1CODE_LIVE_BUDGET_REQUESTS")
        .expect("explicit spend consent required")
        .parse()
        .unwrap();
    assert!(budget > 0);
    let model = std::env::var("S1CODE_LIVE_CLAUDE_MODEL")
        .unwrap_or_else(|_| s1code::claude::DEFAULT_MODEL.into());
    let generator = s1code::claude::Claude::from_env(&model)?;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let result = generator.generate(
        json!({"task":"Inspect the repository before proposing a small parser fix", "evidence":[]}),
        &CancellationToken::new(),tx).await?;
    assert!(!result.proposal.actions.is_empty());
    // No generated actions execute. Outputs remain in memory and are not published.
    Ok(())
}

/// Real generation and retention under pressure. The controller approves only
/// parser.py patches and the fixture's exact unittest commands. Never publishes
/// provider responses or measurements; inspect the private output locally.
#[tokio::test]
#[ignore = "billed Claude/Jev coding task; requires explicit request budget and S1CODE_LIVE_OUTPUT private directory"]
async fn native_context_pressure_live() -> anyhow::Result<()> {
    use anyhow::ensure;
    use s1code::{
        context, demo,
        domain::*,
        engine::{Engine, workspace_for},
        generation, policy,
        session::{Store, atomic_write, private_dir},
    };
    let budget: u64 = std::env::var("S1CODE_LIVE_BUDGET_REQUESTS")?.parse()?;
    ensure!(budget > 0, "explicit positive spend consent required");
    let output = std::path::PathBuf::from(std::env::var("S1CODE_LIVE_OUTPUT")?);
    ensure!(
        !output.exists() || output.read_dir()?.next().is_none(),
        "private live output must be empty"
    );
    private_dir(&output)?;
    atomic_write(&output.join(".gitignore"), b"*\n")?;
    // Keep the workspace outside the private trace tree: its catch-all ignore
    // rule is intentionally respected by native repository discovery.
    let root = tempfile::Builder::new()
        .prefix("s1code-live-workspace-")
        .tempdir()?
        .keep();
    demo::fixture(&root)?;
    let init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .status()?;
    ensure!(init.success(), "fixture git initialization failed");
    let original_tests = std::fs::read(root.join("test_parser.py"))?;
    let mut names = vec![];
    for i in 0..10 {
        let name = format!("old-build-{i}.txt");
        std::fs::write(root.join(&name), (0..100).map(|n| format!("Historical unrelated build {i} entry {n}: documentation index regenerated successfully.\n")).collect::<String>())?;
        names.push(name);
    }
    names.extend(["parser.py".into(), "test_parser.py".into()]);
    let (store, mut session) = Store::create(
        &output.join("data"),
        &root,
        format!(
            "{} Old build logs are unrelated historical evidence; only edit parser.py and leave tests unchanged.",
            demo::TASK
        ),
        RunConfig {
            generation_provider: "claude".into(),
            generation_model: s1code::claude::DEFAULT_MODEL.into(),
            decision: "jev".into(),
            eviction: "jev".into(),
            max_provider_requests: budget.min(16),
            max_generations: 8,
            context_bytes: 32_000,
            ..Default::default()
        },
    )?;
    let workspace = workspace_for(&session)?;
    session.current_revision = workspace.revision()?;
    let cancel = CancellationToken::new();
    // Populate pressure using actual bounded reads, not synthetic model answers.
    for name in names {
        let action = Action::Read {
            path: name,
            start: 1,
            lines: 100,
        };
        let candidate = policy::candidate(
            action.clone(),
            &session.current_revision,
            "live fixture read",
            vec![],
        );
        let result = workspace.execute(&action, &store, &cancel).await?;
        let artifact = store.put(
            result.text.as_bytes(),
            "native_read",
            &candidate.id,
            &session.current_revision,
            vec![],
        )?;
        session.context.push(ContextItem {
            artifact: artifact.clone(),
            action,
            pinned: false,
            evicted: false,
            diagnostic: false,
        });
        session.metrics.tool_calls += 1;
        store.record(
            &mut session,
            "tool_result",
            json!({"call_id":candidate.id,"artifact":artifact,"content":result.text}),
        )?;
    }
    let generator = generation::from_config(&session.config)?;
    let (events, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let (tx, input) = tokio::sync::mpsc::unbounded_channel();
    let engine = Engine {
        workspace,
        store,
        session,
        generator,
        cancel,
        events,
        input,
        interactive: true,
        approved: None,
    };
    let task = tokio::spawn(engine.run());
    while let Some(event) = rx.recv().await {
        if event.kind == "approval_required" {
            let candidate: CandidateAction =
                serde_json::from_value(event.data["candidate"].clone())?;
            let allowed = match candidate.action {
                Action::Patch { edits } => {
                    !edits.is_empty() && edits.iter().all(|e| e.path == "parser.py")
                }
                Action::Run {
                    argv,
                    verification: true,
                } => [
                    vec!["python3", "-m", "unittest"],
                    vec!["python3", "-m", "unittest", "-v"],
                    vec!["python3", "-m", "unittest", "discover", "-v"],
                ]
                .iter()
                .any(|a| *a == argv),
                _ => false,
            };
            tx.send(if allowed {
                UiInput::Approve(candidate.id)
            } else {
                UiInput::Deny(candidate.id)
            })?;
        }
    }
    let completed = task.await??;
    ensure!(
        completed.status == RunStatus::Completed,
        "native live task did not complete; inspect private journal"
    );
    ensure!(completed.verified.is_some(), "missing verification");
    ensure!(
        std::fs::read(root.join("test_parser.py"))? == original_tests,
        "fixture tests changed"
    );
    let (store, mut session) = Store::resume(&output.join("data"), &completed.id)?;
    let id = session
        .context
        .iter()
        .find(|c| c.evicted)
        .map(|c| c.artifact.hash.clone())
        .ok_or_else(|| anyhow::anyhow!("live run did not evict evidence"))?;
    let expected = store.get(&id)?;
    ensure!(
        context::rehydrate(&mut session, &id, &store)? == expected,
        "rehydration mismatch"
    );
    store.record(
        &mut session,
        "rehydrated",
        json!({"artifact":id,"rerun":false,"source":"live fixture verification"}),
    )?;
    atomic_write(
        &output.join("result.json"),
        &serde_json::to_vec(
            &json!({"coding_completed":true,"exact_rehydration":true,"metrics":session.metrics}),
        )?,
    )?;
    Ok(())
}
