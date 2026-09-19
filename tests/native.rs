use s1code::{
    context, demo,
    domain::*,
    engine::{Engine, workspace_for},
    generation::Sse,
    policy,
    session::{Store, hash},
    tools::Workspace,
};
use std::{fs, sync::Arc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[test]
fn zero_test_summaries_cannot_stand_in_for_verification() {
    use s1code::tools::empty_test_run;
    let argv = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    for (args, empty, positive) in [
        (
            argv(&["python3", "-m", "unittest", "-v"]),
            "Ran 0 tests in 0.000s\nOK",
            "Ran 1 test in 0.001s\nOK",
        ),
        (
            argv(&["node", "--test"]),
            "# tests 0\n# fail 0",
            "# tests 4\n# fail 0",
        ),
        (
            argv(&["node", "--test"]),
            "ℹ tests 0\nℹ fail 0",
            "ℹ tests 4\nℹ fail 0",
        ),
        (
            argv(&["cargo", "test", "--offline"]),
            "test result: ok. 0 passed; 0 failed; 0 ignored;",
            "test result: ok. 2 passed; 0 failed; 0 ignored;",
        ),
    ] {
        assert!(empty_test_run(&args, empty));
        assert!(!empty_test_run(&args, positive));
        assert!(!empty_test_run(&args, &format!("{empty}\n{positive}")));
        assert!(!empty_test_run(&args, "unrecognized output"));
    }
    assert!(!empty_test_run(
        &argv(&["cargo", "check", "--offline"]),
        "Finished dev profile"
    ));
}

#[tokio::test]
async fn successful_empty_unittest_run_does_not_complete_a_task() {
    use async_trait::async_trait;
    use s1code::generation::{GenerationResult, Generator};
    use serde_json::Value;
    struct EmptyCheck;
    #[async_trait]
    impl Generator for EmptyCheck {
        fn simulated(&self) -> bool {
            true
        }
        async fn generate(
            &self,
            input: Value,
            _: &CancellationToken,
            _: mpsc::UnboundedSender<String>,
        ) -> anyhow::Result<GenerationResult> {
            let tested = input["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["action"]["type"] == "run");
            let action = if tested {
                assert!(input["verification"].is_null());
                assert!(input.to_string().contains("Verification rejected"));
                Action::Finish {
                    summary: "Incorrectly claims an empty test suite is enough".into(),
                }
            } else {
                Action::Run {
                    argv: ["python3", "-m", "unittest", "-v"]
                        .map(String::from)
                        .to_vec(),
                    verification: true,
                }
            };
            Ok(GenerationResult {
                proposal: Proposal {
                    message: String::new(),
                    actions: vec![action],
                },
                model: "offline fixture".into(),
                usage: Usage::default(),
            })
        }
    }
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (store, session) = Store::create(
        home.path(),
        root.path(),
        "verify".into(),
        RunConfig {
            auto_approve: true,
            max_generations: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let (events, mut rx) = mpsc::unbounded_channel();
    let (_tx, input) = mpsc::unbounded_channel();
    let result = Engine {
        workspace: workspace_for(&session).unwrap(),
        store,
        session,
        generator: Arc::new(EmptyCheck),
        cancel: CancellationToken::new(),
        events,
        input,
        interactive: false,
        approved: None,
    }
    .run()
    .await
    .unwrap();
    assert_ne!(result.status, RunStatus::Completed);
    assert!(result.verified.is_none());
    let mut rejected = false;
    while let Some(event) = rx.recv().await {
        rejected |= event.kind == "verification_rejected";
        assert_ne!(event.kind, "completed");
    }
    assert!(rejected);
}

#[tokio::test]
async fn test_discovery_validates_directory_again_before_execution() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    for name in ["tests", "ignored", "excluded", "secrets"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    fs::write(root.path().join(".gitignore"), "ignored/\n").unwrap();
    fs::write(root.path().join("file.py"), "").unwrap();
    let workspace = Workspace::new(root.path(), vec!["excluded".into()]).unwrap();
    let command = |path: &str| {
        ["python3", "-m", "unittest", "discover", "-s", path, "-v"]
            .map(String::from)
            .to_vec()
    };
    assert!(workspace.validate_command(&command("tests")).is_ok());
    for path in [
        "ignored", "excluded", "secrets", "missing", "file.py", "../tests",
    ] {
        assert!(
            workspace.validate_command(&command(path)).is_err(),
            "{path}"
        );
    }
    let (store, _) = Store::create(
        home.path(),
        root.path(),
        "check".into(),
        RunConfig::default(),
    )
    .unwrap();
    let action = Action::Run {
        argv: command("tests"),
        verification: true,
    };
    // A directory swap is not captured by an earlier candidate's file hashes.
    fs::remove_dir(root.path().join("tests")).unwrap();
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        fs::write(
            outside.path().join("test_escape.py"),
            "raise RuntimeError('must not execute')",
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("tests")).unwrap();
        let error = workspace
            .execute(&action, &store, &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("symlink"));
    }
    #[cfg(not(unix))]
    assert!(
        workspace
            .execute(&action, &store, &CancellationToken::new())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejected_commands_give_the_generator_actionable_feedback() {
    use async_trait::async_trait;
    use s1code::generation::{GenerationResult, Generator};
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct CorrectingGenerator(AtomicUsize);
    #[async_trait]
    impl Generator for CorrectingGenerator {
        fn simulated(&self) -> bool {
            true
        }
        async fn generate(
            &self,
            input: Value,
            _: &CancellationToken,
            _: mpsc::UnboundedSender<String>,
        ) -> anyhow::Result<GenerationResult> {
            let action = if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Action::Run {
                    argv: vec!["npm".into(), "test".into()],
                    verification: true,
                }
            } else {
                let reason = input["rejected_proposals"][0]["reason"].as_str().unwrap();
                assert!(reason.contains("unsupported command"));
                assert!(reason.contains("node --test"));
                Action::Answer {
                    message: "This project requires an unsupported test runner.".into(),
                }
            };
            Ok(GenerationResult {
                proposal: Proposal {
                    message: String::new(),
                    actions: vec![action],
                },
                model: "offline fixture".into(),
                usage: Usage::default(),
            })
        }
    }
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (store, session) = Store::create(
        home.path(),
        root.path(),
        "verify".into(),
        RunConfig::default(),
    )
    .unwrap();
    let (events, mut rx) = mpsc::unbounded_channel();
    let (_tx, input) = mpsc::unbounded_channel();
    let result = Engine {
        workspace: workspace_for(&session).unwrap(),
        store,
        session,
        generator: Arc::new(CorrectingGenerator(AtomicUsize::new(0))),
        cancel: CancellationToken::new(),
        events,
        input,
        interactive: false,
        approved: None,
    }
    .run()
    .await
    .unwrap();
    assert_eq!(result.status, RunStatus::AwaitingInput);
    assert_eq!(result.metrics.tool_calls, 1); // Discovery only, never npm.
    assert_eq!(result.metrics.simulated_turns, 2);
    while let Some(event) = rx.recv().await {
        assert_ne!(event.kind, "approval_required");
    }
}

#[tokio::test]
async fn explicit_auto_approve_completes_real_tools_without_approval_prompts() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    demo::fixture(root.path()).unwrap();
    let (store, s) = Store::create(
        home.path(),
        root.path(),
        demo::TASK.into(),
        RunConfig {
            offline_demo: true,
            auto_approve: true,
            ..Default::default()
        },
    )
    .unwrap();
    let id = s.id.clone();
    let (events, mut rx) = mpsc::unbounded_channel();
    let (_input, inputs) = mpsc::unbounded_channel();
    let engine = Engine {
        workspace: workspace_for(&s).unwrap(),
        generator: Arc::new(demo::OfflineDemo {
            workspace: workspace_for(&s).unwrap(),
        }),
        store,
        session: s,
        cancel: CancellationToken::new(),
        events,
        input: inputs,
        interactive: false,
        approved: None,
    };
    let result = engine.run().await.unwrap();
    assert_eq!(result.status, RunStatus::Completed);
    assert_eq!(result.verified.unwrap().exit_code, 0);
    let mut automatic = 0;
    while let Some(event) = rx.recv().await {
        assert_ne!(event.kind, "approval_required");
        if event.kind == "auto_approved" {
            automatic += 1;
            assert!(event.data["candidate"]["id"].is_string());
        }
    }
    assert_eq!(automatic, 3);
    let (_, restored) = Store::resume(home.path(), &id).unwrap();
    assert!(restored.config.auto_approve);
    let (store, session) = Store::resume(home.path(), &id).unwrap();
    let before = session.metrics.tool_calls;
    let (events, mut rx) = mpsc::unbounded_channel();
    let (_input, inputs) = mpsc::unbounded_channel();
    let resumed = Engine {
        workspace: workspace_for(&session).unwrap(),
        generator: Arc::new(demo::OfflineDemo {
            workspace: workspace_for(&session).unwrap(),
        }),
        store,
        session,
        cancel: CancellationToken::new(),
        events,
        input: inputs,
        interactive: true,
        approved: None,
    }
    .run()
    .await
    .unwrap();
    assert_eq!(resumed.metrics.tool_calls, before);
    let history = rx.recv().await.unwrap();
    assert_eq!(history.kind, "session_restored");
    assert!(
        history.data["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "completed")
    );
    while let Some(event) = rx.recv().await {
        assert_ne!(event.kind, "tool_started");
        assert_ne!(event.kind, "approval_required");
    }
    let mut legacy = serde_json::to_value(RunConfig::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("auto_approve");
    assert!(
        !serde_json::from_value::<RunConfig>(legacy)
            .unwrap()
            .auto_approve
    );
}

#[tokio::test]
async fn native_fixture_executes_patch_verification_and_persists() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    demo::fixture(root.path()).unwrap();
    let (store, s) = Store::create(
        home.path(),
        root.path(),
        demo::TASK.into(),
        RunConfig {
            offline_demo: true,
            ..Default::default()
        },
    )
    .unwrap();
    let id = s.id.clone();
    let (events, mut rx) = mpsc::unbounded_channel();
    let (input, inputs) = mpsc::unbounded_channel();
    let engine = Engine {
        workspace: workspace_for(&s).unwrap(),
        generator: Arc::new(demo::OfflineDemo {
            workspace: workspace_for(&s).unwrap(),
        }),
        store,
        session: s,
        cancel: CancellationToken::new(),
        events,
        input: inputs,
        interactive: true,
        approved: None,
    };
    let task = tokio::spawn(engine.run());
    let mut approvals = 0;
    let mut failures = 0;
    while let Some(e) = rx.recv().await {
        if e.kind == "approval_required" {
            approvals += 1;
            input
                .send(UiInput::Approve(
                    e.data["candidate"]["id"].as_str().unwrap().into(),
                ))
                .unwrap();
        }
        if e.kind == "tool_result" && e.data["exit_code"] == 1 {
            failures += 1;
        }
    }
    let s = task.await.unwrap().unwrap();
    assert_eq!(s.status, RunStatus::Completed);
    assert_eq!(approvals, 3);
    assert_eq!(failures, 1);
    assert_eq!(s.metrics.generative_calls, 0);
    assert!(s.metrics.simulated_turns >= 4);
    assert!(s.verified.is_some());
    let (_, loaded) = Store::resume(home.path(), &id).unwrap();
    assert_eq!(loaded.status, RunStatus::Completed);
    assert!(
        fs::read_to_string(root.path().join("parser.py"))
            .unwrap()
            .contains("int(text.strip())")
    );
    fs::write(
        root.path().join("parser.py"),
        "def parse_integer(text):\n    return 0\n",
    )
    .unwrap();
    let (store, session) = Store::resume(home.path(), &id).unwrap();
    let (events, _rx) = mpsc::unbounded_channel();
    let (_input, inputs) = mpsc::unbounded_channel();
    let resumed = Engine {
        workspace: workspace_for(&session).unwrap(),
        generator: Arc::new(demo::OfflineDemo {
            workspace: workspace_for(&session).unwrap(),
        }),
        store,
        session,
        cancel: CancellationToken::new(),
        events,
        input: inputs,
        interactive: false,
        approved: None,
    }
    .run()
    .await
    .unwrap();
    assert_eq!(resumed.status, RunStatus::Blocked);
    assert!(resumed.verified.is_none());
}

#[test]
fn traversal_symlinks_ignored_secrets_and_stale_candidates_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a.txt"), "a").unwrap();
    fs::write(root.path().join(".env"), "secret").unwrap();
    fs::write(root.path().join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.path().join("ignored.txt"), "secret").unwrap();
    let w = Workspace::new(root.path(), vec![]).unwrap();
    for p in ["../a.txt", "/tmp/a", ".git/config", ".env", "ignored.txt"] {
        assert!(w.path(p, false).is_err(), "{p}");
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.path().join("a.txt"), root.path().join("link")).unwrap();
        assert!(w.path("link", false).is_err());
    }
    let c = policy::candidate(
        Action::Read {
            path: "a.txt".into(),
            start: 1,
            lines: 1,
        },
        &w.revision().unwrap(),
        "test",
        vec![],
    );
    fs::write(root.path().join("a.txt"), "changed").unwrap();
    assert!(policy::revalidate(&c, &w.revision().unwrap()).is_err());
    assert_eq!(
        policy::classify(&Action::Run {
            argv: vec!["sh".into(), "-c".into(), "echo bypass".into()],
            verification: true
        }),
        PolicyClass::Deny
    );
}

#[test]
fn whole_patch_validated_before_writes_and_backups_are_exact() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a"), "before").unwrap();
    fs::write(root.path().join("b"), "other").unwrap();
    let w = Workspace::new(root.path(), vec![]).unwrap();
    let (store, _) =
        Store::create(home.path(), root.path(), "test".into(), Default::default()).unwrap();
    let a = Edit {
        path: "a".into(),
        before_hash: Some(hash(b"before")),
        content: "after".into(),
    };
    let b = Edit {
        path: "b".into(),
        before_hash: Some(hash(b"wrong")),
        content: "no".into(),
    };
    assert!(
        w.apply(&[a.clone(), b], &store, &w.revision().unwrap())
            .is_err()
    );
    assert_eq!(fs::read(root.path().join("a")).unwrap(), b"before");
    w.apply(&[a], &store, &w.revision().unwrap()).unwrap();
    assert_eq!(store.get(&hash(b"before")).unwrap(), b"before");
}

#[test]
fn fragmented_utf8_stream_and_partial_frame() {
    let bytes =
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"héllo\"}\r\n\r\n".as_bytes();
    let mut parser = Sse::default();
    let mut output = vec![];
    for b in bytes {
        output.extend(parser.push(&[*b]).unwrap());
    }
    parser.finish().unwrap();
    assert_eq!(output[0]["delta"], "héllo");
    let mut mixed = Sse::default();
    let records = mixed
        .push(b"data: {\"first\":1}\r\n\r\ndata: {\"second\":2}\n\n")
        .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["first"], 1);
    assert_eq!(records[1]["second"], 2);
    mixed.finish().unwrap();
    let mut parser = Sse::default();
    parser.push(b"data: {").unwrap();
    assert!(parser.finish().is_err());
}

