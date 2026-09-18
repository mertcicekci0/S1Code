//! Offline fixture validation and explicitly budgeted live trials. Reports are private.
use crate::{
    domain::*,
    engine::{Engine, workspace_for},
    generation::Responses,
    session::{Store, atomic_write, hash, private_dir},
    tools::{Workspace, process},
};
use anyhow::{Context, Result, ensure};
use rand::{SeedableRng, seq::SliceRandom};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
struct Suite {
    split: String,
    tasks: Vec<Fixture>,
}
#[derive(Deserialize)]
struct Fixture {
    id: String,
    task: String,
    files: BTreeMap<String, String>,
    checks: String,
    reference: BTreeMap<String, String>,
}

pub struct EvalOptions {
    pub live: bool,
    pub approve_execution: bool,
    pub request_budget: Option<u64>,
    pub policies: Vec<String>,
    pub repeat: usize,
    pub seed: u64,
    pub model: String,
    pub eviction: String,
    pub context_bytes: usize,
}
async fn hidden_check(
    root: &Path,
    control: &Path,
    checks: &str,
    cancel: &CancellationToken,
) -> Result<bool> {
    let checker = control.join("check.py");
    let script = format!(
        "import sys\nsys.path.insert(0,sys.argv[1])\n{checks}\nprint('NERVE_EVALUATOR_CHECKS_PASSED')\n"
    );
    atomic_write(&checker, script.as_bytes())?;
    let result = process(
        control,
        &[
            "python3".into(),
            "-I".into(),
            checker.to_string_lossy().into(),
            root.to_string_lossy().into(),
        ],
        cancel,
        Duration::from_secs(15),
    )
    .await?;
    Ok(result.exit_code == Some(0) && result.text.contains("NERVE_EVALUATOR_CHECKS_PASSED"))
}

