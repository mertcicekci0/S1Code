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

/// Authentication stays in the official client. Never return its account identity.
fn auth_command(operation: &str) -> Result<tokio::process::Command> {
    ensure!(
        matches!(operation, "login" | "logout" | "status"),
        "Unsupported account operation"
    );
    let mut command = tokio::process::Command::new("claude");
    crate::tools::clean_environment(&mut command);
    command.args(["auth", operation]).kill_on_drop(true);
    if operation == "login" {
        command.arg("--claudeai");
    }
    if operation == "status" {
        command.arg("--json");
    }
    Ok(command)
}

pub async fn claude_auth(operation: &str) -> Result<()> {
    ensure!(
        matches!(operation, "login" | "logout"),
        "Use login or logout"
    );
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "Claude account changes require an interactive terminal"
    );
    eprintln!(
        "Official Claude Code manages this account. Subscription login does not authenticate native S1Code API generation or Jev."
    );
    let status = auth_command(operation)?
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .context("Cannot start Claude Code; install the official CLI first")?;
    ensure!(
        status.success(),
        "Claude Code account command failed; use claude auth --help to check compatibility"
    );
    Ok(())
}

fn public_account(value: &serde_json::Value) -> Result<serde_json::Value> {
    let logged_in = value["loggedIn"]
        .as_bool()
        .context("Unsupported Claude Code auth status; update the official CLI")?;
    let method = value["authMethod"].as_str().unwrap_or("unknown");
    // Unknown strings are not forwarded: even error/account metadata can contain identity.
    let method = match method {
        "claude.ai" | "api_key" | "oauth_token" | "bedrock" | "vertex" | "foundry" | "none" => {
            method
        }
        _ => "unknown",
    };
    Ok(
        serde_json::json!({"provider":"claude-code", "logged_in":logged_in, "auth_method":method, "credential_owner":"official Claude Code", "native_api_access":"not checked by this command", "jev_included":false}),
    )
}

pub async fn claude_account_status() -> Result<serde_json::Value> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        auth_command("status")?.stdin(Stdio::null()).output(),
    )
    .await
    .context("Claude account status timed out")?
    .context("Cannot start Claude Code; install the official CLI first")?;
    ensure!(
        output.stdout.len() <= 64 * 1024,
        "Claude account response too large"
    );
    let value = serde_json::from_slice(&output.stdout)
        .context("Claude account status unavailable; run claude auth status directly")?;
    public_account(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_identity_and_unknown_values_never_escape() {
        let value = public_account(&serde_json::json!({"loggedIn":true,"authMethod":"claude.ai","email":"private@example.invalid","access_token":"fixture"})).unwrap();
        let text = value.to_string();
        assert!(!text.contains("private@") && !text.contains("fixture"));
        assert_eq!(value["logged_in"], true);
        assert_eq!(
            public_account(&serde_json::json!({"loggedIn":false,"authMethod":"personal-value"}))
                .unwrap()["auth_method"],
            "unknown"
        );
        assert!(public_account(&serde_json::json!({"email":"private"})).is_err());
    }
    #[test]
    fn auth_uses_official_cli_without_native_credentials() {
        let command = auth_command("login").unwrap();
        let args: Vec<_> = command.as_std().get_args().collect();
        assert_eq!(args, ["auth", "login", "--claudeai"]);
        assert!(
            !command
                .as_std()
                .get_envs()
                .any(|(name, _)| name == "ANTHROPIC_API_KEY" || name == "TYPESAFE_API_KEY")
        );
        assert!(auth_command("setup-token").is_err());
    }
}