#[test]
fn eviction_preserves_bytes_and_pinned_overflow_is_explicit() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (store, mut s) = Store::create(
        home.path(),
        root.path(),
        "task".into(),
        RunConfig {
            context_bytes: 6500,
            ..Default::default()
        },
    )
    .unwrap();
    for i in 0..6 {
        let a = store
            .put(
                format!("{i}{}", "evidence".repeat(180)).as_bytes(),
                "test",
                &i.to_string(),
                "rev",
                vec![],
            )
            .unwrap();
        s.context.push(ContextItem {
            artifact: a,
            action: Action::List,
            pinned: false,
            evicted: false,
            diagnostic: false,
        });
    }
    let dropped = context::compact(&mut s, &store, None).unwrap();
    assert!(!dropped.is_empty());
    let first = &dropped[0];
    let bytes = store.get(first).unwrap();
    assert_eq!(context::rehydrate(&mut s, first, &store).unwrap(), bytes);
    for c in &mut s.context {
        c.pinned = true;
        c.evicted = false;
    }
    assert!(context::compact(&mut s, &store, None).is_err());
}

proptest::proptest! {
    #[test]
    fn unknown_programs_never_gain_permission(name in "[a-z]{1,20}"){
        if name!="cargo"&&name!="python3"{proptest::prop_assert!(!policy::valid_command(&[name]));}
    }
}

