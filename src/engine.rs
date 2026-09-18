use crate::{context, domain::*, generation::Generator, policy, session::Store, tools::Workspace};
use anyhow::{Result, bail, ensure};
use serde_json::json;
use std::{path::Path, sync::Arc, time::Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct Engine {
    pub store: Store,
    pub session: Session,
    pub workspace: Workspace,
    pub generator: Arc<dyn Generator>,
    pub cancel: CancellationToken,
    pub events: mpsc::UnboundedSender<RunEvent>,
    pub input: mpsc::UnboundedReceiver<UiInput>,
    pub interactive: bool,
    pub approved: Option<String>,
}

impl Engine {
    pub fn event(&mut self, kind: &str, data: serde_json::Value) -> Result<()> {
        let e = self.store.record(&mut self.session, kind, data)?;
        let _ = self.events.send(e);
        Ok(())
    }
    pub async fn run(mut self) -> Result<Session> {
        let started = Instant::now();
        let result = self.drive().await;
        self.session.metrics.elapsed_ms += started.elapsed().as_millis() as u64;
        if let Err(e) = result {
            self.session.status = if self.cancel.is_cancelled() {
                RunStatus::Cancelled
            } else {
                RunStatus::Failed
            };
            self.event("error", json!({"message":e.to_string()}))?;
        }
        self.event("summary",json!({"status":self.session.status,"verification":self.session.verified,"metrics":self.session.metrics,"note":"Checks passed is evidence, not proof of full correctness."}))?;
        Ok(self.session)
    }
    async fn drive(&mut self) -> Result<()> {
        ensure!(
            self.session.config.mode == Mode::Native,
            "native engine cannot own delegated mode"
        );
        if self.session.inflight.is_some() || self.store.dir.join("patch-recovery.json").exists() {
            self.session.status = RunStatus::Blocked;
            self.event("recovery_required",json!({"message":"An interrupted action may have completed. Inspect workspace and trace, recover any patch journal, then resume with --acknowledge-interruption. No automatic replay."}))?;
            return Ok(());
        }
        if self.session.status == RunStatus::Completed {
            return Ok(());
        }
        self.session.status = RunStatus::Running;
        self.event("started",json!({"mode":"native","decision":self.session.config.decision,"model":self.session.config.generation_model,"simulation":self.session.config.offline_demo,"session":self.session.id}))?;
        while self.session.steps < self.session.config.max_steps {
            if self.cancel.is_cancelled() {
                self.session.status = RunStatus::Cancelled;
                break;
            }
            let revision = self.workspace.revision()?;
            let chosen = if let Some(pending) = self.session.pending.clone() {
                pending
            } else {
                let dropped = context::compact(&mut self.session, &self.store, None)?;
                if !dropped.is_empty() {
                    self.session.metrics.context_invalidations += 1;
                    self.event("context_evicted",json!({"policy":"conservative","artifacts":dropped,"cache_prefix_changed":true}))?;
                }
                let candidates = self.construct(&revision)?;
                self.event("candidates", json!({"candidates":candidates}))?;
                let chosen = candidates
                    .iter()
                    .find(|c| c.class != PolicyClass::Deny)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("no admissible action"))?;
                self.event("selection",json!({"candidate":chosen.id,"source":"deterministic_rules","explanation":"First admissible evidence or proposed action; model confidence does not grant permission."}))?;
                chosen
            };
            if let Err(e) = policy::revalidate(&chosen, &revision) {
                self.session.pending = None;
                self.session.proposals.clear();
                self.event("stale_rejected", json!({"message":e.to_string()}))?;
                self.session.steps += 1;
                continue;
            }
            let fingerprint = if matches!(chosen.action, Action::AskGenerator) {
                crate::session::hash(&serde_json::to_vec(&(&chosen.id, &self.session.context))?)
            } else {
                chosen.id.clone()
            };
            if self.session.seen.get(&fingerprint).copied().unwrap_or(0) >= 2 {
                self.session.status = RunStatus::Blocked;
                self.event("no_progress",json!({"candidate":fingerprint,"message":"Repeated action on identical state; narrow the task or provide new evidence."}))?;
                break;
            }
            if !self.approve(&chosen).await? {
                break;
            }
            if self.cancel.is_cancelled() {
                self.session.status = RunStatus::Cancelled;
                break;
            }
            // Approval, network waits, and user edits can change the workspace.
            policy::revalidate(&chosen, &self.workspace.revision()?)?;
            self.session.pending = None;
            self.session.steps += 1;
            *self.session.seen.entry(fingerprint).or_default() += 1;
            match &chosen.action {
                Action::AskGenerator => {
                    if self.session.metrics.generative_calls + self.session.metrics.simulated_turns
                        >= self.session.config.max_generations
                    {
                        self.session.status = RunStatus::BudgetExhausted;
                        break;
                    }
                    if self.generator.simulated() {
                        self.session.metrics.simulated_turns += 1;
                    } else {
                        self.session.metrics.generative_calls += 1;
                    }
                    self.event("generation_requested",json!({"purpose":"plan_or_propose_next_action","simulated":self.generator.simulated()}))?;
                    let input = context::render(&self.session, &self.store)?;
                    let (tx, mut rx) = mpsc::unbounded_channel();
                    let generator = Arc::clone(&self.generator);
                    let cancel = self.cancel.clone();
                    let future = generator.generate(input, &cancel, tx);
                    tokio::pin!(future);
                    let response = loop {
                        tokio::select! {result=&mut future=>break result?,Some(delta)=rx.recv()=>{let _=self.events.send(RunEvent{seq:0,session:self.session.id.clone(),kind:"stream".into(),data:json!({"delta":self.store.redactor.text(&delta)})});}}
                    };
                    self.session.metrics.usage.push(response.usage.clone());
                    self.session.proposals = response.proposal.actions.clone();
                    self.event("proposal",json!({"message":response.proposal.message,"actions":response.proposal.actions,"model":response.model,"usage":response.usage}))?;
                }
                Action::Finish { summary } => {
                    if self
                        .session
                        .verified
                        .as_ref()
                        .is_some_and(|v| v.exit_code == 0 && v.revision == revision)
                    {
                        self.session.status = RunStatus::Completed;
                        self.event("completed", json!({"summary":summary,"checks_passed":true}))?;
                        break;
                    }
                    self.session.proposals.clear();
                    self.event("completion_rejected",json!({"reason":"No successful verification for the current workspace. Run a relevant check."}))?;
                }
                Action::Blocked { reason } => {
                    self.session.status = RunStatus::Blocked;
                    self.event("blocked", json!({"reason":reason}))?;
                    break;
                }
                Action::Rehydrate { artifact } => {
                    context::rehydrate(&mut self.session, artifact, &self.store)?;
                    self.session.proposals.clear();
                    self.event("rehydrated", json!({"artifact":artifact,"rerun":false}))?;
                }
                action => {
                    self.session.inflight = Some(chosen.clone());
                    self.session.metrics.tool_calls += 1;
                    self.event("tool_started", json!({"candidate":chosen}))?;
                    let result = self
                        .workspace
                        .execute(action, &self.store, &self.cancel)
                        .await;
                    match result {
                        Ok(result) => {
                            let bytes = self.store.redactor.text(&result.text).into_bytes();
                            let artifact = self.store.put(
                                &bytes,
                                "native_tool",
                                &chosen.id,
                                &revision,
                                vec![],
                            )?;
                            let pinned = matches!(action, Action::Patch { .. })
                                || matches!(action,Action::Read{path,..} if path.ends_with("AGENTS.md"));
                            if let Action::Run {
                                argv,
                                verification: true,
                            } = action
                            {
                                let after = self.workspace.revision()?;
                                if result.exit_code == Some(0) && after == revision {
                                    self.session.verified = Some(Verification {
                                        argv: argv.clone(),
                                        revision: after,
                                        artifact: artifact.hash.clone(),
                                        exit_code: 0,
                                    });
                                    for c in &mut self.session.context {
                                        c.diagnostic = false;
                                    }
                                } else {
                                    self.session.verified = None;
                                }
                            }
                            if matches!(action, Action::Patch { .. }) {
                                self.session.verified = None;
                            }
                            self.session.context.push(ContextItem {
                                artifact: artifact.clone(),
                                action: action.clone(),
                                pinned,
                                evicted: false,
                                diagnostic: result.diagnostic,
                            });
                            self.session.inflight = None;
                            self.session.proposals.clear();
                            self.event("tool_result",json!({"call_id":chosen.id,"artifact":artifact,"content":String::from_utf8_lossy(&bytes),"exit_code":result.exit_code}))?;
                        }
                        Err(e) => {
                            if self.cancel.is_cancelled() {
                                return Err(e);
                            }
                            // Effects may have happened; only known read failures are safe to retry.
                            if matches!(action, Action::Patch { .. } | Action::Run { .. }) {
                                return Err(e);
                            }
                            self.session.inflight = None;
                            self.session.proposals.clear();
                            self.event(
                                "tool_error",
                                json!({"candidate":chosen.id,"message":e.to_string()}),
                            )?;
                        }
                    }
                }
            }
            self.store.save(&self.session)?;
        }
        if self.session.status == RunStatus::Running {
            self.session.status = RunStatus::BudgetExhausted;
        }
        Ok(())
    }
    fn construct(&self, revision: &str) -> Result<Vec<CandidateAction>> {
        let evidence = self
            .session
            .context
            .iter()
            .rev()
            .take(4)
            .map(|c| c.artifact.hash.clone())
            .collect::<Vec<_>>();
        let mut actions = self.session.proposals.clone();
        let mut provenance = "generation proposal";
        if self.session.context.is_empty() {
            actions = vec![Action::List];
            provenance = "bounded repository discovery";
        }
        if actions.is_empty() {
            let files = self.workspace.files()?;
            if self.session.context.len() == 1 {
                if files.contains(&"AGENTS.md".into()) {
                    actions.push(Action::Read {
                        path: "AGENTS.md".into(),
                        start: 1,
                        lines: 300,
                    });
                }
                for word in self
                    .session
                    .task
                    .split_whitespace()
                    .filter(|w| w.len() >= 4 && w.len() <= 80)
                    .take(4)
                {
                    let literal =
                        word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '.');
                    if !literal.is_empty() {
                        actions.push(Action::Search {
                            query: literal.into(),
                        });
                    }
                }
                provenance = "literal user identifiers and repository instructions";
            }
        }
        // Always provide an escape route. A forced initial discovery is deterministic.
        if !matches!(actions.as_slice(), [Action::List]) {
            actions.push(Action::AskGenerator);
        }
        if actions.is_empty() {
            actions.push(Action::AskGenerator);
        }
        Ok(actions
            .into_iter()
            .take(8)
            .map(|a| policy::candidate(a, revision, provenance, evidence.clone()))
            .collect())
    }
    async fn approve(&mut self, c: &CandidateAction) -> Result<bool> {
        if c.class == PolicyClass::Deny {
            bail!("policy denied action")
        }
        if c.class == PolicyClass::Allow {
            return Ok(true);
        }
        self.session.pending = Some(c.clone());
        self.session.status = RunStatus::AwaitingApproval;
        let diff = if let Action::Patch { edits } = &c.action {
            Some(self.workspace.diff(edits)?)
        } else {
            None
        };
        self.event("approval_required",json!({"candidate":c,"diff":diff,"scope":"Exact action, workspace, policy and preconditions. Processes execute repository code without OS isolation."}))?;
        if self.approved.as_deref() == Some(&c.id) {
            self.approved = None;
            self.session.status = RunStatus::Running;
            self.event(
                "approved",
                json!({"candidate":c.id,"source":"explicit CLI id"}),
            )?;
            return Ok(true);
        }
        if !self.interactive {
            return Ok(false);
        }
        loop {
            tokio::select! {biased;_=self.cancel.cancelled()=>{self.session.status=RunStatus::Cancelled;return Ok(false)},input=self.input.recv()=>match input{
                Some(UiInput::Approve(id)) if id==c.id=>{self.session.status=RunStatus::Running;self.event("approved",json!({"candidate":id,"source":"terminal"}))?;return Ok(true)},
                Some(UiInput::Deny(id)) if id==c.id=>{self.session.status=RunStatus::Blocked;self.session.pending=None;self.event("denied",json!({"candidate":id}))?;return Ok(false)},
                None=>return Ok(false),_=>{}
            }}
        }
    }
}

pub fn workspace_for(s: &Session) -> Result<Workspace> {
    Workspace::new(Path::new(&s.workspace), s.config.exclusions.clone())
}