pub async fn evaluate(
    suite_path: &Path,
    output: &Path,
    opts: EvalOptions,
    cancel: &CancellationToken,
) -> Result<Value> {
    ensure!(opts.repeat > 0 && opts.repeat <= 20, "repeat must be 1..20");
    ensure!(
        opts.policies
            .iter()
            .all(|p| ["rules", "jev", "generative"].contains(&p.as_str())),
        "unsupported decision policy"
    );
    if opts.live {
        ensure!(
            opts.approve_execution,
            "live trials require --approve-fixture-execution (disposable trusted fixtures only)"
        );
        ensure!(
            opts.request_budget.is_some_and(|n| n > 0),
            "live trials require --live-budget-requests N: explicit total billable request consent"
        );
        ensure!(
            std::env::var_os("OPENAI_API_KEY").is_some(),
            "OPENAI_API_KEY missing; live validation unverified"
        );
        if opts.policies.iter().any(|p| p == "jev") || opts.eviction == "jev" {
            ensure!(
                std::env::var_os("TYPESAFE_API_KEY").is_some(),
                "TYPESAFE_API_KEY missing; Jev live validation unverified"
            );
        }
    }
    let suite_bytes = std::fs::read(suite_path.join("suite.json"))?;
    let suite: Suite = serde_json::from_slice(&suite_bytes)?;
    let mut order = vec![];
    for repeat in 0..opts.repeat {
        for i in 0..suite.tasks.len() {
            for policy in &opts.policies {
                order.push((repeat, i, policy.clone()));
            }
        }
    }
    order.shuffle(&mut rand::rngs::StdRng::seed_from_u64(opts.seed));
    let mut trials = vec![];
    let mut remaining = opts.request_budget.unwrap_or(0);
    let started = Instant::now();
    for (trial_no, (repeat, index, policy)) in order.iter().enumerate() {
        ensure!(!cancel.is_cancelled(), "evaluation cancelled");
        if opts.live && remaining == 0 {
            break;
        }
        let fixture = &suite.tasks[*index];
        let root = tempfile::tempdir()?;
        let control = tempfile::tempdir()?;
        let home = control.path().join("sessions");
        for (path, content) in &fixture.files {
            ensure!(
                !Path::new(path).is_absolute()
                    && !Path::new(path)
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_))),
                "invalid fixture path"
            );
            let p = root.path().join(path);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, content)?;
        }
        let initial_pass =
            hidden_check(root.path(), control.path(), &fixture.checks, cancel).await?;
        ensure!(
            !initial_pass,
            "fixture {} already passes protected checks",
            fixture.id
        );
        let start = Instant::now();
        let mut metrics = Metrics::default();
        let status;
        let final_pass;
        if opts.live {
            let config = RunConfig {
                decision: policy.clone(),
                generation_model: opts.model.clone(),
                max_provider_requests: remaining.min(24),
                context_bytes: opts.context_bytes,
                eviction: opts.eviction.clone(),
                ..Default::default()
            };
            let (store, s) = Store::create(&home, root.path(), fixture.task.clone(), config)?;
            let generator = Arc::new(Responses::from_env(&opts.model)?);
            let (tx, mut rx) = mpsc::unbounded_channel();
            let (input, inputs) = mpsc::unbounded_channel();
            let engine = Engine {
                workspace: workspace_for(&s)?,
                store,
                session: s,
                generator,
                cancel: cancel.clone(),
                events: tx,
                input: inputs,
                interactive: true,
                approved: None,
            };
            let task = tokio::spawn(engine.run());
            while let Some(event) = rx.recv().await {
                if event.kind == "approval_required" {
                    let id = event.data["candidate"]["id"]
                        .as_str()
                        .context("approval id missing")?;
                    input.send(UiInput::Approve(id.into()))?;
                }
            }
            let result = task.await??;
            metrics = result.metrics;
            remaining =
                remaining.saturating_sub(metrics.generative_calls + metrics.decision_requests);
            status = serde_json::to_value(result.status)?;
            final_pass = hidden_check(root.path(), control.path(), &fixture.checks, cancel).await?;
            // Retain canonical trial trace privately, outside the agent workspace.
            private_dir(output)?;
            let trace_dir = output.join(format!("trial-{trial_no}"));
            private_dir(&trace_dir)?;
            let source = home.join("sessions").join(result.id);
            for entry in std::fs::read_dir(&source)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    std::fs::copy(entry.path(), trace_dir.join(entry.file_name()))?;
                }
            }
        } else {
            // Offline validates the evaluator and reference patch. It is NOT an agent trial.
            let (store, _) =
                Store::create(&home, root.path(), fixture.task.clone(), Default::default())?;
            let w = Workspace::new(root.path(), vec![])?;
            let edits = fixture
                .reference
                .iter()
                .map(|(path, content)| {
                    Ok(Edit {
                        path: path.clone(),
                        before_hash: if root.path().join(path).exists() {
                            Some(hash(&w.bytes(path)?))
                        } else {
                            None
                        },
                        content: content.clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            w.apply(&edits, &store, &w.revision()?)?;
            final_pass = hidden_check(root.path(), control.path(), &fixture.checks, cancel).await?;
            ensure!(
                final_pass,
                "reference patch for {} fails protected checks",
                fixture.id
            );
            status = json!("fixture_validated_not_agent_success");
        }
        trials.push(json!({"task":fixture.id,"repeat":repeat,"trial_order":trial_no,"policy":policy,"mode":if opts.live{"native_live"}else{"OFFLINE_FIXTURE_VALIDATION"},"starting_tree_hash":hash(&serde_json::to_vec(&fixture.files)?),"starting_commit":null,"settings":{"generation_model":opts.model,"eviction":opts.eviction,"context_bytes":opts.context_bytes,"jev_model":"jev-1.13.0","max_provider_requests":24},"initial_protected_checks_passed":initial_pass,"protected_checks_passed":final_pass,"agent_success":if opts.live{Some(final_pass)}else{None},"status":status,"metrics":metrics,"wall_ms":start.elapsed().as_millis()}));
    }
    let source_commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    let report = json!({"format_version":1,"publication":"PRIVATE; provider-restricted measurements require documented clearance","kind":if opts.live{"live_agent_trials"}else{"OFFLINE evaluator validation; not model performance"},"suite_hash":hash(&suite_bytes),"split":suite.split,"source_commit":source_commit,"binary_version":env!("CARGO_PKG_VERSION"),"platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"logical_cpus":std::thread::available_parallelism().ok().map(|n|n.get())},"seed":opts.seed,"sample_count":trials.len(),"wall_ms":started.elapsed().as_millis(),"request_budget_remaining":if opts.live{Some(remaining)}else{None},"cost":null,"trials":trials});
    private_dir(output)?;
    atomic_write(
        &output.join("report.json"),
        &serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}