#[tokio::test]
async fn exhausted_provider_budget_does_not_discard_an_approved_concrete_action() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(root.path().join("test_small.py"), "import unittest\nclass Check(unittest.TestCase):\n def test_ok(self): self.assertEqual(1+1,2)\n").unwrap();
    let (store, mut session) = Store::create(
        home.path(),
        root.path(),
        "verify".into(),
        RunConfig {
            max_provider_requests: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let workspace = workspace_for(&session).unwrap();
    let chosen = policy::candidate(
        Action::Run {
            argv: vec!["python3".into(), "-m".into(), "unittest".into()],
            verification: true,
        },
        &workspace.revision().unwrap(),
        "persisted candidate fixture",
        vec![],
    );
    session.pending = Some(chosen.clone());
    // Represent a previous decision request consuming the saved session's cap.
    session.metrics.decision_requests = 1;
    let (events, _rx) = mpsc::unbounded_channel();
    let (_input, inputs) = mpsc::unbounded_channel();
    let result = Engine {
        generator: Arc::new(demo::OfflineDemo {
            workspace: workspace_for(&session).unwrap(),
        }),
        workspace,
        store,
        session,
        cancel: CancellationToken::new(),
        events,
        input: inputs,
        interactive: false,
        approved: Some(chosen.id),
    }
    .run()
    .await
    .unwrap();
    assert!(
        result.verified.is_some(),
        "the already selected check can finish without another provider call"
    );
    assert_eq!(result.status, RunStatus::BudgetExhausted);
    assert_eq!(result.metrics.generative_calls, 0);
    assert_eq!(result.metrics.simulated_turns, 0);
    assert_eq!(result.metrics.decision_requests, 1);
}

#[tokio::test]
async fn truncated_generation_recovery_is_bounded_counted_and_never_executes_partial_actions() {
    use s1code::generation::{GenerationResult, Generator};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Responses {
        calls: AtomicUsize,
        reason: &'static str,
        always_fail: bool,
    }
    #[async_trait::async_trait]
    impl Generator for Responses {
        async fn generate(
            &self,
            input: serde_json::Value,
            _: &CancellationToken,
            _: mpsc::UnboundedSender<String>,
        ) -> anyhow::Result<GenerationResult> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 || self.always_fail {
                return Err(s1code::claude::IncompleteResponse {
                    reason: self.reason.into(),
                    usage: Usage {
                        output_tokens: Some(8192),
                        ..Default::default()
                    },
                }
                .into());
            }
            assert!(
                input["generation_recovery"]
                    .as_str()
                    .unwrap()
                    .contains("one small action")
            );
            Ok(GenerationResult {
                proposal: Proposal {
                    message: "Fixture recovered safely".into(),
                    actions: vec![Action::Blocked {
                        reason: "Fixture stops without writes".into(),
                    }],
                },
                usage: Usage {
                    output_tokens: Some(12),
                    ..Default::default()
                },
                model: "offline-fixture".into(),
            })
        }
    }
    for (reason, always_fail, budget, expected, calls) in [
        ("max_tokens", false, 4, RunStatus::Blocked, 2),
        ("max_tokens", false, 1, RunStatus::BudgetExhausted, 1),
        ("max_tokens", true, 4, RunStatus::Failed, 2),
        ("refusal", false, 4, RunStatus::Failed, 1),
    ] {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (store, session) = Store::create(
            home.path(),
            root.path(),
            "Create a site".into(),
            RunConfig {
                max_provider_requests: budget,
                ..Default::default()
            },
        )
        .unwrap();
        let generator = Arc::new(Responses {
            calls: AtomicUsize::new(0),
            reason,
            always_fail,
        });
        let (events, mut rx) = mpsc::unbounded_channel();
        let (_tx, inputs) = mpsc::unbounded_channel();
        let result = Engine {
            workspace: workspace_for(&session).unwrap(),
            generator: generator.clone(),
            store,
            session,
            cancel: CancellationToken::new(),
            events,
            input: inputs,
            interactive: false,
            approved: None,
        }
        .run()
        .await
        .unwrap();
        assert_eq!(result.status, expected);
        assert_eq!(generator.calls.load(Ordering::SeqCst), calls);
        assert_eq!(result.metrics.generative_calls, calls as u64);
        assert_eq!(result.metrics.retries, (calls - 1) as u64);
        assert_eq!(result.metrics.usage.len(), calls);
        assert_eq!(result.metrics.usage[0].output_tokens, Some(8192));
        while let Some(event) = rx.recv().await {
            if event.kind == "tool_started" {
                assert_ne!(event.data["candidate"]["action"]["type"], "patch");
            }
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[test]
fn exact_replacement_materializes_a_reviewable_patch_and_rejects_ambiguity() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("code.js");
    let original = "// şeker\nconst count = 1;\n";
    fs::write(&file, original).unwrap();
    let workspace = Workspace::new(root.path(), vec![]).unwrap();
    let digest = hash(original.as_bytes());
    let action = workspace
        .replacement("code.js", &digest, "count = 1", "count = 2")
        .unwrap();
    let Action::Patch { edits } = action else {
        panic!("expected materialized patch")
    };
    assert_eq!(edits[0].before_hash.as_deref(), Some(digest.as_str()));
    assert_eq!(edits[0].content, "// şeker\nconst count = 2;\n");
    assert_eq!(fs::read_to_string(&file).unwrap(), original); // Proposal does not mutate.
    assert!(
        workspace
            .replacement("code.js", &digest, "", "new")
            .is_err()
    );
    assert!(
        workspace
            .replacement("code.js", &digest, "missing", "new")
            .is_err()
    );
    assert!(
        workspace
            .replacement("code.js", &digest, "count", "count")
            .is_err()
    );
    assert!(
        workspace
            .replacement("../escape", &digest, "x", "y")
            .is_err()
    );
    fs::write(&file, "aaa").unwrap();
    assert!(
        workspace
            .replacement("code.js", &digest, "count", "new")
            .is_err()
    );
    assert!(
        workspace
            .replacement("code.js", &hash(b"aaa"), "aa", "b")
            .is_err()
    );
    let unmaterialized = Action::Replace {
        path: "code.js".into(),
        before_hash: digest,
        old: "a".into(),
        new: "b".into(),
    };
    assert_eq!(policy::classify(&unmaterialized), PolicyClass::Deny);
}

#[tokio::test]
async fn native_followup_keeps_evidence_and_reverifies_with_cumulative_metrics() {
    use s1code::generation::{GenerationResult, Generator};
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Turns(AtomicUsize);
    #[async_trait::async_trait]
    impl Generator for Turns {
        fn simulated(&self) -> bool {
            true
        }
        async fn generate(
            &self,
            input: Value,
            _: &CancellationToken,
            _: mpsc::UnboundedSender<String>,
        ) -> anyhow::Result<GenerationResult> {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            if n >= 3 {
                assert_eq!(input["task"], "two");
                assert_eq!(input["prior_user_requests"], json!(["one"]));
                if n == 3 {
                    assert!(input["verification"].is_null());
                }
            }
            let action = match n {
                0 => Action::Patch {
                    edits: vec![Edit {
                        path: "counter.py".into(),
                        before_hash: None,
                        content: "value = 1\n".into(),
                    }],
                },
                1 | 5 => Action::Run {
                    argv: vec!["python3".into(), "-m".into(), "unittest".into()],
                    verification: true,
                },
                2 | 6 => Action::Finish {
                    summary: "Checks passed".into(),
                },
                3 => Action::Read {
                    path: "counter.py".into(),
                    start: 1,
                    lines: 20,
                },
                4 => Action::Replace {
                    path: "counter.py".into(),
                    before_hash: hash(b"value = 1\n"),
                    old: "value = 1".into(),
                    new: "value = 2".into(),
                },
                _ => panic!("unexpected extra generation"),
            };
            Ok(GenerationResult {
                proposal: Proposal {
                    message: "Offline two-turn fixture".into(),
                    actions: vec![action],
                },
                usage: Usage::default(),
                model: "offline-fixture".into(),
            })
        }
    }
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(root.path().join("test_counter.py"), "import unittest\nfrom counter import value\nclass Test(unittest.TestCase):\n def test_value(self): self.assertIn(value, [1, 2])\n").unwrap();
    let (store, session) = Store::create(
        home.path(),
        root.path(),
        "one".into(),
        RunConfig {
            auto_approve: true,
            interactive_followups: true,
            max_provider_requests: 10,
            ..Default::default()
        },
    )
    .unwrap();
    let (events, mut rx) = mpsc::unbounded_channel();
    let (input, inputs) = mpsc::unbounded_channel();
    let engine = Engine {
        workspace: workspace_for(&session).unwrap(),
        store,
        session,
        generator: Arc::new(Turns(AtomicUsize::new(0))),
        cancel: CancellationToken::new(),
        events,
        input: inputs,
        interactive: true,
        approved: None,
    };
    let worker = tokio::spawn(engine.run());
    let mut turns = 0;
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while let Some(event) = rx.recv().await {
            if event.kind == "input_ready" {
                turns += 1;
                input
                    .send(if turns == 1 {
                        UiInput::Message("two".into())
                    } else {
                        UiInput::Close
                    })
                    .unwrap();
            }
        }
    })
    .await
    .unwrap();
    let s = worker.await.unwrap().unwrap();
    assert_eq!(turns, 2);
    assert_eq!(s.status, RunStatus::Completed);
    assert_eq!(s.metrics.simulated_turns, 7);
    assert_eq!(s.metrics.decision_requests, 0);
    assert_eq!(s.prior_user_requests, vec!["one"]);
    assert_eq!(
        fs::read_to_string(root.path().join("counter.py")).unwrap(),
        "value = 2\n"
    );
    assert_eq!(
        s.verified.as_ref().unwrap().revision,
        workspace_for(&s).unwrap().revision().unwrap()
    );
    let (_, restored) = Store::resume(home.path(), &s.id).unwrap();
    assert_eq!(restored.metrics.simulated_turns, 7);
    assert_eq!(restored.prior_user_requests, s.prior_user_requests);
}

