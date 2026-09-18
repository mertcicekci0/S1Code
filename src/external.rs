//! Explicit terminal handoff, not an embedded agent or subscription-token adapter.
use anyhow::{Context, Result, ensure};
use std::{
    io::{self, IsTerminal},
    path::Path,
    process::Stdio,
};

pub async fn claude_code(workspace: &Path) -> Result<()> {
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "Claude Code handoff requires an interactive terminal"
    );
    let workspace = workspace
        .canonicalize()
        .context("workspace does not exist")?;
    ensure!(workspace.is_dir(), "workspace must be a directory");
    eprintln!(
        "Opening official Claude Code. Claude Code owns authentication, permissions, execution and history. S1Code/Jev are not controlling this session. Exit Claude Code to return."
    );
    let mut command = tokio::process::Command::new("claude");
    crate::tools::clean_environment(&mut command);
    // Preserve terminal rendering, never pass other providers' credentials.
    for key in [
        "TERM",
        "COLORTERM",
        "TERM_PROGRAM",
        "LC_CTYPE",
        "ANTHROPIC_API_KEY",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .current_dir(workspace)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // Share the foreground terminal, but let the official child handle Ctrl-C
    // without terminating the surrounding task-entry application.
    let interrupt = tokio::spawn(async { while tokio::signal::ctrl_c().await.is_ok() {} });
    let result = command.status().await;
    interrupt.abort();
    let status = result.context(
        "Cannot start official Claude Code; install it using Anthropic's documented instructions",
    )?;
    ensure!(status.success(), "Claude Code exited with {status}");
    Ok(())
}
