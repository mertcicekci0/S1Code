//! Read existing credentials without exporting them to child-process environments.
use anyhow::{Context, Result, bail, ensure};
use std::sync::{Mutex, OnceLock};

static LOADED: Mutex<Vec<String>> = Mutex::new(Vec::new());
static PRESENCE: OnceLock<Mutex<std::collections::BTreeMap<String, bool>>> = OnceLock::new();
pub const SERVICE: &str = "s1code.credentials.v1";

pub fn variable(provider: &str) -> Result<&'static str> {
    match provider {
        "openai" => Ok("OPENAI_API_KEY"),
        "claude" => Ok("ANTHROPIC_API_KEY"),
        "typesafe" => Ok("TYPESAFE_API_KEY"),
        "openrouter" => Ok("OPENROUTER_API_KEY"),
        _ => bail!(
            "Use openai, claude, typesafe or openrouter for API keys; login codex manages ChatGPT authentication"
        ),
    }
}

fn account(variable: &str) -> Option<&'static str> {
    match variable {
        "OPENAI_API_KEY" => Some("openai"),
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
    if std::env::var("S1CODE_KEYCHAIN").as_deref() == Ok("off") {
        return false;
    }
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
    if std::env::var("S1CODE_KEYCHAIN").as_deref() == Ok("off") {
        bail!("Saved credential lookup disabled; set {variable} explicitly");
    }
    let account = account(variable).context(
        "No saved credential support for this provider; set its API key environment variable",
    )?;
    if !cfg!(target_os = "macos") {
        bail!("Saved credentials require macOS Keychain; set {variable}");
    }
    let mut command = std::process::Command::new("/usr/bin/security");
    command
        .env_clear()
        .args(["find-generic-password", "-s", SERVICE, "-a", account]);
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

#[cfg(any(target_os = "macos", test))]
fn validate_secret(provider: &str, value: &str) -> Result<()> {
    variable(provider)?;
    ensure!(
        (16..=4096).contains(&value.len())
            && !value.chars().any(char::is_whitespace)
            && !value.chars().any(char::is_control),
        "Enter the complete API secret without whitespace; no key was saved"
    );
    let looks_wrong = match provider {
        "claude" => !value.starts_with("sk-ant-"),
        "typesafe" => value.starts_with("sk-"),
        "openrouter" => !value.starts_with("sk-or-"),
        "openai" => {
            value.starts_with("sk-ant-")
                || value.starts_with("sk-or-")
                || value.starts_with("apikey_")
        }
        _ => true,
    };
    ensure!(
        !looks_wrong,
        "This key appears to belong to a different provider; no key was saved"
    );
    Ok(())
}

/// Explicit foreground credential management, never part of a model/tool action.
pub fn manage(operation: &str, provider: &str) -> Result<()> {
    variable(provider)?;
    ensure!(
        matches!(operation, "set" | "remove"),
        "Use auth set or auth remove"
    );
    ensure!(
        std::env::var("S1CODE_KEYCHAIN").as_deref() != Ok("off"),
        "Keychain is disabled by S1CODE_KEYCHAIN=off; no credential was changed"
    );
    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::{delete_generic_password, set_generic_password};
        use std::io::IsTerminal;
        if operation == "set" {
            ensure!(
                std::io::stdin().is_terminal(),
                "API secrets must be entered at the hidden terminal prompt, not as arguments or redirected input"
            );
            let secret = rpassword::prompt_password(format!(
                "{provider} API secret (hidden; stored in macOS Keychain): "
            ))
            .context("Could not read the hidden API secret")?;
            validate_secret(provider, &secret)?;
            set_generic_password(SERVICE, provider, secret.as_bytes())
                .map_err(|_| anyhow::anyhow!("Could not save to macOS Keychain. Unlock it or allow access, then retry. No plaintext fallback was used."))?;
            LOADED
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(secret);
        } else if let Err(error) = delete_generic_password(SERVICE, provider) {
            // Removing an already absent credential is safe and idempotent.
            ensure!(
                error.code() == -25300,
                "Could not remove the saved Keychain entry; unlock or allow Keychain access, then retry"
            );
        }
        if let Some(presence) = PRESENCE.get() {
            presence.lock().unwrap_or_else(|e| e.into_inner()).clear();
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    bail!(
        "Persistent API-key entry currently requires macOS Keychain. Configure {} through your environment or secret manager on this platform; no file was written.",
        variable(provider)?
    )
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
        assert_eq!(account("OPENAI_API_KEY"), Some("openai"));
        assert_eq!(account("ANTHROPIC_API_KEY"), Some("claude"));
        assert_eq!(account("TYPESAFE_API_KEY"), Some("typesafe"));
        assert_eq!(account("OPENROUTER_API_KEY"), Some("openrouter"));
        assert_eq!(account("UNTRUSTED"), None);
    }
    #[test]
    fn credential_entry_rejects_cross_provider_keys_without_echoing_them() {
        for (provider, prefix) in [
            ("claude", "apikey_"),
            ("typesafe", "sk-ant-"),
            ("openai", "sk-or-"),
            ("openrouter", "sk-ant-"),
        ] {
            let value = format!("{prefix}{}", "placeholder".repeat(3));
            let error = validate_secret(provider, &value).unwrap_err().to_string();
            assert!(!error.contains(&value));
        }
        for (provider, prefix) in [
            ("claude", "sk-ant-"),
            ("typesafe", "apikey_"),
            ("openai", "sk-"),
            ("openrouter", "sk-or-"),
        ] {
            let value = format!("{prefix}{}", "placeholder".repeat(3));
            assert!(validate_secret(provider, &value).is_ok());
            assert!(validate_secret(provider, &format!("{value}\n")).is_err());
        }
    }
}