#[test]
fn followup_rejects_unknown_effects_and_secrets_without_resetting_budgets() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (mut store, mut s) = Store::create(
        home.path(),
        root.path(),
        "original constraints".into(),
        Default::default(),
    )
    .unwrap();
    store.redactor = s1code::privacy::Redactor::with_secrets(vec!["private-example-token".into()]);
    s.metrics.generative_calls = 5;
    s.steps = 12;
    s.recovery_needed = true;
    assert!(store.follow_up(&mut s, "change feature").is_err());
    s.recovery_needed = false;
    assert!(store.follow_up(&mut s, "private-example-token").is_err());
    assert!(store.follow_up(&mut s, "").is_err());
    assert!(s.prior_user_requests.is_empty());
    store.follow_up(&mut s, "change feature").unwrap();
    assert_eq!(s.prior_user_requests, vec!["original constraints"]);
    assert_eq!(s.metrics.generative_calls, 5);
    assert_eq!(s.steps, 12);
    assert_eq!(s.config.max_provider_requests, 24);
    assert!(s.pending.is_none() && s.proposals.is_empty() && s.verified.is_none());
}

#[tokio::test]
async fn exact_greetings_do_not_discover_execute_or_call_providers() {
    use s1code::generation::{GenerationResult, Generator};
    struct NoCalls;
    #[async_trait::async_trait]
    impl Generator for NoCalls {
        async fn generate(
            &self,
            _: serde_json::Value,
            _: &CancellationToken,
            _: mpsc::UnboundedSender<String>,
        ) -> anyhow::Result<GenerationResult> {
            panic!("a greeting must not call a provider")
        }
    }
    for greeting in ["hi", " Hello! ", "merhaba", "SELAM."] {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("test.js"),
            "throw new Error('must not execute')",
        )
        .unwrap();
        let (store, session) = Store::create(
            home.path(),
            root.path(),
            greeting.into(),
            RunConfig {
                decision: "jev".into(),
                eviction: "jev".into(),
                auto_approve: true,
                ..Default::default()
            },
        )
        .unwrap();
        let id = session.id.clone();
        let (events, mut rx) = mpsc::unbounded_channel();
        let (_input, inputs) = mpsc::unbounded_channel();
        let result = Engine {
            workspace: workspace_for(&session).unwrap(),
            store,
            session,
            generator: Arc::new(NoCalls),
            cancel: CancellationToken::new(),
            events,
            input: inputs,
            interactive: false,
            approved: None,
        }
        .run()
        .await
        .unwrap();
        assert_eq!(result.status, RunStatus::AwaitingInput);
        assert!(result.context.is_empty());
        assert!(result.verified.is_none());
        assert_eq!(
            result.metrics.tool_calls
                + result.metrics.generative_calls
                + result.metrics.decision_requests,
            0
        );
        let mut answered = false;
        while let Some(event) = rx.recv().await {
            answered |= event.kind == "answered";
            assert!(!matches!(
                event.kind.as_str(),
                "tool_started" | "generation_requested" | "decision_requested" | "completed"
            ));
        }
        assert!(answered);
        let (_store, reloaded) = Store::resume(home.path(), &id).unwrap();
        assert_eq!(reloaded.status, RunStatus::AwaitingInput);
    }
}

