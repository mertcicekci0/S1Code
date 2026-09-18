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
            } else if e.to_string().contains("budget") {
                RunStatus::BudgetExhausted
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
            [
                self.session.config.jev_confidence,
                self.session.config.jev_retention_threshold
            ]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "experimental decision thresholds must be finite values in 0..1"
        );
        ensure!(
            self.session.config.mode == Mode::Native,
            "native engine cannot own delegated mode"
        );
        if self.session.recovery_needed
            || self.session.inflight.is_some()
            || self.store.dir.join("patch-recovery.json").exists()
        {
            self.session.status = RunStatus::Blocked;
            self.event("recovery_required",json!({"message":"An interrupted action may have completed. Inspect workspace and trace, recover any patch journal, then resume with --acknowledge-interruption. No automatic replay."}))?;
            return Ok(());
        }
        if self.session.status == RunStatus::Completed {
            if !self
                .session
                .verified
                .as_ref()
                .is_some_and(|v| self.workspace.revision().is_ok_and(|r| r == v.revision))
            {
                self.session.verified = None;
                self.session.status = RunStatus::Blocked;
                self.event("verification_stale", json!({"message":"Workspace changed since completion; previous checks do not verify its current state. Start a new task to verify the changes."}))?;
            }
            return Ok(());
        }
        let mut jev =
            if self.session.config.decision == "jev" || self.session.config.eviction == "jev" {
                Some(crate::decisions::Jev::from_config(&self.session.config)?)
            } else {
                None
            };
        if let Some(j) = &mut jev {
            j.max_requests = self.session.config.max_provider_requests;
        }
        self.session.status = RunStatus::Running;
        self.event("started",json!({"mode":"native","decision":self.session.config.decision,"decision_provider":self.session.config.jev_provider,"model":self.session.config.generation_model,"generation_provider":self.session.config.generation_provider,"simulation":self.session.config.offline_demo,"session":self.session.id,"task":self.session.task}))?;
        while self.session.steps < self.session.config.max_steps {
            if self.cancel.is_cancelled() {
                self.session.status = RunStatus::Cancelled;
                break;
            }
            let revision = self.workspace.revision()?;
            self.session.current_revision = revision.clone();
            let chosen = if let Some(pending) = self.session.pending.clone() {
                pending
            } else {
                self.compact_context(&mut jev).await?;
                let candidates = self.construct(&revision)?;
                self.event("candidates", json!({"candidates":candidates}))?;
                self.select(candidates, &mut jev).await?
            };
            if let Err(e) = policy::revalidate(&chosen, &revision) {
                self.session.pending = None;
                self.session.proposals.clear();
                self.event("stale_rejected", json!({"message":e.to_string()}))?;
                self.session.steps += 1;
                continue;
            }
            let fingerprint = if matches!(chosen.action, Action::AskGenerator) {
                crate::session::hash(&serde_json::to_vec(&(
                    &chosen.id,
                    &self.session.context,
                    crate::generation::CONTRACT_VERSION,
                ))?)
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
                    if self.session.metrics.generative_calls
                        + self.session.metrics.decision_requests
                        >= self.session.config.max_provider_requests
                    {
                        self.session.status = RunStatus::BudgetExhausted;
                        break;
                    }
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
                    let mut display =
                        crate::privacy::StreamRedactor::new(self.store.redactor.clone());
                    let generator = Arc::clone(&self.generator);
                    let cancel = self.cancel.clone();
                    let future = generator.generate(input, &cancel, tx);
                    tokio::pin!(future);
                    let response = loop {
                        tokio::select! {result=&mut future=>break result?,Some(delta)=rx.recv()=>{let _=self.events.send(RunEvent{seq:0,session:self.session.id.clone(),kind:"stream".into(),data:json!({"delta":display.push(&delta)})});}}
                    };
                    while let Ok(delta) = rx.try_recv() {
                        let _ = self.events.send(RunEvent {
                            seq: 0,
                            session: self.session.id.clone(),
                            kind: "stream".into(),
                            data: json!({"delta":display.push(&delta)}),
                        });
                    }
                    let _ = self.events.send(RunEvent {
                        seq: 0,
                        session: self.session.id.clone(),
                        kind: "stream".into(),
                        data: json!({"delta":display.finish()}),
                    });
                    ensure!(
                        !self
                            .store
                            .redactor
                            .contains_secret(&serde_json::to_string(&response.proposal)?),
                        "provider proposal contained a known secret; refusing persistence/execution"
                    );
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
                                if matches!(action, Action::Patch { .. }) {
                                    chosen.evidence.clone()
                                } else {
                                    vec![]
                                },
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
                            let message = self.store.redactor.text(&e.to_string());
                            let artifact = self.store.put(
                                message.as_bytes(),
                                "native_tool_error",
                                &chosen.id,
                                &revision,
                                vec![],
                            )?;
                            self.session.context.push(ContextItem {
                                artifact,
                                action: action.clone(),
                                pinned: false,
                                evicted: false,
                                diagnostic: true,
                            });
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
        if actions.is_empty()
            && let Some(last) = self.session.context.last()
            && matches!(last.action, Action::Search { .. })
        {
            let bytes = self.store.get(&last.artifact.hash)?;
            for line in String::from_utf8_lossy(&bytes).lines().take(4) {
                let mut fields = line.splitn(3, ':');
                if let (Some(path), Some(number)) = (fields.next(), fields.next())
                    && let Ok(n) = number.parse::<usize>()
                    && self.workspace.path(path, false).is_ok()
                {
                    actions.push(Action::Read {
                        path: path.into(),
                        start: n.saturating_sub(10).max(1),
                        lines: 80,
                    });
                }
            }
            provenance = "literal search results";
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
            .map(|a| {
                let validation = match &a {
                    Action::Read { path, start, lines } => {
                        self.workspace.path(path, false).and_then(|_| {
                            ensure!(
                                *start > 0 && *lines > 0 && *lines <= 300,
                                "invalid read range"
                            );
                            Ok(())
                        })
                    }
                    Action::Search { query } => {
                        if query.is_empty() || query.len() > 256 {
                            Err(anyhow::anyhow!("invalid literal search"))
                        } else {
                            Ok(())
                        }
                    }
                    Action::Patch { edits } => self.workspace.validate_edits(edits).map(|_| ()),
                    Action::Rehydrate { artifact } => {
                        if self
                            .session
                            .context
                            .iter()
                            .any(|c| c.artifact.hash == *artifact)
                        {
                            self.store.get(artifact).map(|_| ())
                        } else {
                            Err(anyhow::anyhow!(
                                "artifact is outside current session context"
                            ))
                        }
                    }
                    _ => Ok(()),
                };
                let mut candidate = policy::candidate(a, revision, provenance, evidence.clone());
                if let Err(error) = validation {
                    candidate.class = PolicyClass::Deny;
                    candidate.provenance = format!("deterministic input rejection: {error}");
                }
                candidate
            })
            .collect())
    }
    async fn select(
        &mut self,
        candidates: Vec<CandidateAction>,
        jev: &mut Option<crate::decisions::Jev>,
    ) -> Result<CandidateAction> {
        use crate::decisions::{Answer, Question};
        let admissible: Vec<_> = candidates
            .into_iter()
            .filter(|c| c.class != PolicyClass::Deny)
            .collect();
        let first = admissible
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no admissible candidate"))?;
        let ambiguous = admissible
            .iter()
            .filter(|c| !matches!(c.action, Action::AskGenerator))
            .count()
            > 1;
        if !ambiguous || self.session.config.decision == "rules" {
            self.event(
                "selection",
                json!({"candidate":first.id,"source":"deterministic_rules","forced":!ambiguous}),
            )?;
            return Ok(first);
        }
        let mut state = context::render(&self.session, &self.store)?;
        state["candidates"] = serde_json::to_value(&admissible)?;
        state["policy_version"] = crate::brand::POLICY_VERSION.into();
        if self.session.config.decision == "generative" {
            ensure!(
                self.session.metrics.generative_calls + self.session.metrics.decision_requests
                    < self.session.config.max_provider_requests,
                "provider request budget exhausted"
            );
            ensure!(
                self.session.metrics.generative_calls < self.session.config.max_generations,
                "generative decision budget exhausted"
            );
            self.session.metrics.generative_calls += 1;
            state["selection_contract"] = "Return exactly ONE of the supplied candidate actions, byte-equivalent in arguments. Select the most relevant next action; ask_generator is the escape route. Do not invent an action or execute tools.".into();
            self.event(
                "generation_requested",
                json!({"purpose":"constrained_decision_baseline"}),
            )?;
            let (tx, _rx) = mpsc::unbounded_channel();
            let result = self.generator.generate(state, &self.cancel, tx).await?;
            self.session.metrics.usage.push(result.usage);
            ensure!(
                result.proposal.actions.len() == 1,
                "baseline must select one action"
            );
            let selected = admissible
                .iter()
                .find(|c| c.action == result.proposal.actions[0])
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("baseline selected outside candidate set"))?;
            self.event("selection",json!({"source":"constrained_generative","candidate":selected.id,"model":result.model}))?;
            return Ok(selected);
        }
        let criteria = admissible
            .iter()
            .map(|c| {
                (
                    c.id.clone(),
                    Some(serde_json::to_string(&c.action).unwrap()),
                )
            })
            .collect();
        let questions=std::collections::BTreeMap::from([("next_action".into(),Question::Choice{instructions:"Select the most useful concrete next action for the user's task from the candidates and evidence. Prefer additional generation when the candidates lack sufficient evidence or complete arguments. Scores never grant permission.".into(),criteria})]);
        self.event("decision_requested",json!({"purpose":"action_relevance","questions":1,"model":self.session.config.jev_model}))?;
        let result = jev
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Jev is not configured"))?
            .ask(state, questions, &self.cancel, &mut self.session.metrics)
            .await;
        match result {
            Ok(response) => {
                let Answer::Choice {
                    choice,
                    confidence,
                    probabilities,
                } = &response.answers["next_action"]
                else {
                    bail!("expected Choice")
                };
                let preferred = admissible.iter().find(|c| c.id == *choice);
                let low_confidence = *confidence < self.session.config.jev_confidence;
                let evidence = untried_evidence(preferred, &admissible, &self.session.seen);
                let selection = if low_confidence {
                    evidence.or_else(|| {
                        admissible
                            .iter()
                            .find(|c| matches!(c.action, Action::AskGenerator))
                    })
                } else {
                    preferred
                }
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("selection unavailable"))?;
                let evidence_fallback = low_confidence && safe_evidence_action(&selection);
                self.event("selection",json!({"source":if evidence_fallback {"jev_low_confidence_evidence"} else {"jev"},"candidate":selection.id,"choice":choice,"selection_probability":probabilities[choice],"distribution_confidence":confidence,"response":response,"threshold_experimental":true,"readonly_evidence_progress":evidence_fallback}))?;
                Ok(selection)
            }
            Err(e) => {
                self.event("decision_failed",json!({"error":e.to_string(),"fallback_rules":self.session.config.jev_fallback_rules}))?;
                if self.cancel.is_cancelled() || !self.session.config.jev_fallback_rules {
                    return Err(e);
                }
                self.event(
                    "selection",
                    json!({"source":"explicit_rules_fallback","candidate":first.id}),
                )?;
                Ok(first)
            }
        }
    }
    async fn compact_context(&mut self, jev: &mut Option<crate::decisions::Jev>) -> Result<()> {
        use crate::decisions::{Answer, Question};
        let size = context::render(&self.session, &self.store)?
            .to_string()
            .len();
        if self.session.config.eviction == "off" {
            ensure!(
                size <= self.session.config.context_bytes,
                "context budget exhausted; eviction disabled"
            );
            return Ok(());
        }
        if size <= self.session.config.context_bytes * 85 / 100 {
            return Ok(());
        }
        let mut preferred = None;
        let mut source = self.session.config.eviction.clone();
        if source == "jev" {
            let state = context::excerpts(&self.session, &self.store)?;
            let mut drop_ids = std::collections::BTreeSet::new();
            let eligible = context::eligible(&self.session);
            let mut failed = None;
            // Independent retention questions share one snapshot; bound each batch.
            for batch in eligible.chunks(8) {
                let questions=batch.iter().map(|i|{let id=self.session.context[*i].artifact.hash.clone();(id.clone(),Question::Noul{instructions:format!("Does the full evidence artifact {id}, shown with actual excerpt and diagnostic lines in the state, need to stay active for the current task? Yes means retain. No means it can be evicted and retrieved exactly later.")})}).collect();
                self.event(
                    "decision_requested",
                    json!({"purpose":"context_retention","questions":batch.len()}),
                )?;
                match jev
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Jev unavailable"))?
                    .ask(
                        state.clone(),
                        questions,
                        &self.cancel,
                        &mut self.session.metrics,
                    )
                    .await
                {
                    Ok(response) => {
                        for (id, answer) in &response.answers {
                            if let Answer::Noul { noul } = answer
                                && *noul < self.session.config.jev_retention_threshold
                            {
                                drop_ids.insert(id.clone());
                            }
                        }
                        self.event(
                            "eviction_decision",
                            json!({"response":response,"noul_is_not_confidence":true,"experimental_retention_threshold":self.session.config.jev_retention_threshold}),
                        )?;
                    }
                    Err(e) => {
                        failed = Some(e);
                        break;
                    }
                }
            }
            if let Some(e) = failed {
                self.event("eviction_failed",json!({"error":e.to_string(),"fallback_rules":self.session.config.jev_fallback_rules}))?;
                if !self.session.config.jev_fallback_rules || self.cancel.is_cancelled() {
                    return Err(e);
                }
                source = "explicit_conservative_fallback".into();
            } else {
                preferred = Some(drop_ids);
            }
        }
        let dropped = context::compact(&mut self.session, &self.store, preferred.as_ref())?;
        if !dropped.is_empty() {
            self.session.metrics.context_invalidations += 1;
            self.event("context_evicted",json!({"policy":source,"artifacts":dropped,"cache_prefix_changed":true,"bytes_before":size,"bytes_after":context::render(&self.session,&self.store)?.to_string().len()}))?;
        }
        Ok(())
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

fn untried_evidence<'a>(
    preferred: Option<&'a CandidateAction>,
    candidates: &'a [CandidateAction],
    seen: &std::collections::BTreeMap<String, usize>,
) -> Option<&'a CandidateAction> {
    preferred
        .filter(|c| safe_evidence_action(c) && !seen.contains_key(&c.id))
        .or_else(|| {
            candidates
                .iter()
                .find(|c| safe_evidence_action(c) && !seen.contains_key(&c.id))
        })
}

