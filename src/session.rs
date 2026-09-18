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
    if let Some(s) = std::env::var_os(brand::HOME_ENV) {
        return Ok(PathBuf::from(s));
    }
    let dirs = directories::ProjectDirs::from("", "", brand::BIN)
        .context("Cannot locate data directory; set NERVE_HOME")?;
    Ok(dirs.data_local_dir().to_path_buf())
}

pub struct Store {
    pub dir: PathBuf,
    _lock: File,
    pub redactor: Redactor,
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
        };
        store.save(&s)?;
        Ok((store, s))
    }
    fn open(home: &Path, id: &str, workspace: &Path) -> Result<Self> {
        uuid::Uuid::parse_str(id).context("invalid session id")?;
        private_dir(home)?;
        #[cfg(unix)]
        let locks = PathBuf::from(format!("/tmp/{}-workspace-locks-{}", brand::BIN, unsafe {
            libc::geteuid()
        }));
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
            .context("Workspace is already owned by another Nerve process")?;
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
        s = serde_json::from_slice(&fs::read(&path)?)?;
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
