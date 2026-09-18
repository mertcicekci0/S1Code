//! Official Codex App Server stdio client. Never executes upstream tool calls.
use crate::{
    brand,
    domain::*,
    session::{Store, hash},
    tools::clean_environment,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;

pub fn approval_policy() -> Value {
    json!({"granular":{"rules":true,"sandbox_approval":true,"mcp_elicitations":true,"request_permissions":true,"skill_approval":true}})
}

pub const TESTED_CLI: &str = "codex-cli 0.153.3";
pub async fn compatibility() -> Result<String> {
    let mut cmd = Command::new("codex");
    cmd.arg("--version");
    clean_environment(&mut cmd);
    let output = tokio::time::timeout(Duration::from_secs(10), cmd.output()).await??;
    ensure!(
        output.status.success(),
        "cannot run Codex CLI; install the official CLI"
    );
    let version = String::from_utf8(output.stdout)?.trim().to_owned();
    ensure!(
        version == TESTED_CLI,
        "unsupported Codex CLI {version}; protocol verified with {TESTED_CLI}. Validate generated schemas before updating the supported version"
    );
    Ok(version)
}

pub struct Rpc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    partial: Vec<u8>,
    next_id: u64,
    queue: VecDeque<Value>,
}
impl Drop for Rpc {
    fn drop(&mut self) {
        if let Some(id) = self.child.id() {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(id as i32), libc::SIGKILL);
            }
        }
    }
}
impl Rpc {
    pub async fn start(cwd: &Path, cancel: &CancellationToken) -> Result<Self> {
        compatibility().await?;
        let mut cmd = Command::new("codex");
        cmd.args([
            "app-server",
            "--listen",
            "stdio://",
            "-c",
            "approval_policy={granular={rules=true,sandbox_approval=true,mcp_elicitations=true,request_permissions=true,skill_approval=true}}",
            "-c",
            "sandbox_mode=\"read-only\"",
            "-c", "approvals_reviewer=\"user\"",
            "-c",
            "analytics.enabled=false",
            "-c",
            "mcp_servers={}",
            "-c",
            "apps._default.enabled=false",
        ]);
        Self::spawn(cmd, cwd, cancel).await
    }
    pub async fn spawn(mut cmd: Command, cwd: &Path, cancel: &CancellationToken) -> Result<Self> {
        clean_environment(&mut cmd);
        // Only the official child needs its managed storage location, never API-key envs.
        if let Some(v) = std::env::var_os("CODEX_HOME") {
            cmd.env("CODEX_HOME", v);
        }
        cmd.current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }
        let mut child = cmd
            .spawn()
            .context("official Codex app-server could not start")?;
        let stdin = child.stdin.take().context("app-server stdin unavailable")?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .context("app-server stdout unavailable")?,
        );
        let mut rpc = Self {
            child,
            stdin,
            stdout,
            partial: vec![],
            next_id: 1,
            queue: VecDeque::new(),
        };
        rpc.call("initialize",json!({"clientInfo":{"name":brand::BIN,"title":brand::NAME,"version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":false}}),cancel).await?;
        rpc.send(json!({"method":"initialized","params":{}}))
            .await?;
        Ok(rpc)
    }
    pub async fn send(&mut self, value: Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(&value)?;
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await?;
        self.stdin.flush().await?;
        Ok(())
    }
    async fn raw(&mut self, cancel: &CancellationToken) -> Result<Value> {
        loop {
            if let Some(i) = self.partial.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.partial.drain(..=i).collect();
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                return serde_json::from_slice(&line).context("malformed App Server JSONL");
            }
            let chunk = tokio::select! {biased;_=cancel.cancelled()=>bail!("bridge cancelled"),v=self.stdout.fill_buf()=>v?};
            ensure!(
                !chunk.is_empty(),
                "App Server closed the transport; run nerve doctor and inspect official CLI configuration"
            );
            let n = chunk.len();
            self.partial.extend_from_slice(chunk);
            self.stdout.consume(n);
            ensure!(
                self.partial.len() <= 2 * 1024 * 1024,
                "App Server frame exceeded 2 MiB"
            );
        }
    }
    pub async fn next(&mut self, cancel: &CancellationToken) -> Result<Value> {
        if let Some(v) = self.queue.pop_front() {
            Ok(v)
        } else {
            self.raw(cancel).await
        }
    }
    pub async fn call(
        &mut self,
        method: &str,
        params: Value,
        cancel: &CancellationToken,
    ) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let v = self.raw(cancel).await?;
                if v["id"] == id && v.get("method").is_none() {
                    ensure!(
                        v.get("error").is_none(),
                        "App Server rejected {method}: code {} (check CLI/schema/permissions)",
                        v["error"]["code"]
                    );
                    return Ok(v["result"].clone());
                }
                ensure!(self.queue.len() < 512, "too many queued App Server events");
                self.queue.push_back(v);
            }
        })
        .await
        .context("App Server request timed out")?
    }
}

