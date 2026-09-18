use anyhow::{Result, ensure};
use clap::{Args, Parser, Subcommand};
use nerve::{
    brand, demo,
    domain::*,
    engine::{Engine, workspace_for},
    generation::{Generator, Responses},
    session::{Store, default_home},
};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(name=brand::BIN,version,about="Nerve — bounded coding actions and recoverable context")]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Args, Clone)]
struct RunArgs {
    task: String,
    #[arg(long, default_value = ".")]
    workspace: PathBuf,
    #[arg(long,default_value="native",value_parser=["native","codex"])]
    mode: String,
    #[arg(long,default_value="rules",value_parser=["rules","jev","generative"])]
    decision: String,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, default_value = "jev-1.13.0")]
    jev_model: String,
    #[arg(long, default_value_t = 40)]
    max_steps: usize,
    #[arg(long, default_value_t = 12)]
    max_generations: u64,
    #[arg(long, default_value_t = 24)]
    max_provider_requests: u64,
    #[arg(long, default_value_t = 96_000)]
    context_bytes: usize,
    #[arg(long)]
    exclude: Vec<String>,
    #[arg(long)]
    headless: bool,
    #[arg(long)]
    jev_fallback_rules: bool,
    #[arg(long, default_value_t = 0.5)]
    jev_confidence: f64,
    #[arg(long, default_value_t = 64_000)]
    jev_request_limit: usize,
    #[arg(long, default_value_t = 32_000)]
    jev_state_limit: usize,
    #[arg(long,default_value="conservative",value_parser=["conservative","jev","off"])]
    eviction: String,
}
#[derive(Subcommand)]
enum Commands {
    Run(RunArgs),
    Resume {
        id: String,
        #[arg(long)]
        approve: Option<String>,
        #[arg(long)]
        headless: bool,
        #[arg(long)]
        acknowledge_interruption: bool,
    },
    Sessions,
    Eval {
        #[arg(long, default_value = "fixtures/core")]
        suite: PathBuf,
        #[arg(long, default_value = "eval-results/latest")]
        output: PathBuf,
        #[arg(long)]
        live: bool,
        #[arg(long)]
        approve_fixture_execution: bool,
        #[arg(long)]
        live_budget_requests: Option<u64>,
        #[arg(long, value_delimiter = ',', default_value = "rules")]
        decisions: Vec<String>,
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value = "gpt-4.1-2025-04-14")]
        model: String,
        #[arg(long,default_value="conservative",value_parser=["conservative","jev","off"])]
        eviction: String,
        #[arg(long, default_value_t = 96_000)]
        context_bytes: usize,
    },
    Doctor,
    Login {
        #[arg(value_parser=["codex"])]
        provider: String,
    },
    Logout {
        #[arg(value_parser=["codex"])]
        provider: String,
    },
    Account {
        #[arg(value_parser=["codex"])]
        provider: String,
    },
    Demo {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        offline: bool,
        #[arg(long)]
        headless: bool,
    },
    ContextDemo {
        #[arg(long)]
        workspace: PathBuf,
    },
    Recover {
        id: String,
    },
    Export {
        id: String,
        output: PathBuf,
    },
    Replay {
        trace: PathBuf,
    },
    Delete {
        id: String,
    },
}

