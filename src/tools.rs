use crate::{
    domain::*,
    session::{Store, atomic_write, hash},
};
use anyhow::{Context, Result, bail, ensure};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;

pub const MAX_FILE: usize = 512 * 1024;
pub const MAX_OUTPUT: usize = 64 * 1024;
pub const MAX_FILES: usize = 10_000;

#[derive(Clone)]
pub struct Workspace {
    pub root: PathBuf,
    exclusions: Vec<String>,
}
#[derive(Debug)]
pub struct ToolResult {
    pub text: String,
    pub exit_code: Option<i32>,
    pub diagnostic: bool,
}

impl Workspace {
    pub fn new(root: &Path, exclusions: Vec<String>) -> Result<Self> {
        Ok(Self {
            root: root.canonicalize()?,
            exclusions,
        })
    }
    fn allowed_name(&self, relative: &Path) -> bool {
        let s = relative.to_string_lossy();
        if self
            .exclusions
            .iter()
            .any(|x| s == *x || s.starts_with(&format!("{x}/")))
        {
            return false;
        }
        relative.components().count() <= 32
            && relative.components().all(|c| match c {
                Component::Normal(x) => {
                    let n = x.to_string_lossy().to_lowercase();
                    !n.starts_with('.')
                        && ![
                            "target",
                            "node_modules",
                            "__pycache__",
                            "credentials",
                            "secrets",
                            "private",
                            "eval-results",
                            "id_rsa",
                            "id_ed25519",
                        ]
                        .contains(&n.as_str())
                        && !n.ends_with(".pem")
                        && !n.ends_with(".key")
                        && !n.ends_with(".env")
                }
                _ => false,
            })
    }
    pub fn path(&self, relative: &str, new: bool) -> Result<PathBuf> {
        let at = self.checked_path(relative, new)?;
        if !new {
            ensure!(
                self.files()?.contains(&relative.to_owned()),
                "ignored or unsupported file"
            );
        } else {
            ensure!(!self.ignored(relative)?, "new file matches ignore rules");
        }
        Ok(at)
    }
    // Check each ancestor even when the name came from a bounded discovery pass.
    // Discovery membership is reusable within one read action, never across actions.
    fn checked_path(&self, relative: &str, new: bool) -> Result<PathBuf> {
        let path = Path::new(relative);
        ensure!(
            !relative.is_empty() && self.allowed_name(path),
            "path is excluded, sensitive, or outside the workspace"
        );
        let mut at = self.root.clone();
        for c in path.components() {
            at.push(c);
            match fs::symlink_metadata(&at) {
                Ok(m) => {
                    ensure!(!m.file_type().is_symlink(), "symlinks are unsupported");
                    if at != self.root.join(path) {
                        ensure!(m.is_dir(), "path parent is not a directory");
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && new => {}
                Err(e) => return Err(e.into()),
            }
        }
        ensure!(at.starts_with(&self.root), "path escaped root");
        Ok(at)
    }
    fn ignored(&self, path: &str) -> Result<bool> {
        self.ignored_path(path, false)
    }
    fn ignored_path(&self, path: &str, is_dir: bool) -> Result<bool> {
        let mut builder = ignore::gitignore::GitignoreBuilder::new(&self.root);
        let mut dir = Some(
            self.root
                .join(path)
                .parent()
                .context("parent")?
                .to_path_buf(),
        );
        let mut parents = vec![];
        while let Some(d) = dir {
            if !d.starts_with(&self.root) {
                break;
            }
            parents.push(d.clone());
            dir = d.parent().map(Path::to_path_buf);
        }
        for d in parents.into_iter().rev() {
            for name in [".gitignore", ".ignore"] {
                let f = d.join(name);
                if f.exists()
                    && let Some(e) = builder.add(f)
                {
                    return Err(e.into());
                }
            }
        }
        Ok(builder
            .build()?
            .matched_path_or_any_parents(self.root.join(path), is_dir)
            .is_ignore())
    }
    /// Validate explicit command inputs. Repository code itself is not sandboxed.
    pub fn validate_command(&self, argv: &[String]) -> Result<()> {
        ensure!(
            crate::policy::valid_command(argv),
            "unsupported command; use npm --offline run test|build|lint|typecheck, python3 -m pytest [-q|-v|--disable-warnings], node --test, python3 -m unittest [-v|-q], python3 -m unittest discover [-s DIRECTORY] [-v|-q], or cargo test/check --offline [--locked] [--all-targets|--lib] [--quiet]; shell strings and additional flags are unsupported"
        );
        if argv.first().is_some_and(|arg| arg == "npm") {
            let bytes = self.bytes("package.json")?;
            let package: serde_json::Value = serde_json::from_slice(&bytes)
                .context("package.json must be valid JSON before executing a script")?;
            ensure!(
                package["scripts"][&argv[3]]
                    .as_str()
                    .is_some_and(|script| !script.trim().is_empty()),
                "Requested script is missing or empty in package.json; inspect scripts before selecting a check"
            );
        }
        // The command grammar has one path argument: unittest -s.
        if let Some(at) = argv.iter().position(|arg| arg == "-s") {
            let relative = &argv[at + 1]; // Proven present by the command grammar.
            let path = self.path(relative, true)?;
            ensure!(
                path.is_dir(),
                "test start directory does not exist or is not a directory"
            );
            ensure!(
                !self.ignored_path(relative, true)?,
                "test start directory matches ignore rules"
            );
        }
        Ok(())
    }
    pub fn files(&self) -> Result<Vec<String>> {
        let mut out = vec![];
        let guard = self.clone();
        for item in WalkBuilder::new(&self.root)
            .filter_entry(move |entry| {
                entry.depth() == 0
                    || entry
                        .path()
                        .strip_prefix(&guard.root)
                        .is_ok_and(|path| guard.allowed_name(path))
            })
            .max_depth(Some(32))
            .hidden(true)
            .follow_links(false)
            .require_git(false)
            .build()
        {
            let item = item?;
            if item.file_type().is_some_and(|t| t.is_file()) {
                let p = item.path().strip_prefix(&self.root)?;
                if self.allowed_name(p) {
                    out.push(p.to_string_lossy().into_owned());
                    ensure!(
                        out.len() <= MAX_FILES,
                        "repository exceeds file limit; narrow the workspace"
                    );
                }
            }
        }
        out.sort();
        Ok(out)
    }
    pub fn bytes(&self, path: &str) -> Result<Vec<u8>> {
        let p = self.path(path, false)?;
        Self::read_bounded(&p)
    }
    fn read_bounded(p: &Path) -> Result<Vec<u8>> {
        let metadata = fs::symlink_metadata(p)?;
        ensure!(
            metadata.is_file() && metadata.len() <= MAX_FILE as u64,
            "file exceeds read limit"
        );
        let mut bytes = vec![];
        fs::File::open(p)?
            .take((MAX_FILE + 1) as u64)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= MAX_FILE, "file grew beyond read limit");
        Ok(bytes)
    }
    pub fn revision(&self) -> Result<String> {
        let mut records = vec![];
        let mut total = 0u64;
        for path in self.files()? {
            let p = self.checked_path(&path, false)?;
            records.push((path, snapshot_hash(&p, &mut total)?));
        }
        // Ignored paths are outside native tools; instruction/ignore changes still invalidate approvals.
        for name in [".gitignore", ".ignore"] {
            let p = self.root.join(name);
            if fs::symlink_metadata(&p).is_ok() {
                records.push((name.into(), snapshot_hash(&p, &mut total)?));
            }
        }
        Ok(hash(&serde_json::to_vec(&(&self.root, records))?))
    }
    /// A unique exact replacement reduces generated output without fuzzy mutation.
    pub fn replacement(
        &self,
        path: &str,
        before_hash: &str,
        old: &str,
        new: &str,
    ) -> Result<Action> {
        ensure!(
            !old.is_empty() && old != new,
            "replacement must change a nonempty exact snippet"
        );
        let bytes = self.bytes(path)?;
        ensure!(
            hash(&bytes) == before_hash,
            "replacement original hash mismatch; read current file first"
        );
        let text = std::str::from_utf8(&bytes).context("replacement requires UTF-8")?;
        ensure!(
            bytes
                .windows(old.len())
                .filter(|window| *window == old.as_bytes())
                .count()
                == 1,
            "replacement snippet must occur exactly once; read more surrounding context"
        );
        let edits = vec![Edit {
            path: path.into(),
            before_hash: Some(before_hash.into()),
            content: text.replacen(old, new, 1),
        }];
        self.validate_edits(&edits)?;
        Ok(Action::Patch { edits })
    }

    pub fn validate_edits(&self, edits: &[Edit]) -> Result<Vec<(PathBuf, Option<Vec<u8>>)>> {
        ensure!(
            !edits.is_empty() && edits.len() <= 16,
            "patch must change 1..16 files"
        );
        let mut names = BTreeSet::new();
        let mut out = vec![];
        for edit in edits {
            let name = edit.path.to_lowercase();
            ensure!(
                !names.iter().any(
                    |existing: &String| name.starts_with(&format!("{existing}/"))
                        || existing.starts_with(&format!("{name}/"))
                ),
                "patch paths conflict as file and parent directory"
            );
            ensure!(
                names.insert(edit.path.to_lowercase()),
                "duplicate/case-alias patch path"
            );
            ensure!(
                edit.content.len() <= MAX_FILE && !edit.content.contains('\0'),
                "patch content is binary or exceeds file limit"
            );
            let path = self.path(&edit.path, edit.before_hash.is_none())?;
            let old = if edit.before_hash.is_some() {
                Some(self.bytes(&edit.path)?)
            } else {
                ensure!(!path.exists(), "new file already exists");
                None
            };
            ensure!(
                old.as_deref().map(hash) == edit.before_hash,
                "original file hash mismatch"
            );
            if let Some(b) = &old {
                std::str::from_utf8(b).context("binary patches unsupported")?;
            }
            out.push((path, old));
        }
        Ok(out)
    }
    pub fn diff(&self, edits: &[Edit]) -> Result<String> {
        let originals = self.validate_edits(edits)?;
        let mut out = String::new();
        for (e, (_, old)) in edits.iter().zip(&originals) {
            let before = std::str::from_utf8(old.as_deref().unwrap_or_default())?;
            out.push_str(
                &similar::TextDiff::from_lines(before, &e.content)
                    .unified_diff()
                    .header(&format!("a/{}", e.path), &format!("b/{}", e.path))
                    .to_string(),
            );
        }
        Ok(out)
    }
    pub fn apply(&self, edits: &[Edit], store: &Store, revision: &str) -> Result<()> {
        let original = self.validate_edits(edits)?;
        for (_, bytes) in &original {
            ensure!(
                !bytes
                    .as_ref()
                    .is_some_and(|b| store.redactor.contains_secret(&String::from_utf8_lossy(b))),
                "patch touches a known secret-bearing file; edit it outside the agent"
            );
        }
        let mut recovery = vec![];
        for (edit, (path, bytes)) in edits.iter().zip(&original) {
            let before = bytes
                .as_ref()
                .map(|b| store.put(b, "patch_backup", "patch", revision, vec![]))
                .transpose()?
                .map(|a| a.hash);
            recovery.push(RecoveryFile {
                path: edit.path.clone(),
                before,
                after: hash(edit.content.as_bytes()),
                #[cfg(unix)]
                before_mode: {
                    use std::os::unix::fs::PermissionsExt;
                    fs::metadata(path).ok().map(|m| m.permissions().mode())
                },
            });
        }
        let journal = store.dir.join("patch-recovery.json");
        let mut created_dirs = vec![];
        atomic_write(
            &journal,
            &serde_json::to_vec(
                &serde_json::json!({"files":recovery,"created_dirs":created_dirs}),
            )?,
        )?;
        for edit in edits {
            let relative = Path::new(&edit.path);
            let mut parent = PathBuf::new();
            for component in relative.parent().context("patch parent")?.components() {
                parent.push(component);
                let relative_parent = parent.to_str().context("non UTF-8 parent")?;
                let at = self.path(relative_parent, true)?;
                if !at.exists() {
                    if let Err(error) = fs::create_dir(&at) {
                        let rollback = self.recover(store);
                        bail!("patch directory creation failed: {error}; rollback: {rollback:?}");
                    }
                    created_dirs.push(relative_parent.to_owned());
                    atomic_write(
                        &journal,
                        &serde_json::to_vec(
                            &serde_json::json!({"files":recovery,"created_dirs":created_dirs}),
                        )?,
                    )?;
                }
            }
        }
        for (edit, (path, old)) in edits.iter().zip(&original) {
            // Recheck immediately before each file replacement. This is not multi-file atomicity.
            self.path(&edit.path, edit.before_hash.is_none())?;
            let current = fs::read(path).ok();
            if current != *old {
                bail!(
                    "concurrent edit during patch; recovery journal retained; run s1code recover"
                );
            }
            let permissions = fs::metadata(path).ok().map(|m| m.permissions());
            if let Err(e) = atomic_write(path, edit.content.as_bytes()) {
                let recovery_result = self.recover(store);
                bail!("patch write failed: {e}; rollback: {recovery_result:?}");
            }
            if let Some(p) = permissions {
                fs::set_permissions(path, p)?;
            }
        }
        fs::remove_file(journal)?;
        Ok(())
    }
    pub fn recover(&self, store: &Store) -> Result<()> {
        let journal = store.dir.join("patch-recovery.json");
        let recovery: PatchJournal =
            serde_json::from_slice(&fs::read(&journal).context("no patch recovery journal")?)?;
        let (recovery, created_dirs) = match recovery {
            PatchJournal::Legacy(files) => (files, vec![]),
            PatchJournal::Directories {
                files,
                created_dirs,
            } => (files, created_dirs),
        };
        for dir in &created_dirs {
            let path = self.path(dir, true)?;
            ensure!(
                !path.exists() || path.is_dir(),
                "recovery directory was replaced by a file; preserve manual changes"
            );
        }
        // Validate all current hashes before any recovery mutation; preserve unrelated edits.
        for r in &recovery {
            let p = self.path(&r.path, true)?;
            let current = fs::read(p).ok().map(|b| hash(&b));
            ensure!(
                current == r.before || current.as_deref() == Some(&r.after),
                "recovery conflict in {}; preserve manual edits before retrying",
                r.path
            );
        }
        for r in recovery {
            let p = self.path(&r.path, true)?;
            if let Some(before) = r.before {
                atomic_write(&p, &store.get(&before)?)?;
                #[cfg(unix)]
                if let Some(mode) = r.before_mode {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&p, fs::Permissions::from_mode(mode))?;
                }
            } else if p.exists() {
                fs::remove_file(p)?;
            }
        }
        for dir in created_dirs.iter().rev() {
            let path = self.path(dir, true)?;
            match fs::remove_dir(path) {
                Ok(()) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(e) => return Err(e.into()),
            }
        }
        fs::remove_file(journal)?;
        Ok(())
    }
    pub async fn execute(
        &self,
        action: &Action,
        store: &Store,
        cancel: &CancellationToken,
    ) -> Result<ToolResult> {
        ensure!(!cancel.is_cancelled(), "cancelled before execution");
        let mut exit = None;
        let mut diagnostic = false;
        let text = match action {
            Action::List => self.files()?.join("\n"),
            Action::Read { path, start, lines } => {
                ensure!(
                    *start > 0 && *lines > 0 && *lines <= 300,
                    "read range must start at 1 or later, with 1..300 lines"
                );
                let bytes = self.bytes(path)?;
                let content = std::str::from_utf8(&bytes).context("file is not UTF-8")?;
                format!(
                    "path: {path}\nsha256: {}\n{}",
                    hash(&bytes),
                    content
                        .lines()
                        .enumerate()
                        .skip(start - 1)
                        .take(*lines)
                        .map(|(i, l)| format!("{}: {l}", i + 1))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
            Action::Search { query } => {
                ensure!(
                    !query.is_empty() && query.len() <= 256,
                    "search needs a literal query of 1..256 bytes"
                );
                let mut out = String::new();
                let mut scanned = 0usize;
                for path in self.files()? {
                    if cancel.is_cancelled() {
                        bail!("cancelled");
                    }
                    // files() already applied all discovery exclusions. Recheck
                    // ancestors and bound the open, without walking N files N times.
                    let Ok(bytes) = self
                        .checked_path(&path, false)
                        .and_then(|p| Self::read_bounded(&p))
                    else {
                        continue;
                    };
                    scanned += bytes.len();
                    if scanned > 16 * 1024 * 1024 {
                        out.push_str("[search scan budget reached]\n");
                        break;
                    }
                    if let Ok(s) = std::str::from_utf8(&bytes) {
                        for (i, l) in s.lines().enumerate() {
                            if l.contains(query) {
                                out.push_str(&format!(
                                    "{path}:{}:{}\n",
                                    i + 1,
                                    l.chars().take(500).collect::<String>()
                                ));
                                if out.len() > MAX_OUTPUT {
                                    break;
                                }
                            }
                        }
                    }
                    if out.len() > MAX_OUTPUT {
                        break;
                    }
                }
                out
            }
            Action::Patch { edits } => {
                let diff = self.diff(edits)?;
                self.apply(edits, store, &self.revision()?)?;
                diff
            }
            Action::Run { argv, .. } => {
                self.validate_command(argv)?;
                let r = process(&self.root, argv, cancel, Duration::from_secs(120)).await?;
                exit = r.exit_code;
                diagnostic = r.diagnostic;
                r.text
            }
            Action::Git => {
                // Builtin git inspections do not run aliases, external diff drivers, pagers, or hooks.
                let filters = process(
                    &self.root,
                    &["git", "config", "--includes", "--get-regexp", "^filter\\."]
                        .map(String::from),
                    cancel,
                    Duration::from_secs(10),
                )
                .await?;
                ensure!(
                    filters.exit_code != Some(0),
                    "repository Git filters may execute code; automatic Git inspection is disabled for this repository"
                );
                ensure!(
                    filters.exit_code == Some(1),
                    "unable to inspect Git configuration safely"
                );
                let paths = self.files()?;
                if paths.is_empty() {
                    return Ok(ToolResult {
                        text: "No allowed files for git inspection".into(),
                        exit_code: None,
                        diagnostic: false,
                    });
                }
                let mut status_args = [
                    "git",
                    "--no-pager",
                    "--literal-pathspecs",
                    "-c",
                    "core.fsmonitor=false",
                    "status",
                    "--short",
                    "--",
                ]
                .map(String::from)
                .to_vec();
                status_args.insert(1, format!("--work-tree={}", self.root.display()));
                status_args.insert(status_args.len() - 1, "--ignore-submodules=all".into());
                status_args.extend(paths.clone());
                let mut diff_args = [
                    "git",
                    "--no-pager",
                    "--literal-pathspecs",
                    "-c",
                    "core.fsmonitor=false",
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--",
                ]
                .map(String::from)
                .to_vec();
                diff_args.insert(1, format!("--work-tree={}", self.root.display()));
                diff_args.insert(diff_args.len() - 1, "--ignore-submodules=all".into());
                diff_args.extend(paths);
                let status =
                    process(&self.root, &status_args, cancel, Duration::from_secs(10)).await?;
                let diff = process(&self.root, &diff_args, cancel, Duration::from_secs(10)).await?;
                format!("{}\n{}", status.text, diff.text)
            }
            Action::Rehydrate { artifact } => {
                String::from_utf8(store.get(artifact)?).context("artifact not UTF-8")?
            }
            _ => bail!("action is an engine transition, not a tool"),
        };
        Ok(ToolResult {
            text: bound(&text, MAX_OUTPUT),
            exit_code: exit,
            diagnostic,
        })
    }
}

// Hash incrementally and enforce the cap on bytes actually read, including files
// that grow after metadata inspection. Never follow an ignore-file symlink.
fn snapshot_hash(path: &Path, total: &mut u64) -> Result<String> {
    use sha2::{Digest, Sha256};
    const LIMIT: u64 = 128 * 1024 * 1024;
    let metadata = fs::symlink_metadata(path)?;
    ensure!(metadata.is_file(), "snapshot requires a regular file");
    ensure!(
        metadata.len() <= LIMIT.saturating_sub(*total),
        "workspace snapshot exceeds 128 MiB; narrow workspace/exclusions"
    );
    let mut reader = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        *total += n as u64;
        ensure!(*total <= LIMIT, "workspace grew beyond snapshot limit");
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum PatchJournal {
    Legacy(Vec<RecoveryFile>),
    Directories {
        files: Vec<RecoveryFile>,
        created_dirs: Vec<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct RecoveryFile {
    path: String,
    before: Option<String>,
    after: String,
    #[cfg(unix)]
    #[serde(default)]
    before_mode: Option<u32>,
}

pub fn bound(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.into();
    }
    let mut at = max;
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    format!("{}\n[capture truncated at {max} bytes]", &s[..at])
}

/// Recognized empty-run summaries are negative verification evidence, even when
/// a runner exits successfully. Unknown formats do not imply a test count.
pub fn empty_test_run(argv: &[String], output: &str) -> bool {
    let args: Vec<_> = argv.iter().map(String::as_str).collect();
    let mut counts = output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let count = match args.as_slice() {
                ["python3", "-m", "unittest", ..] => {
                    let rest = line.strip_prefix("Ran ")?;
                    let (count, rest) = rest.split_once(' ')?;
                    (rest.starts_with("tests in ") || rest.starts_with("test in ")).then_some(count)
                }
                ["node", "--test"] | ["npm", "--offline", "run", "test"] => line
                    .strip_prefix("# tests ")
                    .or_else(|| line.strip_prefix("ℹ tests ")),
                ["python3", "-m", "pytest", ..]
                    if line.contains("no tests ran") || line.contains("collected 0 items") =>
                {
                    Some("0")
                }
                ["python3", "-m", "pytest", ..] => {
                    let line = line.trim_matches('=').trim();
                    line.split_once(" passed").map(|(count, _)| count)
                }
                ["cargo", "test", ..] => line
                    .strip_prefix("test result: ok. ")?
                    .split_once(" passed;")
                    .map(|(count, _)| count),
                _ => None,
            }?;
            count.parse::<u64>().ok()
        })
        .peekable();
    counts.peek().is_some() && counts.all(|count| count == 0)
}

pub fn clean_environment(cmd: &mut Command) {
    cmd.env_clear();
    for k in [
        "PATH",
        "LANG",
        "LC_ALL",
        "TMPDIR",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
    ] {
        if let Some(v) = std::env::var_os(k) {
            cmd.env(k, v);
        }
    }
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("PYTHONDONTWRITEBYTECODE", "1");
}

async fn capture(mut reader: impl tokio::io::AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut out = vec![];
    let mut buf = [0u8; 4096];
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        let remaining = MAX_OUTPUT.saturating_sub(out.len());
        out.extend_from_slice(&buf[..n.min(remaining)]);
    }
    Ok(out)
}

struct ProcessGroup(u32);
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.0 as i32), libc::SIGKILL);
        }
    }
}