pub async fn account(operation: &str, cancel: &CancellationToken) -> Result<Value> {
    let cwd = std::env::temp_dir();
    let mut rpc = Rpc::start(&cwd, cancel).await?;
    match operation {
        "status" => {
            let v = rpc
                .call("account/read", json!({"refreshToken":false}), cancel)
                .await?;
            Ok(
                json!({"managed_by":"official Codex CLI","account_type":v["account"]["type"],"requires_auth":v["requiresOpenaiAuth"]}),
            )
        }
        "logout" => rpc.call("account/logout", json!({}), cancel).await,
        "login" => {
            let v = rpc
                .call("account/login/start", json!({"type":"chatgpt"}), cancel)
                .await?;
            let url = v["authUrl"]
                .as_str()
                .context("managed ChatGPT login did not return an authorization URL")?;
            let parsed = reqwest::Url::parse(url)?;
            ensure!(
                parsed.scheme() == "https"
                    && parsed.host_str().is_some_and(|h| h == "auth.openai.com"
                        || h == "auth0.openai.com"
                        || h == "chatgpt.com"),
                "unexpected authorization host; inspect official CLI"
            );
            // Display only to the user, never a session log, export or child argument.
            eprintln!("Complete the official managed login in your browser:\n{url}");
            let login_id = v["loginId"].clone();
            loop {
                let event = tokio::select! {biased;_=cancel.cancelled()=>{let fresh=CancellationToken::new();let _=rpc.call("account/login/cancel",json!({"loginId":login_id}),&fresh).await;bail!("login cancelled")},v=rpc.next(cancel)=>v?};
                if event["method"] == "account/login/completed"
                    && event["params"]["loginId"] == login_id
                {
                    ensure!(
                        event["params"]["success"] == true,
                        "managed login failed; try the official CLI login diagnostic"
                    );
                    return Ok(json!({"login":"completed","managed_by":"official Codex CLI"}));
                }
            }
        }
        _ => bail!("unsupported account operation"),
    }
}

pub fn validate_permissions(result: &Value, workspace: &str) -> Result<()> {
    ensure!(
        result["approvalPolicy"] == approval_policy(),
        "upstream changed approval policy; refusing delegation"
    );
    ensure!(
        result["sandbox"]["type"] == "readOnly" && result["sandbox"]["networkAccess"] != true,
        "upstream changed sandbox/network policy; refusing delegation"
    );
    ensure!(
        result["cwd"].as_str() == Some(workspace),
        "upstream changed workspace"
    );
    Ok(())
}

/// Maps server requests to an approval kind; unknown requests are never executed.
pub fn approval_kind(method: &str) -> Option<&'static str> {
    match method {
        "item/commandExecution/requestApproval" => Some("command"),
        "item/fileChange/requestApproval" => Some("file"),
        _ => None,
    }
}

#[derive(Default)]
pub struct ApprovalLedger(BTreeMap<String, (String, Value)>);
impl ApprovalLedger {
    fn fingerprint(event: &Value) -> String {
        hash(
            &serde_json::to_vec(&(&event["id"], &event["method"], &event["params"]))
                .expect("JSON serializes"),
        )
    }
    pub fn cached(&self, event: &Value) -> Result<Option<Value>> {
        if let Some((fingerprint, response)) = self.0.get(&event["id"].to_string()) {
            ensure!(
                *fingerprint == Self::fingerprint(event),
                "upstream reused approval ID with changed arguments"
            );
            return Ok(Some(response.clone()));
        }
        Ok(None)
    }
    pub fn remember(&mut self, event: &Value, response: Value) {
        self.0.insert(
            event["id"].to_string(),
            (Self::fingerprint(event), response),
        );
    }
}

