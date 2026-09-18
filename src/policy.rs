use crate::{brand, domain::*, session::hash};
use anyhow::{Result, ensure};

pub fn classify(action: &Action) -> PolicyClass {
    match action {
        Action::Run { argv, .. } if !valid_command(argv) => PolicyClass::Deny,
        Action::Run { .. } | Action::Patch { .. } => PolicyClass::Ask,
        _ => PolicyClass::Allow,
    }
}

/// These commands still execute repository code and always require approval.
pub fn valid_command(argv: &[String]) -> bool {
    if argv.iter().any(|s| s.contains('\0') || s.len() > 1024) {
        return false;
    }
    let a: Vec<&str> = argv.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["cargo", kind, rest @ ..] if ["test", "check"].contains(kind) => {
            rest.contains(&"--offline")
                && rest.iter().all(|a| {
                    ["--offline", "--locked", "--all-targets", "--lib", "--quiet"].contains(a)
                })
        }
        ["python3", "-m", "unittest", rest @ ..] => {
            rest.iter().all(|a| ["discover", "-v", "-q"].contains(a))
        }
        _ => false,
    }
}

pub fn candidate(
    action: Action,
    revision: &str,
    provenance: &str,
    evidence: Vec<String>,
) -> CandidateAction {
    let class = classify(&action);
    let id = hash(
        &serde_json::to_vec(&(brand::POLICY_VERSION, &action, revision))
            .expect("serializable action"),
    );
    CandidateAction {
        id,
        action,
        provenance: provenance.into(),
        evidence,
        revision: revision.into(),
        policy_version: brand::POLICY_VERSION.into(),
        class,
    }
}

pub fn revalidate(c: &CandidateAction, revision: &str) -> Result<()> {
    ensure!(
        c.revision == revision,
        "stale candidate: workspace changed; approval invalidated"
    );
    ensure!(c.policy_version == brand::POLICY_VERSION, "policy changed");
    ensure!(
        classify(&c.action) != PolicyClass::Deny,
        "action denied by deterministic policy"
    );
    ensure!(
        c.class == classify(&c.action),
        "candidate policy classification was altered"
    );
    ensure!(
        c.id == candidate(c.action.clone(), revision, "", vec![]).id,
        "candidate identity mismatch"
    );
    Ok(())
}
