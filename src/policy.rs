use crate::{brand, domain::*, session::hash};
use anyhow::{Result, ensure};
use std::path::{Component, Path};

pub fn classify(action: &Action) -> PolicyClass {
    match action {
        Action::Replace { .. } => PolicyClass::Deny, // Must first become a validated exact patch.
        Action::Run { argv, .. } if !valid_command(argv) => PolicyClass::Deny,
        Action::Run { .. } | Action::Patch { .. } => PolicyClass::Ask,
        _ => PolicyClass::Allow,
    }
}

/// These commands execute repository code and require manual or explicit session consent.
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
        ["python3", "-m", "unittest", rest @ ..] => valid_unittest(rest),
        ["node", "--test"] => true,
        ["python3", "-m", "pytest", rest @ ..] => rest
            .iter()
            .all(|arg| ["-q", "-v", "--disable-warnings"].contains(arg)),
        ["npm", "--offline", "run", script] => {
            ["test", "build", "lint", "typecheck"].contains(script)
        }
        _ => false,
    }
}

fn valid_unittest(args: &[&str]) -> bool {
    if args.is_empty() {
        return true;
    }
    let mut at = usize::from(args.first() == Some(&"discover"));
    let mut has_start_directory = false;
    if at == 0 && args.contains(&"discover") {
        return false;
    }
    while at < args.len() {
        match args[at] {
            "-v" | "-q" => at += 1,
            "-s" if args.first() == Some(&"discover") && at + 1 < args.len() => {
                let path = Path::new(args[at + 1]);
                if has_start_directory
                    || args[at + 1].is_empty()
                    || path.components().count() > 16
                    || !path.components().all(|component| match component {
                        Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
                        _ => false,
                    })
                {
                    return false;
                }
                has_start_directory = true;
                at += 2;
            }
            _ => return false,
        }
    }
    true
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_tests_require_approval_and_cannot_add_execution_flags() {
        let action = Action::Run {
            argv: vec!["node".into(), "--test".into()],
            verification: true,
        };
        assert_eq!(classify(&action), PolicyClass::Ask);
        for argv in [
            vec!["node"],
            vec!["node", "-e", "process.exit(0)"],
            vec!["node", "--test", "--import", "module"],
            vec!["node", "--test", "../outside.js"],
            vec!["npm", "test"],
        ] {
            assert_eq!(
                classify(&Action::Run {
                    argv: argv.into_iter().map(String::from).collect(),
                    verification: true
                }),
                PolicyClass::Deny
            );
        }
    }

    #[test]
    fn unittest_discovery_allows_a_bounded_start_directory_only() {
        assert!(valid_command(&[
            "python3".into(),
            "-m".into(),
            "unittest".into(),
            "discover".into(),
            "-s".into(),
            "tests".into(),
            "-v".into(),
        ]));
        for argv in [
            vec!["python3", "-m", "unittest", "discover", "-s", "../tests"],
            vec!["python3", "-m", "unittest", "discover", "-s", "/tmp/tests"],
            vec!["python3", "-m", "unittest", "discover", "-s"],
            vec!["python3", "-m", "unittest", "-s", "tests"],
            vec!["python3", "-m", "unittest", "discover", "--locals"],
            vec![
                "python3", "-m", "unittest", "discover", "-s", "tests", "-s", "other",
            ],
        ] {
            assert!(!valid_command(
                &argv.into_iter().map(String::from).collect::<Vec<_>>()
            ));
        }
    }
    #[test]
    fn project_checks_do_not_allow_installation_or_arbitrary_scripts() {
        for argv in [
            vec!["npm", "--offline", "run", "test"],
            vec!["npm", "--offline", "run", "build"],
            vec!["python3", "-m", "pytest", "-q"],
        ] {
            let action = Action::Run {
                argv: argv.into_iter().map(String::from).collect(),
                verification: true,
            };
            assert_eq!(classify(&action), PolicyClass::Ask);
        }
        for argv in [
            vec!["npm", "install"],
            vec!["npm", "run", "test"],
            vec!["npm", "--offline", "run", "deploy"],
            vec!["npm", "--offline", "run", "test", "--", "--update"],
            vec!["python3", "-m", "pytest", "--override-ini", "secret"],
            vec!["python3", "-m", "pytest", "../outside"],
        ] {
            assert!(!valid_command(
                &argv.into_iter().map(String::from).collect::<Vec<_>>()
            ));
        }
    }
}