#[tokio::test]
async fn greeting_then_answer_keeps_conversation_without_claiming_verified_completion() {
    use s1code::generation::{GenerationResult, Generator};
    struct Answer;
    #[async_trait::async_trait]
    impl Generator for Answer {
        fn simulated(&self) -> bool {
            true
        }
        async fn generate(
            &self,
            input: serde_json::Value,
            _: &CancellationToken,
            _: mpsc::UnboundedSender<String>,
        ) -> anyhow::Result<GenerationResult> {
            assert_eq!(input["task"], "hi, explain what you can do");
            Ok(GenerationResult {
                proposal: Proposal {
                    message: String::new(),
                    actions: vec![Action::Answer {
                        message: "I can help with coding tasks. What would you like to build?"
                            .into(),
                    }],
                },
                usage: Usage::default(),
                model: "offline-test".into(),
            })
        }
    }
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (store, session) = Store::create(
        home.path(),
        root.path(),
        "hi".into(),
        RunConfig {
            interactive_followups: true,
            ..Default::default()
        },
    )
    .unwrap();
    let (events, mut rx) = mpsc::unbounded_channel();
    let (input, inputs) = mpsc::unbounded_channel();
    let worker = tokio::spawn(
        Engine {
            workspace: workspace_for(&session).unwrap(),
            store,
            session,
            generator: Arc::new(Answer),
            cancel: CancellationToken::new(),
            events,
            input: inputs,
            interactive: true,
            approved: None,
        }
        .run(),
    );
    let mut turns = 0;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(event) = rx.recv().await {
            assert_ne!(event.kind, "completed");
            if event.kind == "input_ready" {
                turns += 1;
                input
                    .send(if turns == 1 {
                        UiInput::Message("hi, explain what you can do".into())
                    } else {
                        UiInput::Close
                    })
                    .unwrap();
            }
        }
    })
    .await
    .unwrap();
    let result = worker.await.unwrap().unwrap();
    assert_eq!(turns, 2);
    assert_eq!(result.status, RunStatus::AwaitingInput);
    assert!(result.verified.is_none());
    assert_eq!(result.metrics.simulated_turns, 1);
    assert_eq!(result.prior_user_requests, ["hi"]);
}

