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
