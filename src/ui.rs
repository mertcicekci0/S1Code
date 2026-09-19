//! Human terminal presentation; JSON remains an explicit diagnostic view and headless format.
use crate::{
    brand,
    domain::{RunEvent, UiInput},
};
use anyhow::Result;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    io,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const ACCENT: Color = Color::Rgb(111, 211, 194);
const MUTED: Color = Color::Rgb(151, 163, 177);
const GOLD: Color = Color::Rgb(242, 199, 105);
pub(crate) struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            crossterm::event::DisableBracketedPaste,
            crossterm::cursor::Show,
            LeaveAlternateScreen
        );
    }
}
fn string<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn short(s: &str) -> String {
    s.chars().take(10).collect()
}
fn quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_/.:=".contains(c))
    {
        s.into()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
fn command(v: &Value) -> String {
    if let Some(args) = v["argv"].as_array() {
        args.iter()
            .map(|v| quote(v.as_str().unwrap_or("")))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        string(v, "command").into()
    }
}
fn action_text(a: &Value) -> String {
    match string(a, "type") {
        "list" => "Discover repository files".into(),
        "search" => format!("Search for {}", string(a, "query")),
        "read" => format!(
            "Read {} · from line {} · {} lines",
            string(a, "path"),
            a["start"],
            a["lines"]
        ),
        "run" => format!(
            "{}\n{}",
            if a["verification"] == true {
                "Run tests / verification"
            } else {
                "Run command"
            },
            command(a)
        ),
        "replace" => format!("Replace exact snippet in {}", string(a, "path")),
        "patch" => format!(
            "Apply patch\n{}",
            a["edits"]
                .as_array()
                .map(|es| es
                    .iter()
                    .map(|e| format!("  {}", string(e, "path")))
                    .collect::<Vec<_>>()
                    .join("\n"))
                .unwrap_or_default()
        ),
        "git" => "Inspect git status and diff".into(),
        "rehydrate" => format!("Restore saved evidence {}", short(string(a, "artifact"))),
        "ask_generator" => "Ask generation provider for the next step".into(),
        "finish" => format!("Finish task\n{}", string(a, "summary")),
        "blocked" => format!("Stop: {}", string(a, "reason")),
        _ if !command(a).is_empty() => format!("Run command through Codex\n{}", command(a)),
        _ => "Review the upstream file change request".into(),
    }
}
fn readable_status(s: &str) -> String {
    s.replace('_', " ")
}

pub fn summary_text(v: &Value) -> String {
    let m = &v["metrics"];
    let status = readable_status(string(v, "status"));
    let check = if v["verification"]["exit_code"] == 0 {
        format!("Checks passed: {}", command(&v["verification"]))
    } else {
        "No current passing verification recorded.".into()
    };
    let simulation = if m["simulated_turns"].as_u64().unwrap_or(0) > 0 {
        "OFFLINE SIMULATION — no model inference\n"
    } else {
        ""
    };
    let counts = if v["mode"] == "codex_delegated" {
        format!(
            "Delegations: {} · internal model calls and tokens unknown",
            m["delegations"]
        )
    } else {
        format!(
            "Tools: {} · generation calls: {} · decision requests: {} · rehydrations: {}",
            m["tool_calls"], m["generative_calls"], m["decision_requests"], m["rehydrations"]
        )
    };
    let budget = if v["status"] == "budget_exhausted" && v["limits"].is_object() {
        format!(
            "\nBudget: {} / {} steps; generation cap {}; combined request cap {}. Resume with explicit higher caps, or narrow the task.",
            v["limits"]["steps"],
            v["limits"]["max_steps"],
            v["limits"]["max_generations"],
            v["limits"]["max_provider_requests"]
        )
    } else {
        String::new()
    };
    format!(
        "{simulation}{status}\n\n{check}\nSession totals · {counts}{budget}\n\nChecks passing is evidence, not proof of full correctness."
    )
}
fn selection_text(v: &Value) -> String {
    let mut out = format!(
        "Selected {}\nMethod: {}",
        short(string(v, "candidate")),
        readable_status(string(v, "source"))
    );
    if !v["action"].is_null() {
        out.push_str(&format!("\n{}", action_text(&v["action"])));
    }
    if !string(v, "provenance").is_empty() {
        out.push_str(&format!("\nEvidence: {}", string(v, "provenance")));
    }
    if let Some(p) = v["selection_probability"].as_f64() {
        out.push_str(&format!("\nSelected-option probability: {p:.3}"));
    }
    if let Some(c) = v["distribution_confidence"].as_f64() {
        out.push_str(&format!(
            "\nDistribution confidence: {c:.3} (not correctness)"
        ));
    }
    out
}
fn candidates_text(v: &Value) -> String {
    v["candidates"]
        .as_array()
        .map(|cs| {
            cs.iter()
                .enumerate()
                .map(|(i, c)| {
                    format!(
                        "{}. {}  [{}]\n{}\nSource: {}",
                        i + 1,
                        short(string(c, "id")),
                        string(c, "class"),
                        action_text(&c["action"]),
                        string(c, "provenance")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default()
}
fn patch_text(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.into();
    }
    let changes = &v["changes"];
    if let Some(a) = changes.as_array() {
        return a
            .iter()
            .map(|c| format!("{}\n{}", string(c, "path"), string(c, "diff")))
            .collect::<Vec<_>>()
            .join("\n");
    }
    "The upstream request did not include a displayable diff. Inspect the upstream request with j before approval.".into()
}
fn approval_action(v: &Value) -> String {
    if v["mode"] == "codex_delegated" && v["kind"] == "command" {
        let mut text = command(&v["candidate"]["action"]);
        if text.is_empty() {
            text = command(&v["diff"]);
        }
        if text.is_empty() {
            text = "Command details missing. Inspect the raw request (j) or deny.".into();
        }
        format!("Run command through Codex\n{text}")
    } else {
        action_text(&v["candidate"]["action"])
    }
}
fn approval_text(v: &Value) -> String {
    let a = &v["candidate"]["action"];
    let mut out = approval_action(v);
    if v["mode"] == "codex_delegated" {
        if !string(a, "cwd").is_empty() {
            out.push_str(&format!("\nDirectory: {}", string(a, "cwd")));
        }
        if !string(a, "reason").is_empty() {
            out.push_str(&format!("\nReason: {}", string(a, "reason")));
        }
        out.push_str(
            "\n\nCodex owns execution. Approval may allow execution beyond its read-only sandbox.",
        );
    } else if a["type"] == "run" {
        out.push_str("\n\nRuns repository code with your user permissions; no OS sandbox.");
    } else {
        out.push_str(
            "\n\nReview the diff. Approval applies only to this patch and current file hashes.",
        );
    }
    out.push_str("\n\ny  Approve once       n  Deny");
    out
}
fn upstream_text(v: &Value) -> String {
    let p = &v["params"];
    let item = &p["item"];
    if !string(p, "delta").is_empty() {
        return string(p, "delta").into();
    }
    match string(item, "type") {
        "agentMessage" => string(item, "text").into(),
        "commandExecution" => format!(
            "{}\nStatus: {} · exit {}\n\n{}",
            command(item),
            string(item, "status"),
            item["exitCode"],
            string(item, "aggregatedOutput")
        ),
        "fileChange" => patch_text(item),
        _ => format!("Codex: {}", string(v, "method").replace('/', " · ")),
    }
}
fn details(e: &RunEvent) -> String {
    let v = &e.data;
    match e.kind.as_str() {
        "started" => format!("Task\n{}\n\nNative execution · {} policy",string(v,"task"),string(v,"decision")),
        "approval_required" => approval_text(v),
        "auto_approved" => format!("Automatically approved by explicit session setting\n{}", action_text(&v["candidate"]["action"])),
        "tool_started" => action_text(&v["candidate"]["action"]),
        "tool_result" => format!("Tool result{}\n\n{}", v["exit_code"].as_i64().map(|c|format!(" · exit {c}")).unwrap_or_default(),string(v,"content")),
        "proposal" => format!("{}\n\n{}",string(v,"message"),v["actions"].as_array().map(|xs|xs.iter().map(action_text).collect::<Vec<_>>().join("\n\n")).unwrap_or_default()),
        "candidates" => candidates_text(v),
        "selection" => selection_text(v),
        "summary" => summary_text(v),
        "generation_requested" => "Preparing the next step from the task and captured evidence…".into(),
        "decision_requested" => format!("Evaluating {} question(s): {}",v["questions"],readable_status(string(v,"purpose"))),
        "upstream" => upstream_text(v),
        "context_evicted" => "Older evidence left active context. Its captured bytes remain available for rehydration.".into(),
        "rehydrated" => format!("Recovered saved artifact {} without rerunning a command.",short(string(v,"artifact"))),
        "approved" => "Approved once. Rechecking the workspace before execution.".into(),
        "denied" => "Action denied. No permission was granted.".into(),
        "completed" => string(v,"summary").into(),
        _ => ["message","reason","error","note"].iter().find_map(|k|v[*k].as_str()).map(str::to_owned).unwrap_or_else(||readable_status(&e.kind)),
    }
}
/// Extract only the visible message from an incomplete structured proposal stream.
fn visible_message(raw: &str) -> String {
    if !raw.trim_start().starts_with('{') {
        return raw.into();
    }
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        return string(&v, "message").into();
    }
    let Some(at) = raw.find("\"message\"") else {
        return String::new();
    };
    let Some((_, rest)) = raw[at + 9..].split_once(':') else {
        return String::new();
    };
    let rest = rest.trim_start();
    if !rest.starts_with('"') {
        return String::new();
    }
    let mut escaped = false;
    for (i, c) in rest.char_indices().skip(1) {
        if c == '"' && !escaped {
            return serde_json::from_str(&rest[..=i]).unwrap_or_default();
        }
        escaped = c == '\\' && !escaped;
    }
    let mut partial = rest.to_owned();
    for _ in 0..8 {
        if let Ok(s) = serde_json::from_str::<String>(&format!("{partial}\"")) {
            return s;
        }
        if partial.pop().is_none() {
            break;
        }
    }
    String::new()
}

#[derive(Default)]
struct Screen {
    inspect: bool,
    conversation: VecDeque<String>,
    awaiting_input: bool,
    closing: bool,
    editor: crate::home::Editor,
    entries: VecDeque<RunEvent>,
    selected: usize,
    list: ListState,
    tab: usize,
    expanded: bool,
    raw: bool,
    scroll: u16,
    pending: Option<RunEvent>,
    summary: Option<RunEvent>,
    status: String,
    task: String,
    candidates: Value,
    selection: Value,
    artifacts: BTreeMap<String, (u64, bool)>,
    diff: String,
    stream: String,
    upstream_stream: bool,
    done: bool,
}
impl Screen {
    fn inspect_view(&mut self, tab: usize) {
        self.inspect = true;
        self.tab = tab;
        self.raw = false;
        self.scroll = 0;
    }
    fn submit_input(&mut self) -> Option<UiInput> {
        let text = self.editor.take();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        match trimmed {
            "/exit" | "/quit" => {
                self.awaiting_input = false;
                self.closing = true;
                Some(UiInput::Close)
            }
            "/help" => {
                self.message("Commands", "Keep typing to continue this saved task.\n/inspect or /activity · tool results and events\n/decisions · candidates and selection\n/context · stored evidence\n/diff · latest proposed patch\n/exit · close task and return\nCtrl+O (or F2) toggles the inspector; Esc returns from it.\nProvider and permission settings are available on the home screen.");
                None
            }
            "/inspect" | "/activity" | "/decisions" | "/context" | "/diff" => {
                self.inspect_view(match trimmed {
                    "/decisions" => 1,
                    "/context" => 2,
                    "/diff" => 3,
                    _ => 0,
                });
                None
            }
            _ if trimmed.starts_with('/') => {
                self.message("Commands", "Unknown task command. /help lists commands; /exit returns to home settings. No provider request was sent.");
                None
            }
            _ => {
                self.awaiting_input = false;
                Some(UiInput::Message(text))
            }
        }
    }
    fn message(&mut self, role: &str, text: &str) {
        if text.is_empty() {
            return;
        }
        self.conversation.push_back(format!(
            "{role}
{text}
"
        ));
        while self.conversation.len() > 100 {
            self.conversation.pop_front();
        }
        self.scroll = 0;
    }
    fn answer_approval(&mut self, approve: bool) -> Option<UiInput> {
        let event = self.pending.take()?;
        let id = string(&event.data["candidate"], "id").to_owned();
        self.status = "Response sent; revalidating".into();
        Some(if approve {
            UiInput::Approve(id)
        } else {
            UiInput::Deny(id)
        })
    }
    fn receive(&mut self, e: RunEvent) {
        let v = &e.data;
        match e.kind.as_str() {
            "session_restored" => {
                if let Some(events) = v["events"].as_array() {
                    for event in events.iter().take(300) {
                        if let Ok(past) = serde_json::from_value::<RunEvent>(event.clone())
                            && past.kind != "session_restored"
                        {
                            self.receive(past);
                        }
                    }
                }
                self.pending = None;
                self.awaiting_input = false;
                self.stream.clear();
                self.status = "Resuming".into();
                self.message(
                    "Session",
                    "Restored recent saved activity. No tool was rerun by restoring this view.",
                );
                return;
            }
            "started" | "delegation_started" => {
                self.task = string(v, "task").into();
                let task = string(v, "task");
                let preview = task.lines().take(3).collect::<Vec<_>>().join("\n");
                let preview: String = preview.chars().take(240).collect();
                let preview = if preview != task {
                    format!("{preview}…\nFull task: Ctrl+O → Activity → started → Enter")
                } else {
                    preview
                };
                self.message("You", &preview);
                self.awaiting_input = false;
                self.status = "Working".into();
            }
            "stream" => {
                self.stream.push_str(string(v, "delta"));
                self.stream = crate::tools::bound(&self.stream, 512 * 1024);
                self.upstream_stream = v["plain_text"].as_bool().unwrap_or(false);
                return;
            }
            "generation_requested" => {
                self.stream.clear();
                self.status = "Preparing next action…".into();
            }
            "generation_incomplete" | "generation_failed" => {
                self.stream.clear();
            }
            "generation_recovery" | "generation_retry" => {
                self.message("Recovery", string(v, "message"));
                self.stream.clear();
            }
            "tool_started" => {
                self.status = "Executing action…".into();
                self.message("Tool", &action_text(&v["candidate"]["action"]));
            }
            "proposal" => {
                if !v["actions"]
                    .as_array()
                    .is_some_and(|actions| actions.len() == 1 && actions[0]["type"] == "answer")
                {
                    self.message("Assistant", string(v, "message"));
                }
                self.stream.clear();
            }
            "answered" => {
                self.message("Assistant", string(v, "message"));
                self.stream.clear();
            }
            "completed" => {
                self.message("Assistant", string(v, "summary"));
                self.stream.clear();
            }
            "input_ready" => {
                self.awaiting_input = true;
                self.status = "Your turn".into();
                if v["native"] == true {
                    self.message("Next", string(v, "message"));
                }
            }
            "input_rejected" => self.message("Input", string(v, "message")),
            "conversation_history" => {
                if let Some(messages) = v["messages"].as_array() {
                    for m in messages {
                        self.message(string(m, "role"), string(m, "text"));
                    }
                }
            }
            "error" | "tool_error" => {
                self.stream.clear();
                self.message("Error", &details(&e));
            }
            "no_progress" | "generation_retry_stopped" => {
                self.message("Stopped", string(v, "message"))
            }
            "blocked" => self.message("Blocked", string(v, "reason")),
            "verification_rejected" => self.message("Verification", string(v, "reason")),
            "candidates" => self.candidates = v.clone(),
            "selection" => self.selection = v.clone(),
            "tool_result" => {
                self.artifacts.insert(
                    string(&v["artifact"], "hash").into(),
                    (v["artifact"]["bytes"].as_u64().unwrap_or(0), false),
                );
                if let Some(code) = v["exit_code"].as_i64() {
                    self.message("Tool result", &if code == 0 {
                        "Command exited successfully.".into()
                    } else {
                        format!("Command failed (exit {code}). Ctrl+O → Activity shows the captured output.")
                    });
                }
            }
            "context_evicted" => {
                if let Some(ids) = v["artifacts"].as_array() {
                    for id in ids {
                        if let Some(a) = self.artifacts.get_mut(id.as_str().unwrap_or("")) {
                            a.1 = true;
                        }
                    }
                }
            }
            "rehydrated" => {
                if let Some(a) = self.artifacts.get_mut(string(v, "artifact")) {
                    a.1 = false;
                }
            }
            "approval_required" => {
                self.pending = Some(e.clone());
                self.status = "Awaiting your approval".into();
                self.expanded = true;
                self.raw = false;
                self.scroll = 0;
                self.tab = 0;
                if !v["diff"].is_null() && v["kind"] != "command" {
                    self.diff = patch_text(&v["diff"]);
                    self.tab = 3;
                }
            }
            "auto_approved" => {
                if !v["diff"].is_null() {
                    self.diff = patch_text(&v["diff"]);
                }
            }
            "approved" | "denied" => {
                self.pending = None;
                self.status = if e.kind == "approved" {
                    "Working"
                } else {
                    "Action denied"
                }
                .into();
                self.expanded = false;
                self.tab = 0;
                self.scroll = 0;
            }
            "summary" => {
                self.status = readable_status(string(v, "status"));
                if v["status"] != "awaiting_input" {
                    self.message("Result", &summary_text(v));
                }
                self.summary = Some(e.clone());
                self.pending = None;
                self.tab = 0;
                self.expanded = true;
                self.raw = false;
                self.scroll = 0;
            }
            "upstream" => {
                if v["method"] == "item/completed" && v["params"]["item"]["type"] == "agentMessage"
                {
                    self.message("Assistant", string(&v["params"]["item"], "text"));
                    self.stream.clear();
                }
                if v["method"] == "error" {
                    self.message("Error", &upstream_text(v));
                }
                if v["method"] == "item/agentMessage/delta" {
                    self.stream.push_str(string(&v["params"], "delta"));
                    self.stream = crate::tools::bound(&self.stream, 32_000);
                    self.upstream_stream = true;
                    return;
                }
            }
            _ => {}
        }
        self.entries.push_back(e);
        if self.entries.len() > 300 {
            self.entries.pop_front();
        }
        self.selected = self.entries.len().saturating_sub(1);
    }
    fn context_text(&self) -> String {
        let active = self.artifacts.values().filter(|a| !a.1).count();
        let bytes: u64 = self.artifacts.values().filter(|a| !a.1).map(|a| a.0).sum();
        let mut text = format!(
            "Active evidence: {active} artifacts · {bytes} captured bytes\nEvicted: {} · canonical bytes remain stored\n\n",
            self.artifacts.len() - active
        );
        for (id, (size, evicted)) in &self.artifacts {
            text.push_str(&format!(
                "{}  {} · {} bytes\n",
                if *evicted { "Evicted" } else { "Active " },
                short(id),
                size
            ));
        }
        text.push_str("\nRehydration restores captured evidence without rerunning commands.\nCaptured bytes are not the full model context/token count.");
        text
    }
    fn draw(&mut self, f: &mut Frame, label: &str) {
        let input_width = f.area().width.saturating_sub(2).max(1) as usize;
        let (input_lines, cursor_row, cursor_col) = self.editor.layout(input_width);
        let footer_height = if self.awaiting_input && !self.inspect {
            (input_lines.len() + 2).clamp(3, 8) as u16
        } else {
            3
        };
        let pending_h = if self.pending.is_some() { 5 } else { 0 };
        let header_height = (1 + Line::raw(label)
            .width()
            .div_ceil(f.area().width.max(1) as usize))
        .min(5) as u16;
        let rows = Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Length(if self.inspect { 2 } else { 0 }),
            Constraint::Length(pending_h),
            Constraint::Min(1),
            Constraint::Length(footer_height),
        ])
        .split(f.area());
        let header = Text::from(vec![
            Line::from(vec![
                Span::styled(
                    format!("{}  ", brand::NAME),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    if self.status.is_empty() {
                        "Starting"
                    } else {
                        &self.status
                    },
                    Style::default().fg(if self.status.eq_ignore_ascii_case("failed") {
                        Color::Red
                    } else if self.pending.is_some() {
                        GOLD
                    } else {
                        Color::White
                    }),
                ),
            ]),
            Line::styled(label, Style::default().fg(MUTED)),
        ]);
        f.render_widget(Paragraph::new(header).wrap(Wrap { trim: false }), rows[0]);
        f.render_widget(
            Tabs::new(["1 Activity", "2 Decide", "3 Context", "4 Diff"])
                .select(self.tab)
                .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            rows[1],
        );
        if let Some(e) = &self.pending {
            let description = approval_action(&e.data);
            let banner = format!(
                "{}\ny  Approve once     n  Deny     4  Review diff",
                description
            );
            f.render_widget(
                Paragraph::new(banner).wrap(Wrap { trim: false }).block(
                    Block::bordered()
                        .title(" Approval required ")
                        .border_style(Style::default().fg(GOLD)),
                ),
                rows[2],
            );
        }
        let body = rows[3];
        if !self.inspect && self.tab != 3 && !self.raw {
            let mut text = self.conversation.iter().cloned().collect::<Vec<_>>().join(
                "
",
            );
            if !self.stream.is_empty() {
                let streaming = if self.upstream_stream {
                    self.stream.clone()
                } else {
                    visible_message(&self.stream)
                };
                if !streaming.is_empty() {
                    text.push_str(&format!("\nAssistant\n{streaming}"));
                }
            }
            if text.is_empty() {
                text = "Connecting…".into();
            }
            let lines: Vec<Line> = text
                .lines()
                .map(|line| {
                    let style = match line {
                        "You" | "Assistant" => {
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                        }
                        "Tool" => Style::default().fg(MUTED),
                        "Error" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        "Recovery" => Style::default().fg(GOLD),
                        _ => Style::default(),
                    };
                    Line::styled(line.to_owned(), style)
                })
                .collect();
            let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
            let end = paragraph
                .line_count(body.width)
                .saturating_sub(body.height as usize)
                .min(u16::MAX as usize) as u16;
            f.render_widget(paragraph.scroll((end.saturating_sub(self.scroll), 0)), body);
        } else if self.tab == 0 && !self.expanded && !self.raw {
            let areas = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(if self.stream.is_empty() { 0 } else { 5 }),
            ])
            .split(body);
            let items: Vec<_> = self
                .entries
                .iter()
                .map(|e| {
                    let first = details(e)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(160)
                        .collect::<String>();
                    let marker = match e.kind.as_str() {
                        "error" | "tool_error" => "!",
                        "approved" | "completed" => "✓",
                        "approval_required" => "?",
                        _ => "·",
                    };
                    ListItem::new(format!("{marker} {first}"))
                })
                .collect();
            self.list.select(Some(self.selected));
            f.render_stateful_widget(
                List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .title(" Activity · Enter for details "),
                    )
                    .highlight_style(Style::default().bg(Color::Rgb(35, 48, 59)).fg(Color::White))
                    .highlight_symbol("› "),
                areas[0],
                &mut self.list,
            );
            if !self.stream.is_empty() {
                let text = if self.upstream_stream {
                    self.stream.clone()
                } else {
                    visible_message(&self.stream)
                };
                f.render_widget(
                    Paragraph::new(if text.is_empty() {
                        "Preparing a proposal…".into()
                    } else {
                        text
                    })
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::TOP).title(" Assistant ")),
                    areas[1],
                );
            }
        } else {
            let (title, text) = if self.raw {
                (
                    " Raw event · j to return ",
                    self.entries
                        .get(self.selected)
                        .map(|e| serde_json::to_string_pretty(&e.data).unwrap_or_default())
                        .unwrap_or_default(),
                )
            } else {
                match self.tab {
                    1 => (
                        " Candidate decisions ",
                        format!(
                            "{}\n\n{}",
                            selection_text(&self.selection),
                            candidates_text(&self.candidates)
                        ),
                    ),
                    2 => (" Recoverable context ", self.context_text()),
                    3 => (
                        " Changes · scroll with PgUp / PgDn ",
                        if self.diff.is_empty() {
                            "No patch proposed yet.".into()
                        } else {
                            self.diff.clone()
                        },
                    ),
                    _ => (
                        " Activity details · Enter to return ",
                        self.entries
                            .get(self.selected)
                            .map(details)
                            .unwrap_or_else(|| "Waiting for engine events…".into()),
                    ),
                }
            };
            let lines: Vec<_> = text
                .lines()
                .map(|line| {
                    Line::styled(
                        line.to_owned(),
                        Style::default().fg(if self.tab == 3 && !self.raw {
                            if line.starts_with('+') {
                                Color::Green
                            } else if line.starts_with('-') {
                                Color::Red
                            } else if line.starts_with("@@") {
                                ACCENT
                            } else {
                                Color::White
                            }
                        } else {
                            Color::White
                        }),
                    )
                })
                .collect();
            f.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll, 0))
                    .block(Block::default().borders(Borders::TOP).title(title)),
                body,
            );
        }
        if self.awaiting_input && !self.inspect {
            let input_rows =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(rows[4]);
            let scroll = cursor_row.saturating_sub(input_rows[0].height.saturating_sub(2) as usize);
            f.render_widget(
                Paragraph::new(input_lines.join("\n"))
                    .scroll((scroll.min(u16::MAX as usize) as u16, 0))
                    .block(Block::default().borders(Borders::TOP).title(" Follow-up ")),
                input_rows[0],
            );
            f.render_widget(
                Paragraph::new("Enter Send · /help · Esc Close · Ctrl+O Inspect")
                    .style(Style::default().fg(MUTED)),
                input_rows[1],
            );
            if input_rows[0].height >= 2 {
                f.set_cursor_position((
                    input_rows[0].x + cursor_col.min(input_width - 1) as u16,
                    input_rows[0].y + 1 + cursor_row.saturating_sub(scroll) as u16,
                ));
            }
            return;
        }
        let footer = if self.done {
            "q Close · Ctrl+O / F2 Inspect · 1–4 Views\nPgUp/PgDn Scroll · text remains copyable"
        } else {
            "Ctrl+O / F2 Inspect · 1–4 Views · Enter Details\nPgUp/PgDn Scroll · Esc Back/Cancel · Ctrl-C Cancel"
        };
        f.render_widget(
            Paragraph::new(footer)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(MUTED))
                .block(Block::default().borders(Borders::TOP)),
            rows[4],
        );
    }
}

