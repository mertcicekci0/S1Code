use nerve::{
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
