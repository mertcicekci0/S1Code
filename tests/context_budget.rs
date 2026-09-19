use s1code::{context, domain::*, session::Store};
use serde_json::Value;

fn fixture(count: usize, text: &str) -> (tempfile::TempDir, tempfile::TempDir, Store, Session) {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let (store, mut session) = Store::create(
        home.path(),
        root.path(),
        "find the regression".into(),
        RunConfig::default(),
    )
    .unwrap();
    for i in 0..count {
        let artifact = store
            .put(
                format!("{i}:{text}").as_bytes(),
                "fixture",
                &i.to_string(),
                "revision",
                vec![],
            )
            .unwrap();
        session.context.push(ContextItem {
            artifact,
            action: Action::List,
            pinned: false,
            evicted: false,
            diagnostic: false,
        });
    }
    (root, home, store, session)
}

#[test]
fn overflow_is_transactional_and_small_evidence_is_not_inflated() {
    let (_root, _home, store, mut s) = fixture(6, &"evidence".repeat(500));
    let tiny = store
        .put(b"ok", "fixture", "tiny", "revision", vec![])
        .unwrap();
    s.context.insert(
        0,
        ContextItem {
            artifact: tiny.clone(),
            action: Action::List,
            pinned: false,
            evicted: false,
            diagnostic: false,
        },
    );
    s.config.context_bytes = 100;
    assert!(context::compact(&mut s, &store, None).is_err());
    assert!(s.context.iter().all(|c| !c.evicted));
    s.config.context_bytes = 18_000;
    let dropped = context::compact(&mut s, &store, None).unwrap();
    assert!(!dropped.is_empty());
    assert!(!s.context[0].evicted);
    assert!(!dropped.contains(&tiny.hash));
    assert!(context::render(&s, &store).unwrap().to_string().len() <= 18_000);
}

#[test]
fn retention_batches_have_only_the_scored_snapshot_and_real_excerpts() {
    let (_root, _home, store, s) = fixture(26, "actual result\nFAIL: boundary case");
    let eligible = context::eligible(&s);
    let mut observed = vec![];
    for indices in eligible.chunks(8) {
        let state = context::excerpts(&s, &store, indices).unwrap();
        let items = state["eligible"].as_array().unwrap();
        assert_eq!(items.len(), indices.len());
        assert_eq!(state["task"], s.task);
        for (item, index) in items.iter().zip(indices) {
            assert_eq!(item["artifact"], s.context[*index].artifact.hash);
            assert!(item["diagnostic_lines"].as_str().unwrap().contains("FAIL"));
            observed.push(item["artifact"].clone());
        }
    }
    assert_eq!(observed.len(), eligible.len());
    observed.sort_by_key(Value::to_string);
    observed.dedup();
    assert_eq!(observed.len(), eligible.len());
}

proptest::proptest! {
    #[test]
    fn serialized_budget_counts_escaped_and_unicode_bytes_exactly(text in "[a-z\"\\\\\n界]{40,120}") {
        let (_root, _home, store, mut s) = fixture(12, &text.repeat(30));
        let original = context::render(&s, &store).unwrap().to_string().len();
        s.config.context_bytes = original * 3 / 4;
        let dropped = context::compact(&mut s, &store, None).unwrap();
        proptest::prop_assert!(!dropped.is_empty());
        proptest::prop_assert!(context::render(&s, &store).unwrap().to_string().len() <= s.config.context_bytes * 65 / 100);
        for id in dropped {
            let expected = store.get(&id).unwrap();
            proptest::prop_assert_eq!(context::rehydrate(&mut s, &id, &store).unwrap(), expected);
        }
    }
}

#[test]
fn evicted_patch_does_not_survive_inside_action_arguments() {
    let (_root, _home, store, mut s) = fixture(5, &"patch result bytes\n".repeat(400));
    let source = "unique-full-file-code".repeat(1000);
    s.context[0].action = Action::Patch {
        edits: vec![Edit {
            path: "game.js".into(),
            before_hash: None,
            content: source.clone(),
        }],
    };
    let artifact = s.context[0].artifact.clone();
    let exact = store.get(&artifact.hash).unwrap();
    let rendered = context::render(&s, &store).unwrap();
    assert!(!rendered.to_string().contains("unique-full-file-code"));
    assert_eq!(
        rendered["evidence"][0]["action"]["files"][0]["after_hash"],
        s1code::session::hash(source.as_bytes())
    );
    let excerpts = context::excerpts(&s, &store, &[0]).unwrap();
    assert!(!excerpts.to_string().contains("unique-full-file-code"));
    s.config.context_bytes = 20_000;
    let evicted = context::compact(&mut s, &store, None).unwrap();
    assert!(evicted.contains(&artifact.hash));
    assert_eq!(store.get(&artifact.hash).unwrap(), exact);
    assert_eq!(
        s.context[0].action,
        Action::Patch {
            edits: vec![Edit {
                path: "game.js".into(),
                before_hash: None,
                content: source
            }]
        }
    );
}

