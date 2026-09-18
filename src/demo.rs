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
