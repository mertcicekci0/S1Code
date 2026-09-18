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
        relative.components().all(|c| match c {
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
        let path = Path::new(relative);
        ensure!(
            !relative.is_empty() && self.allowed_name(path),
            "path is excluded, sensitive, or outside the workspace"
        );
        let mut at = self.root.clone();
        for c in path.components() {
            at.push(c);
            match fs::symlink_metadata(&at) {
                Ok(m) => ensure!(!m.file_type().is_symlink(), "symlinks are unsupported"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && new => {}
                Err(e) => return Err(e.into()),
            }
        }
        ensure!(at.starts_with(&self.root), "path escaped root");
        if !new {
            ensure!(
                self.files()?.contains(&relative.to_owned()),
                "ignored or unsupported file"
            );
        } else {
            // Query ignore matching through the same walker after checking parent.
            let parent = at.parent().context("no parent")?;
            ensure!(parent.is_dir(), "new files require an existing directory");
            ensure!(!self.ignored(relative)?, "new file matches ignore rules");
        }
        Ok(at)
    }
    fn ignored(&self, path: &str) -> Result<bool> {
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
            .matched_path_or_any_parents(self.root.join(path), false)
            .is_ignore())
    }
    pub fn files(&self) -> Result<Vec<String>> {
        let mut out = vec![];
        for item in WalkBuilder::new(&self.root)
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
        ensure!(
            p.metadata()?.len() <= MAX_FILE as u64,
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
            let p = self.root.join(&path);
            let meta = p.metadata()?;
            total += meta.len();
            ensure!(
                total <= 128 * 1024 * 1024,
                "workspace snapshot exceeds 128 MiB; narrow workspace/exclusions"
            );
            let content = fs::read(&p)?;
            records.push((path, hash(&content)));
        }
        // Ignored paths are outside native tools; instruction/ignore changes still invalidate approvals.
        for name in [".gitignore", ".ignore"] {
            let p = self.root.join(name);
            if p.is_file() {
                records.push((name.into(), hash(&fs::read(p)?)));
            }
        }
        Ok(hash(&serde_json::to_vec(&(&self.root, records))?))
    }
    pub fn validate_edits(&self, edits: &[Edit]) -> Result<Vec<(PathBuf, Option<Vec<u8>>)>> {
        ensure!(
            !edits.is_empty() && edits.len() <= 16,
            "patch must change 1..16 files"
        );
        let mut names = BTreeSet::new();
        let mut out = vec![];
        for edit in edits {
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
        let mut recovery = vec![];
        for (edit, (_, bytes)) in edits.iter().zip(&original) {
            let before = bytes
                .as_ref()
                .map(|b| store.put(b, "patch_backup", "patch", revision, vec![]))
                .transpose()?
                .map(|a| a.hash);
            recovery.push(RecoveryFile {
                path: edit.path.clone(),
                before,
                after: hash(edit.content.as_bytes()),
            });
        }
        let journal = store.dir.join("patch-recovery.json");
        atomic_write(&journal, &serde_json::to_vec(&recovery)?)?;
        for (edit, (path, old)) in edits.iter().zip(&original) {
            // Recheck immediately before each file replacement. This is not multi-file atomicity.
            let current = fs::read(path).ok();
            if current != *old {
                bail!("concurrent edit during patch; recovery journal retained; run nerve recover");
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
        let recovery: Vec<RecoveryFile> =
            serde_json::from_slice(&fs::read(&journal).context("no patch recovery journal")?)?;
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
            } else if p.exists() {
                fs::remove_file(p)?;
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
                    let Ok(bytes) = self.bytes(&path) else {
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
                ensure!(crate::policy::valid_command(argv), "command denied");
                let r = process(&self.root, argv, cancel, Duration::from_secs(120)).await?;
                exit = r.exit_code;
                diagnostic = r.diagnostic;
                r.text
            }
            Action::Git => {
                // Builtin git inspections do not run aliases, external diff drivers, pagers, or hooks.
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

#[derive(Serialize, Deserialize)]
struct RecoveryFile {
    path: String,
    before: Option<String>,
    after: String,
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