#[test]
fn decision_snapshot_is_focused_and_changes_with_evidence_or_task() {
    let (_root, _home, store, mut s) = fixture(30, &"implementation detail\n".repeat(700));
    let patch = Action::Patch {
        edits: vec![Edit {
            path: "game.js".into(),
            before_hash: None,
            content: "candidate implementation\n".repeat(3000),
        }],
    };
    let c = s1code::policy::candidate(
        patch.clone(),
        "revision",
        "proposal",
        vec![s.context[29].artifact.hash.clone()],
    );
    let view = context::selection_state(&s, &store, std::slice::from_ref(&c)).unwrap();
    assert!(view.to_string().len() < 32_000);
    assert!(view["evidence_scope"]["shown"].as_u64().unwrap() <= 12);
    assert_eq!(view["candidates"][0]["id"], c.id);
    assert!(
        view["candidates"][0]["action"]["files"][0]["proposed_excerpt"]
            .as_str()
            .unwrap()
            .contains("candidate implementation")
    );
    assert!(
        !view
            .to_string()
            .contains(&"candidate implementation\n".repeat(1000))
    );
    s.task = "different task".into();
    assert_ne!(
        view,
        context::selection_state(&s, &store, std::slice::from_ref(&c)).unwrap()
    );
    s.context[0].evicted = true;
    assert_ne!(
        view["snapshot_hash"],
        context::selection_state(&s, &store, &[c]).unwrap()["snapshot_hash"]
    );
    assert_eq!(
        patch,
        s1code::policy::candidate(patch.clone(), "revision", "proposal", vec![]).action
    );
}

#[test]
fn retention_batches_shrink_to_both_limits_without_losing_questions() {
    let (_root, _home, store, mut s) = fixture(20, &"error: diagnostic\n".repeat(150));
    s.config.jev_request_limit = 6500;
    s.config.jev_state_limit = 5500;
    let indices = context::retention_candidates(&s, &store).unwrap();
    let mut offset = 0;
    let mut ids = std::collections::BTreeSet::new();
    while offset < indices.len() {
        let (request, count) = context::retention_batch(&s, &store, &indices[offset..]).unwrap();
        assert!(count < 8);
        s1code::decisions::check_budget(&request, 6500, 5500).unwrap();
        ids.extend(request.questions.into_keys());
        offset += count;
    }
    assert_eq!(ids.len(), indices.len());
    s.prior_user_requests.push("constraint".repeat(1000));
    assert!(context::retention_batch(&s, &store, &indices).is_err());
}

#[test]
fn tiny_results_do_not_need_retention_requests() {
    let (_root, _home, store, s) = fixture(12, "ok");
    assert!(
        context::retention_candidates(&s, &store)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn patch_dependencies_follow_original_file_evidence_not_unrelated_history() {
    let (_root, _home, store, mut s) = fixture(8, &"unrelated output".repeat(100));
    let before = s1code::session::hash(b"old\n");
    let artifact = store
        .put(
            format!("path: main.py\nsha256: {before}\n1: old").as_bytes(),
            "native_tool",
            "read",
            "revision",
            vec![],
        )
        .unwrap();
    s.context.push(ContextItem {
        artifact: artifact.clone(),
        action: Action::Read {
            path: "main.py".into(),
            start: 1,
            lines: 10,
        },
        pinned: false,
        evicted: false,
        diagnostic: false,
    });
    let old_edit = Edit {
        path: "main.py".into(),
        before_hash: Some(before),
        content: "new\n".into(),
    };
    assert_eq!(
        context::patch_dependencies(&s, &store, &[old_edit]).unwrap(),
        vec![artifact.hash]
    );
    let new_edit = Edit {
        path: "other.py".into(),
        before_hash: None,
        content: "new\n".into(),
    };
    assert!(
        context::patch_dependencies(&s, &store, &[new_edit])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn decision_budget_trims_excerpts_but_keeps_task_ids_and_preconditions() {
    use s1code::decisions::{DecisionRequest, Question, fit_selection_request};
    let (_root, _home, store, s) = fixture(20, &"large evidence\n".repeat(700));
    let c = s1code::policy::candidate(
        Action::Read {
            path: "main.py".into(),
            start: 1,
            lines: 80,
        },
        "r",
        "test",
        vec![],
    );
    let state = context::selection_state(&s, &store, std::slice::from_ref(&c)).unwrap();
    let questions = std::collections::BTreeMap::from([(
        "pick".into(),
        Question::Choice {
            instructions: "Choose the best next read".into(),
            criteria: std::collections::BTreeMap::from([(
                c.id.clone(),
                Some("Read main.py".into()),
            )]),
        },
    )]);
    let request = DecisionRequest {
        model: "jev-1.13.0".into(),
        state,
        questions,
    };
    let fitted = fit_selection_request(request, 4500, 4000).unwrap();
    assert_eq!(fitted.state["task"], s.task);
    assert_eq!(fitted.state["candidates"][0]["id"], c.id);
    assert_eq!(fitted.state["candidates"][0]["action"]["path"], "main.py");
    assert!(
        fitted.state["evidence_scope"]["omitted_for_budget"]
            .as_u64()
            .unwrap()
            > 0
    );
    let mut oversized = fitted;
    oversized.state["task"] = "pinned constraint".repeat(1000).into();
    assert!(fit_selection_request(oversized, 4500, 4000).is_err());
}
