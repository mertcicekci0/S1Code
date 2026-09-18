//! Offline fixture validation and explicitly budgeted live trials. Reports are private.
use crate::{
    domain::*,
    engine::{Engine, workspace_for},
    generation,
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
    pub generation_provider: String,
    pub eviction: String,
    pub context_bytes: usize,
    pub jev_provider: String,
    pub jev_resolved_model: Option<String>,
}

fn retain_trace(source: &Path, destination: &Path) -> Result<()> {
    private_dir(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            retain_trace(&entry.path(), &target)?;
        } else if kind.is_file() {
            atomic_write(&target, &std::fs::read(entry.path())?)?;
        } else {
            anyhow::bail!("unexpected non-file in private session trace");
        }
    }
    Ok(())
}
async fn hidden_check(
    root: &Path,
    control: &Path,
    checks: &str,
    cancel: &CancellationToken,
) -> Result<bool> {
    let checker = control.join("check.py");
    let script = format!(
        "import sys\nsys.path.insert(0,sys.argv[1])\n{checks}\nprint('S1CODE_EVALUATOR_CHECKS_PASSED')\n"
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
    Ok(result.exit_code == Some(0) && result.text.contains("S1CODE_EVALUATOR_CHECKS_PASSED"))
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
            std::env::var_os(if opts.generation_provider == "claude" {
                "ANTHROPIC_API_KEY"
            } else {
                "OPENAI_API_KEY"
            })
            .is_some(),
            "selected generation provider API key missing; live validation unverified"
        );
        if opts.policies.iter().any(|p| p == "jev") || opts.eviction == "jev" {
            ensure!(
                std::env::var_os(if opts.jev_provider == "openrouter" {
                    "OPENROUTER_API_KEY"
                } else {
                    "TYPESAFE_API_KEY"
                })
                .is_some(),
                "selected Jev provider key missing; live validation unverified"
            );
        }
    }
    private_dir(output)?;
    if !output.join(".gitignore").exists() {
        atomic_write(&output.join(".gitignore"), b"*\n")?;
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
    let per_trial_limit = remaining.min(24);
    let started = Instant::now();
    for (trial_no, (repeat, index, policy)) in order.iter().enumerate() {
        ensure!(!cancel.is_cancelled(), "evaluation cancelled");
        if opts.live && remaining < per_trial_limit {
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
                generation_provider: opts.generation_provider.clone(),
                max_provider_requests: per_trial_limit,
                context_bytes: opts.context_bytes,
                eviction: opts.eviction.clone(),
                jev_provider: opts.jev_provider.clone(),
                jev_model: if opts.jev_provider == "openrouter" {
                    crate::decisions::OPENROUTER_MODEL.into()
                } else {
                    "jev-1.13.0".into()
                },
                jev_resolved_model: opts.jev_resolved_model.clone(),
                ..Default::default()
            };
            let generator = generation::from_config(&config)?;
            let (store, s) = Store::create(&home, root.path(), fixture.task.clone(), config)?;
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
            retain_trace(&source, &trace_dir)?;
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
        trials.push(json!({"task":fixture.id,"repeat":repeat,"trial_order":trial_no,"policy":policy,"mode":if opts.live{"native_live"}else{"OFFLINE_FIXTURE_VALIDATION"},"starting_tree_hash":hash(&serde_json::to_vec(&fixture.files)?),"starting_commit":null,"settings":{"generation_model":opts.model,"generation_provider":opts.generation_provider,"eviction":opts.eviction,"context_bytes":opts.context_bytes,"jev_provider":opts.jev_provider,"jev_model":if opts.jev_provider=="openrouter"{crate::decisions::OPENROUTER_MODEL}else{"jev-1.13.0"},"jev_resolved_model":if opts.jev_provider=="openrouter"{Some(opts.jev_resolved_model.as_deref().unwrap_or(crate::decisions::OPENROUTER_RESOLVED))}else{None},"max_provider_requests":per_trial_limit},"initial_protected_checks_passed":initial_pass,"protected_checks_passed":final_pass,"agent_success":if opts.live{Some(final_pass)}else{None},"status":status,"metrics":metrics,"wall_ms":start.elapsed().as_millis()}));
    }
    let mut git = tokio::process::Command::new("git");
    crate::tools::clean_environment(&mut git);
    let source_commit = git
        .args(["rev-parse", "HEAD"])
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    let mut git = tokio::process::Command::new("git");
    crate::tools::clean_environment(&mut git);
    let source_tree_dirty = git
        .args(["status", "--porcelain"])
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty());
    let report = json!({"format_version":1,"publication":"PRIVATE; provider-restricted measurements require documented clearance","kind":if opts.live{"live_agent_trials"}else{"OFFLINE evaluator validation; not model performance"},"suite_hash":hash(&suite_bytes),"split":suite.split,"source_commit":source_commit,"source_commit_scope":"invocation checkout; installed binary source must be recorded separately","source_tree_dirty":source_tree_dirty,"binary_version":env!("CARGO_PKG_VERSION"),"platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"logical_cpus":std::thread::available_parallelism().ok().map(|n|n.get())},"seed":opts.seed,"sample_count":trials.len(),"wall_ms":started.elapsed().as_millis(),"request_budget_remaining":if opts.live{Some(remaining)}else{None},"cost":null,"trials":trials});
    private_dir(output)?;
    atomic_write(
        &output.join("report.json"),
        &serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}
