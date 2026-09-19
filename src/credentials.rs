//! Read existing credentials without exporting them to child-process environments.
use anyhow::{Context, Result, bail};
use std::sync::Mutex;

static LOADED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn account(variable: &str) -> Option<&'static str> {
    match variable {
        "ANTHROPIC_API_KEY" => Some("claude"),
        "TYPESAFE_API_KEY" => Some("typesafe"),
        "OPENROUTER_API_KEY" => Some("openrouter"),
        _ => None,
    }
}

pub fn get(variable: &str) -> Result<String> {
    let value = resolve(std::env::var(variable).ok(), || stored(variable))?;
    let mut loaded = LOADED.lock().unwrap_or_else(|e| e.into_inner());
    if !loaded.contains(&value) {
        loaded.push(value.clone());
    }
    Ok(value)
}

fn resolve(environment: Option<String>, saved: impl FnOnce() -> Result<String>) -> Result<String> {
    match environment.filter(|v| !v.trim().is_empty()) {
        Some(value) => Ok(value),
        None => saved(),
    }
}

pub fn loaded_secrets() -> Vec<String> {
    LOADED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn present(variable: &str) -> bool {
    if std::env::var(variable).is_ok_and(|v| !v.trim().is_empty()) {
        return true;
    }
    lookup(variable, false).is_ok()
}

fn stored(variable: &str) -> Result<String> {
    let bytes = lookup(variable, true)?;
    let value = String::from_utf8(bytes).context("Stored credential is not UTF-8")?;
    let value = value.trim_end_matches(['\r', '\n']).to_owned();
    if value.is_empty() {
        bail!("Stored credential is empty");
    }
    Ok(value)
}

fn lookup(variable: &str, secret: bool) -> Result<Vec<u8>> {
    let account = account(variable).context(
        "No saved credential support for this provider; set its API key environment variable",
    )?;
    if !cfg!(target_os = "macos") {
        bail!("Saved credentials require macOS Keychain; set {variable}");
    }
    let mut command = std::process::Command::new("/usr/bin/security");
    command.env_clear().args([
        "find-generic-password",
        "-s",
        "s1code.credentials.v1",
        "-a",
        account,
    ]);
    if secret {
        command.arg("-w");
    }
    let output = command.output().context("Cannot access macOS Keychain")?;
    if !output.status.success() {
        bail!(
            "{variable} unavailable: no accessible saved Keychain entry; unlock/allow Keychain access or set the environment variable"
        );
    }
    Ok(if secret { output.stdout } else { Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_wins_without_touching_keychain() {
        assert_eq!(
            resolve(Some("explicit".into()), || panic!("must not read store")).unwrap(),
            "explicit"
        );
        assert_eq!(
            resolve(Some(" ".into()), || Ok("saved".into())).unwrap(),
            "saved"
        );
        assert!(resolve(None, || bail!("locked")).is_err());
    }
    #[test]
    fn provider_accounts_are_separate() {
        assert_eq!(account("ANTHROPIC_API_KEY"), Some("claude"));
        assert_eq!(account("TYPESAFE_API_KEY"), Some("typesafe"));
        assert_eq!(account("OPENROUTER_API_KEY"), Some("openrouter"));
        assert_eq!(account("UNTRUSTED"), None);
    }
}