pub async fn process(
    cwd: &Path,
    argv: &[String],
    cancel: &CancellationToken,
    timeout: Duration,
) -> Result<ToolResult> {
    ensure!(!argv.is_empty(), "empty command");
    ensure!(!cancel.is_cancelled(), "cancelled");
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    clean_environment(&mut cmd);
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().context("could not start command")?;
    let guard = ProcessGroup(child.id().context("process id unavailable")?);
    let out = tokio::spawn(capture(child.stdout.take().context("stdout unavailable")?));
    let err = tokio::spawn(capture(child.stderr.take().context("stderr unavailable")?));
    let status = tokio::select! {
        biased;
        _=cancel.cancelled()=>{drop(guard);let _=child.wait().await;out.abort();err.abort();bail!("cancelled; process group stopped")},
        _=tokio::time::sleep(timeout)=>{drop(guard);let _=child.wait().await;out.abort();err.abort();bail!("process timed out; process group stopped")},
        status=child.wait()=>status?,
    };
    drop(guard); // Stop descendants even when the leader exits successfully.
    let stdout = tokio::time::timeout(Duration::from_secs(2), out).await???;
    let stderr = tokio::time::timeout(Duration::from_secs(2), err).await???;
    let truncated = stdout.len() == MAX_OUTPUT || stderr.len() == MAX_OUTPUT;
    let text = format!(
        "exit: {:?}\nstdout:\n{}\nstderr:\n{}{}",
        status.code(),
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr),
        if truncated {
            "\n[capture truncated]"
        } else {
            ""
        }
    );
    Ok(ToolResult {
        text,
        exit_code: status.code(),
        diagnostic: !status.success(),
    })
}