// Confidence is not permission or correctness. A validated, policy-allowed evidence
// action can resolve uncertainty without another generation call.
fn safe_evidence_action(candidate: &CandidateAction) -> bool {
    candidate.class == PolicyClass::Allow
        && matches!(
            candidate.action,
            Action::Read { .. } | Action::Search { .. } | Action::Rehydrate { .. }
        )
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn uncertain_patch_can_gather_untried_evidence_without_replanning() {
        let patch = policy::candidate(
            Action::Patch { edits: vec![] },
            "revision",
            "fixture",
            vec![],
        );
        let read = policy::candidate(
            Action::Read {
                path: "parser.py".into(),
                start: 1,
                lines: 40,
            },
            "revision",
            "fixture",
            vec![],
        );
        let candidates = vec![patch, read];
        let mut seen = std::collections::BTreeMap::new();
        assert_eq!(
            untried_evidence(Some(&candidates[0]), &candidates, &seen)
                .unwrap()
                .id,
            candidates[1].id
        );
        seen.insert(candidates[1].id.clone(), 1);
        assert!(untried_evidence(Some(&candidates[0]), &candidates, &seen).is_none());
    }
    #[test]
    fn only_allowed_evidence_actions_bypass_confidence_escalation() {
        let read = policy::candidate(
            Action::Read {
                path: "parser.py".into(),
                start: 1,
                lines: 40,
            },
            "revision",
            "fixture",
            vec![],
        );
        assert!(safe_evidence_action(&read));
        let mut denied = read.clone();
        denied.class = PolicyClass::Deny;
        assert!(!safe_evidence_action(&denied));
        for action in [
            Action::AskGenerator,
            Action::List,
            Action::Patch { edits: vec![] },
            Action::Run {
                argv: vec!["python3".into(), "-m".into(), "unittest".into()],
                verification: true,
            },
        ] {
            assert!(!safe_evidence_action(&policy::candidate(
                action,
                "revision",
                "fixture",
                vec![]
            )));
        }
    }
}
