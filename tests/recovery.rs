use s1code::{
    bridge::{ApprovalLedger, Rpc, approval_policy, validate_permissions},
    context,
    domain::*,
    privacy::{Redactor, StreamRedactor},
    session::{Store, hash},
    tools::{Workspace, process},
};
use serde_json::json;
use std::{fs, time::Duration};
use tokio_util::sync::CancellationToken;

#[test]
fn approval_id_reuse_cannot_change_arguments_or_execute_twice() {
    let request = json!({"id":"req","method":"item/commandExecution/requestApproval","params":{"command":"cargo test --offline","turnId":"turn"}});
    let mut ledger = ApprovalLedger::default();
    assert!(ledger.cached(&request).unwrap().is_none());
    let reply = json!({"id":"req","result":{"decision":"accept"}});
    ledger.remember(&request, reply.clone());
    assert_eq!(ledger.cached(&request).unwrap(), Some(reply));
    let mut changed = request.clone();
    changed["params"]["command"] = "rm -rf .".into();
    assert!(ledger.cached(&changed).is_err());
    assert!(validate_permissions(&json!({"approvalPolicy":approval_policy(),"approvalsReviewer":"user","sandbox":{"type":"readOnly","networkAccess":false},"cwd":"/fixture"}),"/fixture").is_ok());
    assert!(validate_permissions(&json!({"approvalPolicy":"never","sandbox":{"type":"dangerFullAccess"},"cwd":"/fixture"}),"/fixture").is_err());
}

#[tokio::test]
async fn local_app_server_protocol_handshake_queue_and_request_ids() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(
        &script,
        r#"import sys,json
for line in sys.stdin:
 r=json.loads(line)
 if 'id' not in r: continue
 if r['method']=='initialize':
  assert r['params']['capabilities']['experimentalApi'] is True
  print(json.dumps({'id':r['id'],'result':{'userAgent':'offline protocol fixture'}}),flush=True)
 else:
  print(json.dumps({'method':'fixture/notification','params':{'value':1}}),flush=True)
  print(json.dumps({'id':r['id'],'result':{'ok':True}}),flush=True)
"#,
    )
    .unwrap();
    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg("-u").arg(script);
    let c = CancellationToken::new();
    let mut rpc = Rpc::spawn(cmd, dir.path(), &c).await.unwrap();
    assert_eq!(
        rpc.call("fixture/request", json!({}), &c).await.unwrap(),
        json!({"ok":true})
    );
    assert_eq!(
        rpc.next(&c).await.unwrap()["method"],
        "fixture/notification"
    );
}

#[test]
fn secret_redaction_handles_every_fragment_boundary() {
    let secret = "sk-test-private-1234567890";
    let input = format!("message before {secret} and after\u{1b}[31m");
    for split in 0..input.len() {
        if !input.is_char_boundary(split) {
            continue;
        }
        let mut s = StreamRedactor::new(Redactor::with_secrets(vec![secret.into()]));
        let out = s.push(&input[..split]) + &s.push(&input[split..]) + &s.finish();
        assert!(!out.contains(secret));
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains('\u{1b}'));
    }
}

#[test]
fn single_workspace_writer_across_homes_and_unknown_action_restart() {
    let root = tempfile::tempdir().unwrap();
    let h1 = tempfile::tempdir().unwrap();
    let h2 = tempfile::tempdir().unwrap();
    let (store, mut s) =
        Store::create(h1.path(), root.path(), "task".into(), Default::default()).unwrap();
    assert!(Store::create(h2.path(), root.path(), "other".into(), Default::default()).is_err());
    s.inflight = Some(s1code::policy::candidate(
        Action::Run {
            argv: vec!["python3".into(), "-m".into(), "unittest".into()],
            verification: true,
        },
        "rev",
        "test",
        vec![],
    ));
    store.save(&s).unwrap();
    drop(store);
    let (_, loaded) = Store::resume(h1.path(), &s.id).unwrap();
    assert!(loaded.recovery_needed);
    assert_eq!(loaded.status, RunStatus::Blocked);
}

#[test]
fn patch_recovery_restores_and_refuses_concurrent_user_edit() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a"), "after").unwrap();
    let (store, _) =
        Store::create(home.path(), root.path(), "test".into(), Default::default()).unwrap();
    store
        .put(b"before", "backup", "patch", "rev", vec![])
        .unwrap();
    let journal =
        json!([{"path":"a","before":hash(b"before"),"after":hash(b"after"),"before_mode":493}]);
    fs::write(store.dir.join("patch-recovery.json"), journal.to_string()).unwrap();
    let w = Workspace::new(root.path(), vec![]).unwrap();
    fs::write(root.path().join("a"), "user edit").unwrap();
    assert!(w.recover(&store).is_err());
    assert_eq!(fs::read(root.path().join("a")).unwrap(), b"user edit");
    fs::write(root.path().join("a"), "after").unwrap();
    w.recover(&store).unwrap();
    assert_eq!(fs::read(root.path().join("a")).unwrap(), b"before");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.path().join("a"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
}

