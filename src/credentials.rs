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
    static PRESENCE: std::sync::OnceLock<Mutex<std::collections::BTreeMap<String, bool>>> =
        std::sync::OnceLock::new();
    let mut presence = PRESENCE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *presence
        .entry(variable.into())
        .or_insert_with(|| lookup(variable, false).is_ok())
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
    let output = bounded_output(command, std::time::Duration::from_secs(15))?;
    if !output.status.success() {
        bail!(
            "{variable} unavailable: no accessible saved Keychain entry; unlock/allow Keychain access or set the environment variable"
        );
    }
    Ok(if secret { output.stdout } else { Vec::new() })
}

fn bounded_output(
    mut command: std::process::Command,
    timeout: std::time::Duration,
) -> Result<std::process::Output> {
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());
    let mut child = command.spawn().context("Cannot access macOS Keychain")?;
    let started = std::time::Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child
                .wait_with_output()
                .context("Cannot read Keychain result");
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "Keychain access timed out waiting for macOS permission. Allow access when prompted, then reopen S1Code. No provider request was sent"
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn credential_prompt_cannot_hang_startup_indefinitely() {
        let mut command = std::process::Command::new("/bin/sleep");
        command.arg("30");
        let start = std::time::Instant::now();
        let error = bounded_output(command, std::time::Duration::from_millis(30)).unwrap_err();
        assert!(error.to_string().contains("Keychain access timed out"));
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }
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
