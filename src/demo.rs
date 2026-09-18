//! Explicit simulation for offline harness testing, never live generation.
use crate::{
    domain::*,
    generation::{GenerationResult, Generator},
    session::{hash, private_dir},
    tools::Workspace,
};
use anyhow::{Result, ensure};
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const TASK: &str = "Fix parse_count so it parses whole signed integers, returns zero for blank input, and rejects invalid text. Inspect and run the tests.";
pub fn fixture(root: &Path) -> Result<()> {
    ensure!(
        !root.exists() || std::fs::read_dir(root)?.next().is_none(),
        "demo directory must be empty"
    );
    private_dir(root)?;
    std::fs::write(
        root.join("parser.py"),
        include_str!("../fixtures/demo/parser.py"),
    )?;
    std::fs::write(
        root.join("test_parser.py"),
        include_str!("../fixtures/demo/test_parser.py"),
    )?;
    Ok(())
}

pub struct OfflineDemo {
    pub workspace: Workspace,
}
#[async_trait]
impl Generator for OfflineDemo {
    fn simulated(&self) -> bool {
        true
    }
    async fn generate(
        &self,
        input: Value,
        _: &CancellationToken,
        deltas: mpsc::UnboundedSender<String>,
    ) -> Result<GenerationResult> {
        let evidence = input["evidence"].as_array().cloned().unwrap_or_default();
        let read = evidence
            .iter()
            .any(|e| e["action"]["type"] == "read" && e["action"]["path"] == "parser.py");
        let tested = evidence.iter().any(|e| e["action"]["type"] == "run");
        let test = || Action::Run {
            argv: vec![
                "python3".into(),
                "-m".into(),
                "unittest".into(),
                "-v".into(),
            ],
            verification: true,
        };
        let action = if !tested {
            test()
        } else if !read {
            Action::Read {
                path: "parser.py".into(),
                start: 1,
                lines: 100,
            }
        } else {
            let bytes = self.workspace.bytes("parser.py")?;
            let content = String::from_utf8(bytes.clone())?;
            if content.contains("int(text.strip()[0])") {
                Action::Patch {
                    edits: vec![Edit {
                        path: "parser.py".into(),
                        before_hash: Some(hash(&bytes)),
                        content: content.replace("int(text.strip()[0])", "int(text.strip())"),
                    }],
                }
            } else if input["verification"].is_null() {
                test()
            } else {
                Action::Finish{summary:"OFFLINE SIMULATION: real parser patch applied and actual unittest checks passed. No model inference occurred.".into()}
            }
        };
        let message =
            "OFFLINE SIMULATION — deterministic fixture driver; no provider call.".to_owned();
        let _ = deltas.send(message.clone());
        Ok(GenerationResult {
            proposal: Proposal {
                message,
                actions: vec![action],
            },
            usage: Usage::default(),
            model: "offline_fixture_driver".into(),
        })
    }
}

/// Real read -> eviction -> rehydration, without a generator or scripted decision answers.
pub async fn context_demo(home: &Path, root: &Path) -> Result<serde_json::Value> {
    use crate::{context, session::Store};
    ensure!(
        !root.exists() || std::fs::read_dir(root)?.next().is_none(),
        "context demo directory must be empty"
    );
    private_dir(root)?;
    std::fs::write(
        root.join("SPEC.md"),
        "Pinned task constraint: exact original bytes must be recoverable.\n",
    )?;
    for i in 0..4 {
        std::fs::write(
            root.join(format!("log{i}.txt")),
            (0..80)
                .map(|j| format!("history {i} line {j}: obsolete diagnostic evidence\n"))
                .collect::<String>(),
        )?;
    }
    let (store, mut s) = Store::create(
        home,
        root,
        "Demonstrate real artifact eviction and exact rehydration; no model is involved.".into(),
        RunConfig {
            context_bytes: 12_000,
            ..Default::default()
        },
    )?;
    let w = Workspace::new(root, vec![])?;
    let revision = w.revision()?;
    for name in ["SPEC.md", "log0.txt", "log1.txt", "log2.txt", "log3.txt"] {
        let action = Action::Read {
            path: name.into(),
            start: 1,
            lines: 100,
        };
        let call =
            crate::policy::candidate(action.clone(), &revision, "context demo real read", vec![]);
        let result = w
            .execute(&action, &store, &CancellationToken::new())
            .await?;
        let artifact = store.put(
            result.text.as_bytes(),
            "native_read",
            &call.id,
            &revision,
            vec![],
        )?;
        s.context.push(ContextItem {
            artifact: artifact.clone(),
            action,
            pinned: name == "SPEC.md",
            evicted: false,
            diagnostic: false,
        });
        s.metrics.tool_calls += 1;
        store.record(
            &mut s,
            "tool_result",
            serde_json::json!({"artifact":artifact,"content":result.text}),
        )?;
    }
    let before = context::render(&s, &store)?.to_string().len();
    let evicted = context::compact(&mut s, &store, None)?;
    ensure!(
        !evicted.is_empty(),
        "scenario did not create context pressure"
    );
    let after = context::render(&s, &store)?.to_string().len();
    s.metrics.context_invalidations += 1;
    store.record(&mut s,"context_evicted",serde_json::json!({"policy":"deterministic_conservative","artifacts":evicted,"bytes_before":before,"bytes_after":after}))?;
    let id = evicted[0].clone();
    let expected = store.get(&id)?;
    let restored = context::rehydrate(&mut s, &id, &store)?;
    ensure!(restored == expected, "rehydration mismatch");
    store.record(
        &mut s,
        "rehydrated",
        serde_json::json!({"artifact":id,"exact_bytes":restored.len(),"rerun":false}),
    )?;
    s.status = RunStatus::Completed;
    store.save(&s)?;
    Ok(
        serde_json::json!({"demo":"REAL CONTEXT MECHANICS — no model decisions","session":s.id,"evicted_artifacts":evicted.len(),"before_bytes":before,"after_bytes":after,"rehydrated_exactly":true,"tool_calls":s.metrics.tool_calls,"rehydrations":s.metrics.rehydrations}),
    )
}