#[tokio::test]
async fn git_inspection_disables_monitors_and_rejects_included_filters() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let token = CancellationToken::new();
    for argv in [
        vec!["git", "init", "--quiet"],
        vec![
            "git",
            "config",
            "core.fsmonitor",
            "sh -c 'touch unexpected-monitor'",
        ],
    ] {
        assert_eq!(
            process(
                root.path(),
                &argv.into_iter().map(String::from).collect::<Vec<_>>(),
                &token,
                Duration::from_secs(10)
            )
            .await
            .unwrap()
            .exit_code,
            Some(0)
        );
    }
    fs::write(root.path().join("a.txt"), "before\n").unwrap();
    assert_eq!(
        process(
            root.path(),
            &["git", "-c", "core.fsmonitor=false", "add", "a.txt"].map(String::from),
            &token,
            Duration::from_secs(10)
        )
        .await
        .unwrap()
        .exit_code,
        Some(0)
    );
    fs::write(root.path().join("a.txt"), "after\n").unwrap();
    let (store, _) = Store::create(
        home.path(),
        root.path(),
        "git check".into(),
        Default::default(),
    )
    .unwrap();
    let workspace = Workspace::new(root.path(), vec![]).unwrap();
    let index_before = fs::read(root.path().join(".git/index")).unwrap();
    let result = workspace
        .execute(&Action::Git, &store, &token)
        .await
        .unwrap();
    assert!(result.text.contains("-before") && result.text.contains("+after"));
    assert_eq!(
        fs::read(root.path().join(".git/index")).unwrap(),
        index_before
    );
    assert!(!root.path().join("unexpected-monitor").exists());
    fs::write(
        root.path().join(".git/filters"),
        "[filter \"unsafe\"]\n clean = sh -c 'touch unexpected-filter'\n",
    )
    .unwrap();
    assert_eq!(
        process(
            root.path(),
            &["git", "config", "--local", "include.path", "filters"].map(String::from),
            &token,
            Duration::from_secs(10)
        )
        .await
        .unwrap()
        .exit_code,
        Some(0)
    );
    let error = workspace
        .execute(&Action::Git, &store, &token)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("Git filters"));
    assert!(!root.path().join("unexpected-filter").exists());
}

#[tokio::test]
async fn cancellation_kills_descendants_and_stops_queued_work() {
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("late");
    let script = root.path().join("fork.py");
    let child_code = format!(
        "import time,pathlib;time.sleep(0.8);pathlib.Path({}).write_text('bad')",
        serde_json::to_string(&marker.to_string_lossy()).unwrap()
    );
    fs::write(
        &script,
        format!(
            "import subprocess,time\nsubprocess.Popen(['python3','-c',{}])\ntime.sleep(10)\n",
            serde_json::to_string(&child_code).unwrap()
        ),
    )
    .unwrap();
    let c = CancellationToken::new();
    let cancel = c.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        cancel.cancel();
    });
    assert!(
        process(
            root.path(),
            &["python3".into(), script.to_string_lossy().into()],
            &c,
            Duration::from_secs(5)
        )
        .await
        .is_err()
    );
    tokio::time::sleep(Duration::from_millis(950)).await;
    assert!(!marker.exists());
    assert!(
        process(
            root.path(),
            &["python3".into(), "-c".into(), "raise SystemExit(0)".into()],
            &c,
            Duration::from_secs(1)
        )
        .await
        .is_err()
    );
}

#[test]
fn dependency_closure_and_corrupt_artifact_protection() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (store, mut s) =
        Store::create(home.path(), root.path(), "test".into(), Default::default()).unwrap();
    let a = store
        .put(b"evidence", "read", "call1", "rev", vec![])
        .unwrap();
    let b = store
        .put(b"dependent", "patch", "call2", "rev", vec![a.hash.clone()])
        .unwrap();
    for artifact in [a.clone(), b] {
        s.context.push(ContextItem {
            artifact,
            action: Action::List,
            pinned: false,
            evicted: false,
            diagnostic: false,
        });
    }
    assert!(context::protected(&s.context).contains(&a.hash));
    fs::write(store.dir.join("artifacts").join(&a.hash), b"corrupt").unwrap();
    assert!(store.get(&a.hash).is_err());
}