#[tokio::main]
async fn main() {
    if let Err(e) = entry().await {
        eprintln!("nerve: {e:#}");
        std::process::exit(1)
    }
}
async fn entry() -> Result<()> {
    let cli = Cli::parse();
    let home = cli.home.unwrap_or(default_home()?);
    let cmd = if let Some(cmd) = cli.command {
        cmd
    } else {
        ensure!(
            io::stdin().is_terminal(),
            "provide a task: nerve run \"fix a bug\" --headless"
        );
        eprint!("Nerve · native / OpenAI Responses · task: ");
        io::stderr().flush()?;
        let mut task = String::new();
        io::stdin().read_line(&mut task)?;
        Commands::Run(RunArgs {
            task: task.trim().into(),
            workspace: ".".into(),
            mode: "native".into(),
            decision: "rules".into(),
            model: None,
            jev_model: "jev-1.13.0".into(),
            max_steps: 40,
            max_generations: 12,
            max_provider_requests: 24,
            context_bytes: 96_000,
            exclude: vec![],
            headless: false,
            jev_fallback_rules: false,
            jev_confidence: 0.5,
            jev_request_limit: 64_000,
            jev_state_limit: 32_000,
            eviction: "conservative".into(),
        })
    };
    match cmd {
        Commands::Run(a) => {
            ensure!(!a.task.trim().is_empty(), "task cannot be empty");
            let config = RunConfig {
                mode: if a.mode == "codex" {
                    Mode::Codex
                } else {
                    Mode::Native
                },
                decision: a.decision,
                generation_model: a.model.unwrap_or_else(|| {
                    if a.mode == "codex" {
                        "codex-default".into()
                    } else {
                        RunConfig::default().generation_model
                    }
                }),
                jev_model: a.jev_model,
                max_steps: a.max_steps,
                max_generations: a.max_generations,
                max_provider_requests: a.max_provider_requests,
                context_bytes: a.context_bytes,
                exclusions: a.exclude,
                jev_fallback_rules: a.jev_fallback_rules,
                jev_confidence: a.jev_confidence,
                jev_request_limit: a.jev_request_limit,
                jev_state_limit: a.jev_state_limit,
                eviction: a.eviction,
                offline_demo: false,
            };
            let generator: Option<Arc<dyn Generator>> = if config.mode == Mode::Native {
                Some(Arc::new(Responses::from_env(&config.generation_model)?))
            } else {
                None
            };
            let (store, s) = Store::create(&home, &a.workspace, a.task, config)?;
            if let Some(generator) = generator {
                drive(store, s, generator, a.headless, None).await?;
            } else {
                drive_bridge(store, s, a.headless).await?;
            }
        }
        Commands::Demo {
            workspace,
            offline,
            headless,
        } => {
            ensure!(
                offline,
                "Demo requires --offline until live provider setup and explicit spend consent; see docs"
            );
            demo::fixture(&workspace)?;
            let config = RunConfig {
                offline_demo: true,
                ..Default::default()
            };
            let (store, s) = Store::create(&home, &workspace, demo::TASK.into(), config)?;
            let generator = Arc::new(demo::OfflineDemo {
                workspace: workspace_for(&s)?,
            });
            drive(store, s, generator, headless, None).await?;
        }
        Commands::Resume {
            id,
            approve,
            headless,
            acknowledge_interruption,
        } => {
            let (store, mut s) = Store::resume(&home, &id)?;
            if acknowledge_interruption {
                ensure!(
                    !store.dir.join("patch-recovery.json").exists(),
                    "run nerve recover first"
                );
                s.inflight = None;
                s.recovery_needed = false;
                s.pending = None;
                s.proposals.clear();
                s.verified = None;
                s.status = RunStatus::Running;
                store.record(
                    &mut s,
                    "interruption_acknowledged",
                    serde_json::json!({"rerun":false}),
                )?;
            }
            if s.config.mode == Mode::Codex {
                drive_bridge(store, s, headless).await?;
                return Ok(());
            }
            let generator: Arc<dyn Generator> = if s.config.offline_demo {
                Arc::new(demo::OfflineDemo {
                    workspace: workspace_for(&s)?,
                })
            } else {
                Arc::new(Responses::from_env(&s.config.generation_model)?)
            };
            drive(store, s, generator, headless, approve).await?;
        }
        Commands::Eval {
            suite,
            output,
            live,
            approve_fixture_execution,
            live_budget_requests,
            decisions,
            repeat,
            seed,
            model,
            eviction,
            context_bytes,
        } => {
            let cancel = CancellationToken::new();
            let c = cancel.clone();
            let signal = tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                c.cancel();
            });
            let report = nerve::evaluation::evaluate(
                &suite,
                &output,
                nerve::evaluation::EvalOptions {
                    live,
                    approve_execution: approve_fixture_execution,
                    request_budget: live_budget_requests,
                    policies: decisions,
                    repeat,
                    seed,
                    model,
                    eviction,
                    context_bytes,
                },
                &cancel,
            )
            .await;
            signal.abort();
            let report = report?;
            println!(
                "{}",
                serde_json::json!({"report":"private report.json written in requested output directory","kind":report["kind"],"sample_count":report["sample_count"]})
            );
        }
        Commands::Sessions => {
            let path = home.join("sessions");
            if path.exists() {
                for entry in std::fs::read_dir(path)? {
                    let p = entry?.path().join("checkpoint.json");
                    if let Ok(bytes) = std::fs::read(p)
                        && let Ok(s) = serde_json::from_slice::<Session>(&bytes)
                    {
                        println!(
                            "{}",
                            serde_json::json!({"id":s.id,"status":s.status,"mode":s.config.mode,"simulation":s.config.offline_demo})
                        );
                    }
                }
            }
        }
        Commands::Login { .. } => {
            account_command("login").await?;
        }
        Commands::Logout { .. } => {
            account_command("logout").await?;
        }
        Commands::Account { .. } => {
            account_command("status").await?;
        }
        Commands::Doctor => {
            nerve::session::private_dir(&home)?;
            let probe = home.join(format!(".probe-{}", uuid::Uuid::new_v4()));
            std::fs::write(&probe, b"ok")?;
            std::fs::remove_file(probe)?;
            let codex = std::process::Command::new("codex")
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
            println!(
                "{}",
                serde_json::json!({"name":brand::NAME,"version":env!("CARGO_PKG_VERSION"),"storage_version":brand::STORAGE_VERSION,"storage_writable":true,"openai_key_present":std::env::var_os("OPENAI_API_KEY").is_some(),"typesafe_key_present":std::env::var_os("TYPESAFE_API_KEY").is_some(),"codex_cli":codex,"native_security":{"os_sandbox":false,"network_isolation":false,"process_groups":cfg!(unix),"exact_approval":true},"supported_platforms":["macOS","Linux"],"telemetry":false})
            );
        }
        Commands::ContextDemo { workspace } => {
            println!("{}", demo::context_demo(&home, &workspace).await?);
        }
        Commands::Recover { id } => {
            let (store, mut s) = Store::resume(&home, &id)?;
            workspace_for(&s)?.recover(&store)?;
            s.inflight = None;
            s.recovery_needed = false;
            s.pending = None;
            s.verified = None;
            s.proposals.clear();
            s.status = RunStatus::Blocked;
            store.record(
                &mut s,
                "patch_recovered",
                serde_json::json!({"restored":true}),
            )?;
            println!("{}", serde_json::json!({"recovered":id}));
        }
        Commands::Export { id, output } => {
            let (store, s) = Store::resume(&home, &id)?;
            store.export(&s, &output)?;
        }
        Commands::Replay { trace } => {
            for line in std::fs::read_to_string(trace)?.lines() {
                let event: serde_json::Value = serde_json::from_str(line)?;
                println!(
                    "{}",
                    serde_json::json!({"playback":"REPLAY — not live execution","record":nerve::privacy::Redactor::environment("").value(&event)})
                );
            }
        }
        Commands::Delete { id } => {
            let (store, _) = Store::resume(&home, &id)?;
            std::fs::remove_dir_all(&store.dir)?;
            println!("{}", serde_json::json!({"deleted":id}));
        }
    }
    Ok(())
}