pub async fn terminal(
    mut events: mpsc::UnboundedReceiver<RunEvent>,
    input: mpsc::UnboundedSender<UiInput>,
    cancel: CancellationToken,
    label: String,
) -> Result<Option<RunEvent>> {
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        crossterm::event::EnableBracketedPaste
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut keys = EventStream::new();
    let mut screen = Screen::default();
    loop {
        terminal.draw(|f| screen.draw(f, &label))?;
        tokio::select! {
            event=events.recv(),if !screen.done=>match event{Some(e)=>screen.receive(e),None=>{screen.done=true;if screen.closing { break; }}},
            key=keys.next()=>if let Some(Ok(event))=key {
                if let Event::Paste(text) = &event { if screen.awaiting_input { screen.editor.insert(text); } continue; }
                let Event::Key(key) = event else { continue; };
                if key.kind!=KeyEventKind::Press{continue;}
                if key.code == KeyCode::F(2) || (key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL)) { screen.inspect = !screen.inspect; screen.tab=0; screen.raw=false; screen.scroll=0; continue; }
                if screen.inspect && key.code == KeyCode::Esc { screen.inspect=false; screen.tab=0; screen.scroll=0; continue; }
                if screen.awaiting_input && !screen.inspect {
                    match key.code {
                        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => screen.editor.insert("\n"),
                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => { screen.editor.take(); },
                        KeyCode::Esc => { let _=input.send(UiInput::Close); screen.awaiting_input=false; screen.closing=true; }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => { let _=input.send(UiInput::Close); screen.awaiting_input=false; screen.closing=true; }
                        KeyCode::Enter => { if let Some(message) = screen.submit_input() { let _=input.send(message); } }
                        KeyCode::PageUp => screen.scroll=screen.scroll.saturating_add(10),
                        KeyCode::PageDown => screen.scroll=screen.scroll.saturating_sub(10),
                        _ => screen.editor.key(key.code),
                    }
                    continue;
                }
                match key.code {
                KeyCode::Esc|KeyCode::Char('q')=>{if screen.done{break;}cancel.cancel();screen.status="Cancelling; waiting for cleanup".into();},
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL)=>{if screen.done{break;}cancel.cancel();screen.status="Cancelling; waiting for cleanup".into();},
                KeyCode::Char('y')|KeyCode::Char('n')=>{if let Some(msg)=screen.answer_approval(key.code==KeyCode::Char('y')) {let _=input.send(msg);}},
                KeyCode::Char(c @ '1'..='4')=>{screen.inspect=true;screen.tab=(c as u8-b'1')as usize;screen.raw=false;screen.scroll=0;},
                KeyCode::Up=>{screen.selected=screen.selected.saturating_sub(1);screen.scroll=0;},
                KeyCode::Down=>{screen.selected=(screen.selected+1).min(screen.entries.len().saturating_sub(1));screen.scroll=0;},
                KeyCode::Enter=>{screen.expanded = !screen.expanded;screen.raw=false;screen.scroll=0;},
                KeyCode::Char('j')=>{screen.raw = !screen.raw;screen.scroll=0;},
                KeyCode::PageDown=>{screen.scroll=if screen.inspect || screen.tab==3 {screen.scroll.saturating_add(10)} else {screen.scroll.saturating_sub(10)};},
                KeyCode::PageUp=>{screen.scroll=if screen.inspect || screen.tab==3 {screen.scroll.saturating_sub(10)} else {screen.scroll.saturating_add(10)};},
                KeyCode::Home=>screen.scroll=0,
                _=>{}
            }}
        }
    }
    terminal.show_cursor()?;
    Ok(screen.summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use serde_json::json;
    fn event(kind: &str, data: Value) -> RunEvent {
        RunEvent {
            seq: 1,
            session: "fixture".into(),
            kind: kind.into(),
            data,
        }
    }
    fn render(screen: &mut Screen, width: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, 28)).unwrap();
        t.draw(|f| screen.draw(f, "OFFLINE SIMULATION · real tools · no model calls"))
            .unwrap();
        t.backend()
            .buffer()
            .content
            .chunks(width as usize)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
    #[test]
    fn restored_history_cannot_reactivate_old_approvals() {
        let mut screen = Screen::default();
        screen.receive(event("session_restored", serde_json::json!({"events":[
            event("started", serde_json::json!({"task":"Fix input parsing"})),
            event("approval_required", serde_json::json!({"candidate":{"id":"old-action","action":{"type":"run","argv":["node","--test"]}}})),
            event("completed", serde_json::json!({"summary":"Fixed parsing with test coverage."}))
        ]})));
        assert!(screen.pending.is_none());
        assert!(!screen.awaiting_input);
        assert!(screen.answer_approval(true).is_none());
        assert!(
            screen
                .conversation
                .iter()
                .any(|line| line.contains("Fixed parsing with test coverage."))
        );
        assert_eq!(screen.entries.len(), 3);
    }

    #[test]
    fn completion_summary_and_local_commands_stay_in_the_conversation() {
        let mut screen = Screen::default();
        screen.receive(event(
            "completed",
            serde_json::json!({"summary":"Fixed input parsing; four checks passed."}),
        ));
        screen.receive(event("input_ready", serde_json::json!({})));
        assert!(render(&mut screen, 70).contains("Fixed input parsing; four checks passed."));
        for (command, tab) in [
            ("/activity", 0),
            ("/decisions", 1),
            ("/context", 2),
            ("/diff", 3),
        ] {
            screen.editor.insert(command);
            assert!(screen.submit_input().is_none());
            assert!(screen.awaiting_input && screen.inspect);
            assert_eq!(screen.tab, tab);
        }
        screen.inspect = false;
        for command in ["/help", "/provider claude"] {
            screen.editor.insert(command);
            assert!(screen.submit_input().is_none());
            assert!(screen.awaiting_input);
        }
        screen
            .editor
            .insert("Add Unicode coverage\nKeep the API stable");
        assert!(
            matches!(screen.submit_input(), Some(UiInput::Message(text)) if text.contains('\n'))
        );
        assert!(!screen.awaiting_input);
    }

    #[test]
    fn conversation_answers_do_not_render_a_failed_verification_summary() {
        let mut screen = Screen::default();
        screen.receive(event("started", json!({"task":"hi"})));
        screen.receive(event(
            "answered",
            json!({"message":"Hi! What would you like to build?"}),
        ));
        screen.receive(event(
            "summary",
            json!({"status":"awaiting_input","verification":null,"metrics":{}}),
        ));
        screen.receive(event("input_ready", json!({})));
        for width in [50, 110] {
            let view = render(&mut screen, width);
            assert!(view.contains("Hi! What would you like to build?"));
            assert!(view.contains("Follow-up"));
            assert!(!view.contains("No current passing verification"));
        }
        assert!(
            screen.summary.is_some(),
            "the inspector still has full metrics"
        );
    }
    #[test]
    fn native_followup_input_wraps_and_preserves_multiline_text() {
        let mut screen = Screen::default();
        screen.receive(event(
            "summary",
            json!({"status":"completed","metrics":{},"verification":null}),
        ));
        screen.receive(event(
            "input_ready",
            json!({"native":true,"message":"At most 24 additional requests"}),
        ));
        let text = format!(
            "Add mobile controls\n{}\nKeep tests passing",
            "Unicode sınırları ".repeat(30)
        );
        screen.editor.insert(&text);
        for width in [24, 50, 110] {
            let view = render(&mut screen, width);
            assert!(view.contains("Follow-up"));
            assert!(view.contains("Keep tests passing"));
            assert_eq!(screen.editor.text.iter().collect::<String>(), text);
        }
        screen.receive(event(
            "input_rejected",
            json!({"message":"Use a shorter message"}),
        ));
        screen.receive(event("input_ready", json!({})));
        assert!(screen.awaiting_input);
    }
    #[test]
    fn long_task_is_folded_but_inspectable_and_recovery_removes_partial_stream() {
        let task = format!(
            "Build a game\n{}\nlast requirement",
            "detailed requirement\n".repeat(40)
        );
        let mut screen = Screen::default();
        screen.receive(event("started", json!({"task":task})));
        assert_eq!(screen.task, task);
        assert_eq!(screen.entries[0].data["task"], task);
        screen.receive(event("stream", json!({"delta":"{\"message\":\"partial"})));
        screen.receive(event(
            "generation_incomplete",
            json!({"stop_reason":"max_tokens"}),
        ));
        screen.receive(event(
            "generation_recovery",
            json!({"message":"Requesting one smaller action"}),
        ));
        for width in [45, 100] {
            let view = render(&mut screen, width);
            assert!(view.contains("Full task:"));
            assert!(view.contains("smaller action"));
            assert!(!view.contains("partial"));
            assert!(!view.contains("last requirement"));
        }
    }
    #[test]
    fn conversation_hides_protocol_and_keeps_full_wrapped_reply() {
        let mut screen = Screen::default();
        screen.receive(event("delegation_started", json!({"task":"hello"})));
        screen.receive(event("bridge_connected", json!({})));
        screen.receive(event(
            "upstream",
            json!({"method":"thread/started","params":{}}),
        ));
        let reply = format!("{} the final words survive", "A useful answer. ".repeat(14));
        screen.receive(event("upstream", json!({"method":"item/completed","params":{"item":{"type":"agentMessage","text":reply}}})));
        screen.receive(event("input_ready", json!({})));
        for width in [50, 110] {
            let text = render(&mut screen, width);
            assert!(
                text.contains("Your turn")
                    && text
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .contains("final words survive"),
                "{text}"
            );
            assert!(
                !text.contains("thread")
                    && !text.contains("bridge connected")
                    && !text.contains("1 Activity")
            );
            assert!(text.contains("Enter Send"));
        }
        screen.inspect = true;
        assert!(render(&mut screen, 110).contains("1 Activity"));
    }
    #[test]
    fn stop_reason_is_visible_without_inspector() {
        let mut screen = Screen::default();
        screen.receive(event(
            "no_progress",
            json!({"message":"Repeated action without new evidence."}),
        ));
        assert!(render(&mut screen, 80).contains("Repeated action without new evidence."));
        screen.receive(event(
            "blocked",
            json!({"reason":"Required tool unavailable."}),
        ));
        assert!(render(&mut screen, 80).contains("Required tool unavailable."));
    }
    #[test]
    fn approval_is_readable_at_narrow_and_wide_sizes() {
        let mut screen = Screen::default();
        screen.receive(event(
            "started",
            json!({"task":"Fix the parser","decision":"rules"}),
        ));
        screen.receive(event("approval_required",json!({"candidate":{"id":"exact-approval-id","action":{"type":"run","argv":["python3","-m","unittest","-v"],"verification":true},"policy_version":"policy-1","evidence":["opaque-hash"]},"diff":null})));
        for width in [50, 110] {
            let text = render(&mut screen, width);
            assert!(
                text.contains("Approval required")
                    && text.contains("python3 -m unittest -v")
                    && text.contains("Approve once")
            );
            assert!(text.contains("OFFLINE SIMULATION"));
            assert!(
                !text.contains("policy_version")
                    && !text.contains("opaque-hash")
                    && !text.contains("gpt-4.1")
            );
        }
        screen.raw = true;
        assert!(render(&mut screen, 110).contains("\"candidate\""));
        assert!(
            matches!(screen.answer_approval(true), Some(UiInput::Approve(id)) if id == "exact-approval-id")
        );
        assert!(screen.answer_approval(true).is_none());
    }
    #[test]
    fn activity_follows_latest_event_and_stream_hides_json() {
        let mut screen = Screen::default();
        for i in 0..100 {
            screen.receive(event(
                "proposal",
                json!({"message":format!("Step {i}"),"actions":[]}),
            ));
        }
        assert!(render(&mut screen, 70).contains("Step 99"));
        assert_eq!(
            visible_message("{\"message\":\"Reading the pars"),
            "Reading the pars"
        );
        assert_eq!(visible_message("{\"message\":\"Fix \\"), "Fix ");
        assert_eq!(visible_message("{\"actions\":[{\"type\":\"patch\"}"), "");
        assert_eq!(
            visible_message("{\"message\":\"Use \\\"x\\\" now\",\"actions\":["),
            "Use \"x\" now"
        );
    }
}
