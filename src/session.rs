use crate::{brand, domain::*, privacy::Redactor};
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("path has no parent")?;
    let tmp = parent.join(format!(".write-{}", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut f = options.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

pub fn default_home() -> Result<PathBuf> {
    if let Some(s) =
        std::env::var_os(brand::HOME_ENV).or_else(|| std::env::var_os(brand::LEGACY_HOME_ENV))
    {
        return Ok(PathBuf::from(s));
    }
    let dirs = directories::ProjectDirs::from("", "", brand::BIN)
        .context("Cannot locate data directory; set S1CODE_HOME")?;
    let current = dirs.data_local_dir().to_path_buf();
    let legacy = directories::ProjectDirs::from("", "", brand::LEGACY_BIN)
        .context("Cannot locate legacy data directory")?;
    Ok(select_home(current, legacy.data_local_dir().to_path_buf()))
}

/// Reuse the existing store in place; never copy a live journal or merge stores.
pub fn select_home(current: PathBuf, legacy: PathBuf) -> PathBuf {
    if !current.exists() && legacy.join("sessions").is_dir() {
        legacy
    } else {
        current
    }
}

#[derive(Debug, serde::Serialize)]
pub struct SavedSession {
    pub id: String,
    pub task: String,
    pub status: RunStatus,
    pub mode: Mode,
    pub simulation: bool,
    pub workspace_name: String,
    pub updated_unix_ms: Option<u128>,
    #[serde(skip)]
    pub workspace: PathBuf,
}

/// Read-only index. A corrupt checkpoint cannot hide other recoverable sessions.
pub fn saved_sessions(home: &Path) -> Result<Vec<SavedSession>> {
    let dir = home.join("sessions");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error).context("cannot list saved sessions"),
    };
    let mut sessions = vec![];
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let checkpoint = entry.path().join("checkpoint.json");
        let Ok(metadata) = fs::symlink_metadata(&checkpoint) else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
            continue;
        }
        let Ok(bytes) = fs::read(&checkpoint) else {
            continue;
        };
        let Ok(s) = serde_json::from_slice::<Session>(&bytes) else {
            continue;
        };
        if s.version != brand::STORAGE_VERSION
            || uuid::Uuid::parse_str(&s.id).is_err()
            || entry.file_name().to_str() != Some(&s.id)
        {
            continue;
        }
        let redactor = Redactor::environment(&s.workspace);
        let workspace = PathBuf::from(&s.workspace);
        sessions.push(SavedSession {
            id: s.id,
            task: redactor.text(&s.task).chars().take(160).collect(),
            status: s.status,
            mode: s.config.mode,
            simulation: s.config.offline_demo,
            workspace_name: redactor
                .text(&workspace.file_name().unwrap_or_default().to_string_lossy()),
            workspace,
            updated_unix_ms: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|time| time.as_millis()),
        });
    }
    sessions.sort_by(|a, b| {
        b.updated_unix_ms
            .cmp(&a.updated_unix_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(sessions)
}

pub fn valid_session_selector(selector: &str) -> bool {
    selector == "latest"
        || (8..=36).contains(&selector.len())
            && selector.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-')
}

/// `latest` is always scoped to the current workspace; prefixes must be unique.
pub fn resolve_session(home: &Path, selector: &str, workspace: &Path) -> Result<String> {
    ensure!(
        valid_session_selector(selector),
        "Use latest, a session UUID, or a unique prefix of at least 8 characters"
    );
    if uuid::Uuid::parse_str(selector).is_ok() {
        return Ok(selector.to_lowercase());
    }
    let sessions = saved_sessions(home)?;
    if selector == "latest" {
        let workspace = workspace
            .canonicalize()
            .context("current workspace unavailable")?;
        return sessions.into_iter().find(|s| s.workspace == workspace).map(|s| s.id)
            .context("No saved session in this workspace. Use s1code sessions or /sessions to find an existing task.");
    }
    let prefix = selector.to_lowercase();
    let matches: Vec<_> = sessions
        .into_iter()
        .filter(|s| s.id.starts_with(&prefix))
        .collect();
    ensure!(
        matches.len() <= 1,
        "Ambiguous session prefix; use more characters or the full ID"
    );
    matches
        .into_iter()
        .next()
        .map(|s| s.id)
        .context("No saved session matches this prefix")
}

pub struct Store {
    pub dir: PathBuf,
    _lock: File,
    pub redactor: Redactor,
}
impl Drop for Store {
    fn drop(&mut self) {
        // Closing only our descriptor can leave flock held by a forked child
        // until exec. Release ownership when the store itself stops owning it.
        let _ = FileExt::unlock(&self._lock);
    }
}

impl Store {
    pub fn create(
        home: &Path,
        workspace: &Path,
        task: String,
        config: RunConfig,
    ) -> Result<(Self, Session)> {
        let workspace = workspace.canonicalize()?;
        let id = uuid::Uuid::new_v4().to_string();
        let store = Self::open(home, &id, &workspace)?;
        let s = Session {
            version: brand::STORAGE_VERSION,
            id,
            workspace: workspace.to_string_lossy().into(),
            task: store.redactor.text(&task),
            prior_user_requests: vec![],
            config,
            status: RunStatus::Running,
            context: vec![],
            pending: None,
            inflight: None,
            verified: None,
            metrics: Metrics::default(),
            steps: 0,
            seen: Default::default(),
            proposals: vec![],
            bridge_thread: None,
            bridge_turn: None,
            event_seq: 0,
            recovery_needed: false,
            current_revision: String::new(),
        };
        store.save(&s)?;
        Ok((store, s))
    }
    pub fn follow_up(&self, s: &mut Session, message: &str) -> Result<()> {
        ensure!(
            s.config.mode == Mode::Native,
            "native follow-up requires a native session"
        );
        ensure!(
            !s.recovery_needed
                && s.inflight.is_none()
                && !self.dir.join("patch-recovery.json").exists(),
            "Recover interrupted execution before submitting a follow-up; no actions were replayed"
        );
        ensure!(
            !message.trim().is_empty() && message.chars().count() <= 8192,
            "follow-up must contain 1..8192 characters"
        );
        ensure!(
            !self.redactor.contains_secret(message),
            "follow-up contains sensitive content; enter credentials outside the conversation"
        );
        s.prior_user_requests
            .push(std::mem::replace(&mut s.task, message.trim().into()));
        if s.status == RunStatus::Completed {
            for c in &mut s.context {
                if matches!(c.action, Action::Patch { .. }) {
                    c.pinned = false;
                }
            }
        }
        s.pending = None;
        s.proposals.clear();
        s.seen.clear();
        s.verified = None;
        s.status = RunStatus::Running;
        self.record(s, "user_followup", serde_json::json!({"message":message.trim(),"old_proposals_discarded":true,"metrics_reset":false}))?;
        Ok(())
    }
    fn open(home: &Path, id: &str, workspace: &Path) -> Result<Self> {
        uuid::Uuid::parse_str(id).context("invalid session id")?;
        private_dir(home)?;
        #[cfg(unix)]
        let locks = PathBuf::from(format!(
            "/tmp/{}-workspace-locks-{}",
            brand::WORKSPACE_LOCK_NAMESPACE,
            unsafe { libc::geteuid() }
        ));
        #[cfg(not(unix))]
        let locks = home.join("locks");
        private_dir(&locks)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(locks.join(hash(workspace.as_os_str().as_encoded_bytes())))?;
        lock.try_lock_exclusive()
            .context("Workspace is already owned by another S1Code process")?;
        let dir = home.join("sessions").join(id);
        private_dir(&dir)?;
        private_dir(&dir.join("artifacts"))?;
        Ok(Self {
            dir,
            _lock: lock,
            redactor: Redactor::environment(&workspace.to_string_lossy()),
        })
    }
    pub fn resume(home: &Path, id: &str) -> Result<(Self, Session)> {
        uuid::Uuid::parse_str(id).context("invalid session id")?;
        let path = home.join("sessions").join(id).join("checkpoint.json");
        let mut s: Session =
            serde_json::from_slice(&fs::read(&path).context("session checkpoint unavailable")?)?;
        ensure!(
            s.version == brand::STORAGE_VERSION,
            "unsupported storage version {}; no automatic migration",
            s.version
        );
        let store = Self::open(home, id, Path::new(&s.workspace))?;
        let locked: Session = serde_json::from_slice(&fs::read(&path)?)?;
        ensure!(
            locked.id == id && locked.version == brand::STORAGE_VERSION,
            "session checkpoint identity or version mismatch"
        );
        ensure!(
            locked.workspace == s.workspace,
            "session workspace changed while acquiring its lock; retry resume"
        );
        s = locked;
        // The journal may be ahead of the checkpoint. Do not replay unknown effects.
        let events = store.events()?;
        if events.last().is_some_and(|e| e.seq > s.event_seq) || s.inflight.is_some() {
            s.status = RunStatus::Blocked;
            s.recovery_needed = true;
        }
        s.event_seq = events
            .last()
            .map_or(s.event_seq, |e| e.seq.max(s.event_seq));
        Ok((store, s))
    }
    pub fn save(&self, s: &Session) -> Result<()> {
        atomic_write(
            &self.dir.join("checkpoint.json"),
            &serde_json::to_vec_pretty(s)?,
        )
    }
    pub fn record(&self, s: &mut Session, kind: &str, data: serde_json::Value) -> Result<RunEvent> {
        s.event_seq += 1;
        let mut data = data;
        if s.config.offline_demo
            && let Some(object) = data.as_object_mut()
        {
            object.insert(
                "execution_label".into(),
                "OFFLINE SIMULATION — no model inference".into(),
            );
        }
        let event = RunEvent {
            seq: s.event_seq,
            session: s.id.clone(),
            kind: kind.into(),
            data: self.redactor.value(&data),
        };
        let mut line = serde_json::to_vec(&event)?;
        line.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("events.jsonl"))?;
        file.write_all(&line)?;
        file.sync_data()?;
        self.save(s)?;
        Ok(event)
    }
    pub fn events(&self) -> Result<Vec<RunEvent>> {
        let path = self.dir.join("events.jsonl");
        if !path.exists() {
            return Ok(vec![]);
        }
        let bytes = fs::read(path)?;
        ensure!(
            bytes.is_empty() || bytes.ends_with(b"\n"),
            "partial journal tail; preserve and inspect events.jsonl before recovery"
        );
        bytes
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .map(|l| Ok(serde_json::from_slice(l)?))
            .collect()
    }
    pub fn put(
        &self,
        bytes: &[u8],
        origin: &str,
        call_id: &str,
        revision: &str,
        dependencies: Vec<String>,
    ) -> Result<ArtifactRef> {
        let h = hash(bytes);
        let path = self.dir.join("artifacts").join(&h);
        if !path.exists() {
            atomic_write(&path, bytes)?;
        }
        Ok(ArtifactRef {
            hash: h,
            bytes: bytes.len(),
            origin: origin.into(),
            call_id: call_id.into(),
            revision: revision.into(),
            dependencies,
        })
    }
    pub fn get(&self, id: &str) -> Result<Vec<u8>> {
        ensure!(
            id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid artifact id"
        );
        let bytes = fs::read(self.dir.join("artifacts").join(id))?;
        ensure!(hash(&bytes) == id, "artifact integrity failure");
        Ok(bytes)
    }
    pub fn export(&self, s: &Session, path: &Path) -> Result<()> {
        if s.config.decision == "jev" || s.config.eviction == "jev" {
            bail!("Jev traces remain private pending documented clearance; export disabled");
        }
        let mut out = String::new();
        for e in self.events()? {
            out.push_str(&serde_json::to_string(&serde_json::json!({"playback":"REPLAY — recorded events, not live execution", "event":self.redactor.value(&serde_json::to_value(e)?)}))?);
            out.push('\n');
        }
        atomic_write(path, out.as_bytes())
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;

    #[test]
    fn session_lookup_scopes_latest_and_refuses_ambiguous_prefixes() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (store, mut s) = Store::create(
            home.path(),
            first.path(),
            "first task".into(),
            Default::default(),
        )
        .unwrap();
        let first_id = s.id.clone();
        drop(store);
        let (store, other) = Store::create(
            home.path(),
            second.path(),
            "other task".into(),
            Default::default(),
        )
        .unwrap();
        drop(store);
        assert_eq!(
            resolve_session(home.path(), "latest", first.path()).unwrap(),
            first_id
        );
        assert_eq!(
            resolve_session(home.path(), "latest", second.path()).unwrap(),
            other.id
        );
        assert_eq!(
            resolve_session(home.path(), &first_id[..8], second.path()).unwrap(),
            first_id
        );
        assert!(resolve_session(home.path(), "../../private", first.path()).is_err());
        let empty = tempfile::tempdir().unwrap();
        assert!(resolve_session(home.path(), "latest", empty.path()).is_err());
        for suffix in ['a', 'b'] {
            s.id = format!("abcdefab-0000-4000-8000-00000000000{suffix}");
            let dir = home.path().join("sessions").join(&s.id);
            fs::create_dir(&dir).unwrap();
            atomic_write(
                &dir.join("checkpoint.json"),
                &serde_json::to_vec(&s).unwrap(),
            )
            .unwrap();
        }
        assert!(
            resolve_session(home.path(), "abcdefab", first.path())
                .unwrap_err()
                .to_string()
                .contains("Ambiguous")
        );
        fs::write(
            home.path()
                .join("sessions")
                .join(&first_id)
                .join("checkpoint.json"),
            b"partial",
        )
        .unwrap();
        assert_eq!(saved_sessions(home.path()).unwrap().len(), 3);
    }

    #[test]
    fn resume_refuses_mismatched_checkpoint_identity() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (store, mut s) =
            Store::create(home.path(), root.path(), "task".into(), Default::default()).unwrap();
        let id = s.id.clone();
        s.id = uuid::Uuid::new_v4().to_string();
        store.save(&s).unwrap();
        drop(store);
        assert!(Store::resume(home.path(), &id).is_err());
    }

    #[test]
    fn dropping_store_releases_lock_with_a_duplicate_descriptor_alive() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (store, session) =
            Store::create(home.path(), root.path(), "test".into(), Default::default()).unwrap();
        let duplicate = store._lock.try_clone().unwrap();
        assert!(Store::resume(home.path(), &session.id).is_err());
        drop(store);
        let (resumed, _) = Store::resume(home.path(), &session.id).unwrap();
        assert!(Store::resume(home.path(), &session.id).is_err());
        drop(duplicate);
        assert!(Store::resume(home.path(), &session.id).is_err());
        drop(resumed);
        assert!(Store::resume(home.path(), &session.id).is_ok());
    }
}