async fn drive(
    store: Store,
    s: Session,
    generator: Arc<dyn Generator>,
    headless: bool,
    approved: Option<String>,
) -> Result<()> {
    let (tx, rx) = mpsc::unbounded_channel();
    let (input, inputs) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let signal_task = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        signal.cancel();
    });
    let interactive = !headless && io::stdin().is_terminal() && io::stdout().is_terminal();
    let label = format!(
        "native · {} · {}{}",
        s.config.generation_model,
        s.config.decision,
        if s.config.offline_demo {
            " · OFFLINE SIMULATION"
        } else {
            ""
        }
    );
    let engine = Engine {
        workspace: workspace_for(&s)?,
        store,
        session: s,
        generator,
        cancel: cancel.clone(),
        events: tx,
        input: inputs,
        interactive,
        approved,
    };
    let task = tokio::spawn(engine.run());
    present(rx, input, cancel, interactive, label).await?;
    signal_task.abort();
    let result = task.await??;
    if result.status == RunStatus::Failed {
        anyhow::bail!(
            "session {} failed; inspect events and resume after resolving the error",
            result.id
        )
    }
    Ok(())
}
async fn present(
    mut rx: mpsc::UnboundedReceiver<RunEvent>,
    input: mpsc::UnboundedSender<UiInput>,
    cancel: CancellationToken,
    interactive: bool,
    label: String,
) -> Result<()> {
    if interactive {
        let result = nerve::ui::terminal(rx, input, cancel.clone(), label).await;
        if result.is_err() {
            cancel.cancel();
        }
        if let Some(summary) = result? {
            println!(
                "Session {}: {}",
                summary.session,
                serde_json::to_string_pretty(&summary.data)?
            );
        }
    } else {
        while let Some(event) = rx.recv().await {
            println!("{}", serde_json::to_string(&event)?);
        }
    }
    Ok(())
}

async fn account_command(operation: &str) -> Result<()> {
    let cancel = CancellationToken::new();
    let c = cancel.clone();
    let signal = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        c.cancel();
    });
    let result = nerve::bridge::account(operation, &cancel).await;
    signal.abort();
    println!("{}", result?);
    Ok(())
}
async fn drive_bridge(store: Store, s: Session, headless: bool) -> Result<()> {
    let (events, rx) = mpsc::unbounded_channel();
    let (input, inputs) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let c = cancel.clone();
    let signal = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        c.cancel();
    });
    let interactive = !headless && io::stdin().is_terminal() && io::stdout().is_terminal();
    let bridge = nerve::bridge::Bridge {
        store,
        session: s,
        events,
        inputs,
        cancel: cancel.clone(),
        interactive,
    };
    let task = tokio::spawn(bridge.run());
    present(
        rx,
        input,
        cancel,
        interactive,
        "CODEX BRIDGE · delegated · managed ChatGPT auth".into(),
    )
    .await?;
    signal.abort();
    let result = task.await??;
    if result.status == RunStatus::Failed {
        anyhow::bail!("delegated session {} failed", result.id)
    }
    Ok(())
}