#[tokio::test]
async fn npm_checks_require_an_existing_allowed_manifest_script() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(root.path(), vec![]).unwrap();
    let argv = ["npm", "--offline", "run", "test"]
        .map(String::from)
        .to_vec();
    assert!(workspace.validate_command(&argv).is_err());
    fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"build":"node build.js"}}"#,
    )
    .unwrap();
    assert!(workspace.validate_command(&argv).is_err());
    fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"test":"node --test"}}"#,
    )
    .unwrap();
    fs::write(root.path().join("core.test.js"), "const {test}=require('node:test'); const assert=require('node:assert/strict'); test('addition',()=>assert.equal(2+2,4));").unwrap();
    assert!(workspace.validate_command(&argv).is_ok());
    let excluded = Workspace::new(root.path(), vec!["package.json".into()]).unwrap();
    assert!(excluded.validate_command(&argv).is_err());
    // Exercise the actual project-script runner where npm is installed.
    if std::process::Command::new("npm")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        let (store, _) = Store::create(
            home.path(),
            root.path(),
            "check".into(),
            RunConfig::default(),
        )
        .unwrap();
        let result = workspace
            .execute(
                &Action::Run {
                    argv: argv.clone(),
                    verification: true,
                },
                &store,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result.exit_code, Some(0));
        assert!(!s1code::tools::empty_test_run(&argv, &result.text));
        assert!(result.text.contains("addition"));
    }
    fs::write(root.path().join("package.json"), "invalid").unwrap();
    assert!(workspace.validate_command(&argv).is_err());
}

#[test]
fn additional_test_runners_reject_recognized_empty_runs() {
    let npm = ["npm", "--offline", "run", "test"].map(String::from);
    assert!(s1code::tools::empty_test_run(&npm, "# tests 0\n# fail 0"));
    let pytest = ["python3", "-m", "pytest", "-q"].map(String::from);
    assert!(s1code::tools::empty_test_run(
        &pytest,
        "no tests ran in 0.01s"
    ));
    assert!(!s1code::tools::empty_test_run(
        &pytest,
        "==== 3 passed in 0.01s ===="
    ));
}