pub struct Bridge {
    pub store: Store,
    pub session: Session,
    pub events: mpsc::UnboundedSender<RunEvent>,
    pub inputs: mpsc::UnboundedReceiver<UiInput>,
    pub cancel: CancellationToken,
    pub interactive: bool,
}
impl Bridge {
    fn event(&mut self, kind: &str, data: Value) -> Result<()> {
        let event = self.store.record(&mut self.session, kind, data)?;
        let _ = self.events.send(event);
        Ok(())
    }
    pub async fn run(mut self) -> Result<Session> {
        let start = std::time::Instant::now();
        if let Err(e) = self.drive().await {
            self.session.status = if self.cancel.is_cancelled() {
                RunStatus::Cancelled
            } else {
                RunStatus::Failed
            };
            self.event("error", json!({"message":e.to_string()}))?;
        }
        self.session.metrics.elapsed_ms += start.elapsed().as_millis() as u64;
        self.event("summary",json!({"mode":"codex_delegated","status":self.session.status,"metrics":self.session.metrics,"internal_generative_calls":null,"internal_token_usage":null,"verification":self.session.verified,"note":"Upstream execution and verification belong to Codex; Nerve does not control hidden turns or context."}))?;
        Ok(self.session)
    }
    async fn drive(&mut self) -> Result<()> {
        ensure!(
            self.session.config.exclusions.is_empty(),
            "native path exclusions cannot be enforced inside Codex; use native mode or configure Codex separately"
        );
        if self.session.status == RunStatus::Completed {
            return Ok(());
        }
        let mut rpc = Rpc::start(Path::new(&self.session.workspace), &self.cancel).await?;
        let account = rpc
            .call("account/read", json!({"refreshToken":false}), &self.cancel)
            .await?;
        ensure!(
            account["account"]["type"] == "chatgpt",
            "Codex bridge requires managed ChatGPT login; run nerve login codex"
        );
        let mut params = json!({"cwd":self.session.workspace,"sandbox":"read-only","approvalPolicy":approval_policy(),"config":{"analytics.enabled":false,"mcp_servers":{},"apps._default.enabled":false}});
        if self.session.config.generation_model != "codex-default" {
            params["model"] = self.session.config.generation_model.clone().into();
        }
        let result = if let Some(id) = &self.session.bridge_thread {
            params["threadId"] = id.clone().into();
            rpc.call("thread/resume", params, &self.cancel).await?
        } else {
            rpc.call("thread/start", params, &self.cancel).await?
        };
        validate_permissions(&result, &self.session.workspace)?;
        let thread = result["thread"]["id"]
            .as_str()
            .context("thread response missing id")?
            .to_owned();
        self.session.bridge_thread = Some(thread.clone());
        self.event("bridge_connected",json!({"mode":"codex_delegated","thread":thread,"model":result["model"],"cli":TESTED_CLI,"sandbox":"readOnly","approval":"granular_all_gates","native_exclusions_enforced":false}))?;
        // A saved turn is observed through thread/read. It is never resubmitted.
        if let Some(turn) = self.session.bridge_turn.clone() {
            let snapshot = rpc
                .call(
                    "thread/read",
                    json!({"threadId":thread,"includeTurns":true}),
                    &self.cancel,
                )
                .await?;
            let status = snapshot["thread"]["turns"]
                .as_array()
                .and_then(|ts| ts.iter().find(|t| t["id"] == turn))
                .and_then(|t| t["status"].as_str())
                .unwrap_or("unknown");
            self.session.status = RunStatus::Blocked;
            self.event("bridge_resume_observed",json!({"turn":turn,"upstream_status":status,"automatic_resubmit":false,"message":"Previous delegated turn observed. Review the upstream record before starting another task."}))?;
            return Ok(());
        }
        ensure!(
            !self.session.recovery_needed,
            "delegation completion unknown; inspect upstream thread, no resubmission"
        );
        // Persist intent before sending. A crash between intent and response must not replay.
        self.session.recovery_needed = true;
        self.session.metrics.delegations += 1;
        self.event("delegation_started",json!({"task":self.session.task,"execution_owner":"official Codex","usage_unknown":true}))?;
        let started=rpc.call("turn/start",json!({"threadId":thread,"input":[{"type":"text","text":format!("{}\nScope: one bounded coding task. Inspect changes, run relevant verification, report results. Do not commit or push. Stop if blocked.",self.session.task)}],"cwd":self.session.workspace,"approvalPolicy":approval_policy(),"sandboxPolicy":{"type":"readOnly","networkAccess":false}}),&self.cancel).await?;
        let turn = started["turn"]["id"]
            .as_str()
            .context("turn missing id")?
            .to_owned();
        self.session.bridge_turn = Some(turn.clone());
        self.session.status = RunStatus::Running;
        self.store.save(&self.session)?;
        let mut answered = ApprovalLedger::default();
        let mut items = BTreeMap::<String, Value>::new();
        let mut check_seen = false;
        loop {
            let event = tokio::select! {biased;_=self.cancel.cancelled()=>{let fresh=CancellationToken::new();let _=rpc.call("turn/interrupt",json!({"threadId":thread,"turnId":turn}),&fresh).await;self.session.status=RunStatus::Cancelled;self.event("bridge_interrupted",json!({"turn":turn,"resubmit":false}))?;return Ok(())},e=rpc.next(&self.cancel)=>e?};
            let method = event["method"].as_str().unwrap_or("");
            let p = &event["params"];
            if let Some(id) = event.get("id") {
                if let Some(response) = answered.cached(&event)? {
                    rpc.send(response).await?;
                    continue;
                }
                let response = if let Some(kind) = approval_kind(method) {
                    ensure!(
                        p["threadId"] == thread && p["turnId"] == turn,
                        "approval scope mismatch"
                    );
                    let item = items
                        .get(p["itemId"].as_str().unwrap_or(""))
                        .cloned()
                        .unwrap_or(Value::Null);
                    let token = hash(&serde_json::to_vec(&(&thread, &turn, id, p, &item))?);
                    self.session.status = RunStatus::AwaitingApproval;
                    self.event("approval_required",json!({"candidate":{"id":token,"action":p},"diff":item,"mode":"codex_delegated","kind":kind,"scope":"Upstream exact request. An approval may permit execution beyond read-only sandbox. Nerve cannot enforce native file/command restrictions inside Codex."}))?;
                    let mut accept = false;
                    if self.interactive {
                        loop {
                            tokio::select! {biased;_=self.cancel.cancelled()=>break,msg=self.inputs.recv()=>match msg{Some(UiInput::Approve(v))if v==token=>{accept=true;break},Some(UiInput::Deny(v))if v==token=>break,None=>break,_=>{}}}
                        }
                    }
                    // Persistent grants and changed-root permissions are unsupported.
                    if kind == "file" && !p["grantRoot"].is_null() {
                        accept = false;
                    }
                    self.event(
                        if accept { "approved" } else { "denied" },
                        json!({"candidate":token,"upstream_request":id,"mode":"codex_delegated"}),
                    )?;
                    self.session.status = RunStatus::Running;
                    json!({"id":id,"result":{"decision":if accept{"accept"}else{"cancel"}}})
                } else if method == "item/permissions/requestApproval" {
                    json!({"id":id,"result":{"permissions":{},"scope":"turn"}})
                } else {
                    json!({"id":id,"error":{"code":-32601,"message":"Unsupported client request; no local execution"}})
                };
                answered.remember(&event, response.clone());
                rpc.send(response).await?;
                continue;
            }
            if method.contains("reasoning") || method.starts_with("account/") {
                continue;
            }
            if p.get("threadId").is_some() && p["threadId"] != thread {
                continue;
            }
            if p.get("turnId").is_some() && p["turnId"] != turn {
                continue;
            }
            if method == "item/started" || method == "item/completed" {
                if let Some(id) = p["item"]["id"].as_str() {
                    items.insert(id.into(), p["item"].clone());
                }
                if p["item"]["type"] == "fileChange" {
                    check_seen = false;
                }
                if method == "item/completed"
                    && p["item"]["type"] == "commandExecution"
                    && p["item"]["exitCode"] == 0
                {
                    let command = p["item"]["command"].as_str().unwrap_or("");
                    if command.contains("test") || command.contains("check") {
                        check_seen = true;
                    }
                }
            }
            if p["item"]["type"] == "reasoning" {
                continue;
            }
            if ![
                "thread/started",
                "turn/started",
                "turn/completed",
                "item/started",
                "item/completed",
                "item/agentMessage/delta",
                "item/commandExecution/outputDelta",
                "item/fileChange/outputDelta",
                "error",
                "thread/tokenUsage/updated",
            ]
            .contains(&method)
            {
                continue;
            }
            self.event(
                "upstream",
                json!({"method":method,"params":p,"execution_owner":"official Codex"}),
            )?;
            if method == "turn/completed" && p["turn"]["id"] == turn {
                self.session.recovery_needed = false;
                self.session.status = match p["turn"]["status"].as_str() {
                    Some("completed") if check_seen => RunStatus::Completed,
                    Some("interrupted") => RunStatus::Cancelled,
                    Some("failed") => RunStatus::Failed,
                    _ => RunStatus::Blocked,
                };
                self.event("delegation_finished",json!({"upstream_status":p["turn"]["status"],"observed_successful_check_after_last_edit":check_seen,"verification_owner":"upstream; command-name heuristic is not proof"}))?;
                return Ok(());
            }
        }
    }
}