#[test]
fn eviction_and_rehydration_preserve_transitive_dependencies() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (store, mut s) = Store::create(
        home.path(),
        root.path(),
        "dependency task".into(),
        RunConfig {
            context_bytes: 12_000,
            ..Default::default()
        },
    )
    .unwrap();
    let mut hashes: Vec<String> = vec![];
    for i in 0..5 {
        let dependencies = if i == 1 || i == 2 {
            vec![hashes[i - 1].clone()]
        } else {
            vec![]
        };
        let bytes = if i < 3 {
            format!("{i}:{}", "evidence".repeat(900))
        } else {
            format!("recent {i}")
        };
        let artifact = store
            .put(
                bytes.as_bytes(),
                "test",
                &format!("call-{i}"),
                "rev",
                dependencies,
            )
            .unwrap();
        hashes.push(artifact.hash.clone());
        s.context.push(ContextItem {
            artifact,
            action: Action::List,
            pinned: false,
            evicted: false,
            diagnostic: false,
        });
    }
    let dropped = context::compact(&mut s, &store, None).unwrap();
    assert!(hashes[..3].iter().all(|h| dropped.contains(h)));
    assert!(s.context[..3].iter().all(|c| c.evicted));
    let recovered = context::rehydrate(&mut s, &hashes[2], &store).unwrap();
    assert_eq!(recovered, store.get(&hashes[2]).unwrap());
    assert!(
        s.context
            .iter()
            .filter(|c| hashes[..3].contains(&c.artifact.hash))
            .all(|c| !c.evicted)
    );
    assert!(
        context::compact(&mut s, &store, None).is_err(),
        "rehydrated recent evidence pins its dependency closure; overflow must be explicit"
    );
}

#[test]
fn approvals_cannot_be_reclassified_and_orphan_results_are_unrepresentable() {
    let mut c = s1code::policy::candidate(
        Action::Run {
            argv: vec!["python3".into(), "-m".into(), "unittest".into()],
            verification: true,
        },
        "revision",
        "test",
        vec![],
    );
    c.class = PolicyClass::Allow;
    assert!(s1code::policy::revalidate(&c, "revision").is_err());
    assert!(serde_json::from_value::<ContextItem>(json!({"artifact":{"hash":"x","bytes":1,"origin":"test","call_id":"call","revision":"r","dependencies":[]},"pinned":false,"evicted":false,"diagnostic":false})).is_err());
}

#[test]
fn sanitized_export_and_jev_export_block() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (mut store, mut s) =
        Store::create(home.path(), root.path(), "test".into(), Default::default()).unwrap();
    store.redactor = Redactor::with_secrets(vec!["secret-key-1234".into()]);
    store
        .record(
            &mut s,
            "test",
            json!({"output":"secret-key-1234\u{1b}[31m","access_token":"also-sensitive"}),
        )
        .unwrap();
    let export = home.path().join("trace.jsonl");
    store.export(&s, &export).unwrap();
    let text = fs::read_to_string(export).unwrap();
    assert!(
        !text.contains("secret-key-1234")
            && !text.contains("also-sensitive")
            && !text.contains('\u{1b}')
    );
    assert!(text.contains("REPLAY"));
    s.config.decision = "jev".into();
    assert!(
        store
            .export(&s, &home.path().join("forbidden.jsonl"))
            .is_err()
    );
}

#[test]
fn journal_ahead_of_checkpoint_requires_recovery_and_partial_tail_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (store, mut s) =
        Store::create(home.path(), root.path(), "test".into(), Default::default()).unwrap();
    let old = fs::read(store.dir.join("checkpoint.json")).unwrap();
    store
        .record(&mut s, "intent", json!({"action":"test"}))
        .unwrap();
    fs::write(store.dir.join("checkpoint.json"), old).unwrap();
    let dir = store.dir.clone();
    drop(store);
    let (store, s) = Store::resume(home.path(), &s.id).unwrap();
    assert!(s.recovery_needed);
    drop(store);
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(dir.join("events.jsonl"))
        .unwrap();
    f.write_all(b"{partial").unwrap();
    assert!(Store::resume(home.path(), &s.id).is_err());
}

#[test]
fn renamed_brand_preserves_existing_store_without_merging() {
    let root = tempfile::tempdir().unwrap();
    let current = root.path().join("s1code");
    let legacy = root.path().join("nerve");
    assert_eq!(
        s1code::session::select_home(current.clone(), legacy.clone()),
        current
    );
    std::fs::create_dir_all(legacy.join("sessions")).unwrap();
    assert_eq!(
        s1code::session::select_home(current.clone(), legacy.clone()),
        legacy
    );
    std::fs::create_dir(&current).unwrap();
    assert_eq!(
        s1code::session::select_home(current.clone(), legacy),
        current
    );
    let mut config = serde_json::to_value(RunConfig::default()).unwrap();
    config
        .as_object_mut()
        .unwrap()
        .remove("generation_provider");
    assert_eq!(
        serde_json::from_value::<RunConfig>(config)
            .unwrap()
            .generation_provider,
        "openai"
    );
}
