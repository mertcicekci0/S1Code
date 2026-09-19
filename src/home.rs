//! Interactive task entry. Opening the screen never starts inference or changes accounts.
use crate::{brand, generation, privacy::Redactor};
use anyhow::{Context, Result, bail, ensure};
use crossterm::{
    event::{EnableBracketedPaste, Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};
use std::{io, path::PathBuf};

const ACCENT: Color = Color::Rgb(111, 211, 194);
const MUTED: Color = Color::Rgb(151, 163, 177);
pub const HELP: &str = "/provider codex|openai|claude  /model opus|sonnet|MODEL_ID  /effort LEVEL  /output-limit TOKENS  /decision rules|jev|generative\n/jev openrouter|typesafe  /eviction jev|conservative|off  /workspace PATH  /permissions manual|full-access  /login  /account  /sessions\n/resume ID  /continue ID (Codex)  /demo  /claude-code  /help  /exit\nNative keys: OPENAI_API_KEY or ANTHROPIC_API_KEY; Jev: OPENROUTER_API_KEY or TYPESAFE_API_KEY. Set keys in the environment, never in this prompt.";

#[derive(Clone)]
pub struct Settings {
    pub auto_approve: bool,
    pub max_output_tokens: u32,
    pub effort: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub decision: String,
    pub jev_provider: String,
    pub eviction: String,
    pub workspace: PathBuf,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_approve: false,
            max_output_tokens: crate::domain::default_output_limit(),
            effort: None,
            provider: "codex".into(),
            model: None,
            decision: "rules".into(),
            jev_provider: "typesafe".into(),
            eviction: "conservative".into(),
            workspace: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        }
    }
}
impl Settings {
    pub fn load(home: &std::path::Path) -> Result<Self> {
        let path = home.join("preferences-v1.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(&path).context("Cannot read preferences-v1.json")?;
        ensure!(bytes.len() <= 8192, "preferences file too large");
        let saved: serde_json::Value = serde_json::from_slice(&bytes)
            .context("Invalid preferences-v1.json; move it aside to reset preferences")?;
        ensure!(saved["version"] == 1, "unsupported preferences version");
        let mut settings = Self::default();
        for (key, command) in [
            ("provider", "/provider"),
            ("model", "/model"),
            ("effort", "/effort"),
            ("decision", "/decision"),
            ("jev_provider", "/jev"),
            ("eviction", "/eviction"),
        ] {
            if key != "provider" && settings.provider == "codex" && key != "model" {
                continue;
            }
            if let Some(value) = saved[key].as_str() {
                // Jev endpoint preference must not silently turn a rules policy into Jev.
                let old_decision = settings.decision.clone();
                interpret(&format!("{command} {value}"), &mut settings)?;
                if key == "jev_provider" {
                    settings.decision = old_decision;
                }
            }
        }
        if let Some(value) = saved["max_output_tokens"].as_u64() {
            interpret(&format!("/output-limit {value}"), &mut settings)?;
        }
        Ok(settings)
    }
    pub fn save(&self, home: &std::path::Path) -> Result<()> {
        crate::session::private_dir(home)?;
        // Credentials, current directory and automatic consent are deliberately absent.
        crate::session::atomic_write(
            &home.join("preferences-v1.json"),
            &serde_json::to_vec_pretty(
                &serde_json::json!({"version":1,"provider":self.provider,"model":self.model,"decision":self.decision,"jev_provider":self.jev_provider,"eviction":self.eviction,"effort":self.effort,"max_output_tokens":self.max_output_tokens}),
            )?,
        )
    }
    pub fn ownership(&self) -> String {
        if self.provider == "codex" {
            "ChatGPT account · Codex owns execution · Jev not active".into()
        } else {
            format!(
                "Native · S1Code executes tools · decisions: {}{}{} · context: {}",
                self.decision,
                if self.decision == "jev" {
                    format!(" via {}", self.jev_provider)
                } else {
                    String::new()
                },
                if self.auto_approve {
                    " · AUTO APPROVE (supported actions)"
                } else {
                    ""
                },
                self.eviction
            )
        }
    }
    pub fn model_name(&self) -> &str {
        self.model.as_deref().unwrap_or_else(|| {
            if self.provider == "codex" {
                "Codex default"
            } else {
                generation::default_model(&self.provider)
            }
        })
    }
    fn next_provider(&mut self) {
        self.provider = match self.provider.as_str() {
            "codex" => "openai",
            "openai" => "claude",
            _ => "codex",
        }
        .into();
        self.model = None;
        self.effort = None;
        self.auto_approve = false;
    }
    fn readiness(&self) -> String {
        if self.provider == "codex" {
            return "Managed login: /login · inspect account: /account".into();
        }
        let key = if self.provider == "claude" {
            "ANTHROPIC_API_KEY"
        } else {
            "OPENAI_API_KEY"
        };
        let mut result = format!(
            "{key}: {}",
            if key_present(key) {
                "configured"
            } else {
                "missing"
            }
        );
        if self.decision == "jev" || self.eviction == "jev" {
            let key = if self.jev_provider == "openrouter" {
                "OPENROUTER_API_KEY"
            } else {
                "TYPESAFE_API_KEY"
            };
            result.push_str(&format!(
                " · {key}: {}",
                if key_present(key) {
                    "configured"
                } else {
                    "missing"
                }
            ));
        }
        result
    }
}
fn key_present(name: &str) -> bool {
    crate::credentials::present(name)
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Task(String),
    Login,
    Account,
    Sessions,
    Resume(String, bool),
    Demo,
    ClaudeCode,
    Exit,
}

pub fn interpret(line: &str, settings: &mut Settings) -> Result<Option<Command>> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    if !line.starts_with('/') {
        return Ok(Some(Command::Task(line.into())));
    }
    let (command, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let args = args.trim();
    match command {
        "/provider" => {
            ensure!(
                ["codex", "openai", "claude"].contains(&args),
                "Choose /provider codex, openai or claude"
            );
            settings.provider = args.into();
            settings.auto_approve = false;
            settings.model = None;
            settings.effort = None;
        }
        "/model" => {
            let model = match (settings.provider.as_str(), args) {
                ("claude", "opus") => "claude-opus-5",
                ("claude", "sonnet") => "claude-sonnet-5",
                _ => args,
            };
            ensure!(
                !model.is_empty()
                    && model.len() <= 200
                    && !model.contains(char::is_whitespace)
                    && !model.starts_with("sk-")
                    && !model.starts_with("apikey_")
                    && !model.ends_with('-'),
                "Use /model opus, /model sonnet, or a complete MODEL_ID"
            );
            settings.model = Some(model.into());
        }
        "/decision" | "/jev" => {
            ensure!(
                settings.provider != "codex",
                "Codex owns its loop. Select /provider openai or /provider claude to use native Jev selection."
            );
            if command == "/decision" {
                ensure!(
                    ["rules", "jev", "generative"].contains(&args),
                    "Choose rules, jev or generative"
                );
                settings.decision = args.into();
            } else {
                ensure!(
                    ["openrouter", "typesafe"].contains(&args),
                    "Choose /jev openrouter or typesafe"
                );
                settings.jev_provider = args.into();
                settings.decision = "jev".into();
            }
        }
        "/effort" => {
            ensure!(
                settings.provider == "claude",
                "Effort is currently available for native Claude"
            );
            ensure!(
                ["low", "medium", "high", "xhigh", "max", "default"].contains(&args),
                "Use /effort low|medium|high|xhigh|max|default"
            );
            settings.effort = if args == "default" {
                None
            } else {
                Some(args.into())
            };
        }
        "/eviction" => {
            ensure!(
                settings.provider != "codex",
                "Codex owns its context; choose a native provider first"
            );
            ensure!(
                ["jev", "conservative", "off"].contains(&args),
                "Use /eviction jev|conservative|off"
            );
            settings.eviction = args.into();
        }
        "/output-limit" => {
            let limit: u32 = args.parse().context("Use /output-limit 16384")?;
            ensure!(
                (1024..=65536).contains(&limit),
                "Output limit must be 1024..65536"
            );
            settings.max_output_tokens = limit;
        }
        "/permissions" => {
            ensure!(
                settings.provider != "codex",
                "Codex manages its own permissions; choose a native provider first"
            );
            ensure!(
                ["manual", "full-access"].contains(&args),
                "Use /permissions manual or full-access"
            );
            settings.auto_approve = args == "full-access";
        }
        "/workspace" => {
            ensure!(!args.is_empty(), "Use /workspace PATH");
            let path = PathBuf::from(args)
                .canonicalize()
                .context("workspace does not exist")?;
            ensure!(path.is_dir(), "workspace must be a directory");
            settings.workspace = path;
        }
        "/login" if args.is_empty() => return Ok(Some(Command::Login)),
        "/account" if args.is_empty() => return Ok(Some(Command::Account)),
        "/sessions" if args.is_empty() => return Ok(Some(Command::Sessions)),
        "/resume" | "/continue" => {
            uuid::Uuid::parse_str(args).context("Use a session ID from /sessions")?;
            return Ok(Some(Command::Resume(args.into(), command == "/continue")));
        }
        "/demo" if args.is_empty() => return Ok(Some(Command::Demo)),
        "/claude-code" if args.is_empty() => return Ok(Some(Command::ClaudeCode)),
        "/exit" | "/quit" if args.is_empty() => return Ok(Some(Command::Exit)),
        "/help" => bail!("{HELP}"),
        _ => bail!(
            "Unknown command. Type /help. API keys belong in environment variables, not here."
        ),
    }
    Ok(None)
}

#[derive(Default)]
pub(crate) struct Editor {
    pub(crate) text: Vec<char>,
    cursor: usize,
}
impl Editor {
    pub(crate) fn insert(&mut self, text: &str) {
        for c in text
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        {
            if self.text.len() >= 8192 {
                break;
            }
            self.text
                .insert(self.cursor, if c == '\t' { ' ' } else { c });
            self.cursor += 1;
        }
    }
    pub(crate) fn layout(&self, width: usize) -> (Vec<String>, usize, usize) {
        let width = width.max(1);
        let mut lines = vec![String::new()];
        let (mut row, mut col) = (0, 0);
        let mut cursor = (0, 0);
        for (index, c) in self.text.iter().enumerate() {
            let cell_width = Line::raw(c.to_string()).width();
            if *c != '\n' && col + cell_width > width {
                lines.push(String::new());
                row += 1;
                col = 0;
            }
            if index == self.cursor {
                cursor = (row, col);
            }
            if *c == '\n' {
                lines.push(String::new());
                row += 1;
                col = 0;
            } else {
                lines[row].push(*c);
                col += cell_width;
            }
        }
        if self.cursor == self.text.len() {
            if col >= width {
                lines.push(String::new());
                row += 1;
                col = 0;
            }
            cursor = (row, col);
        }
        (lines, cursor.0, cursor.1)
    }
    pub(crate) fn key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => self.insert(&c.to_string()),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.text.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                self.text.remove(self.cursor);
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.text.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            _ => {}
        }
    }
    pub(crate) fn take(&mut self) -> String {
        self.cursor = 0;
        self.text.drain(..).collect()
    }
}

fn draw(f: &mut Frame, settings: &Settings, editor: &Editor, messages: &[String]) {
    let safe = Redactor::environment("");
    let status = format!(
        "{} · {}\n{}\n{}\nWorkspace: {}",
        settings.provider,
        settings.model_name(),
        settings.ownership(),
        settings.readiness(),
        settings.workspace.display()
    );
    let status_width = f.area().width.saturating_sub(2).max(1) as usize;
    let status_height = status
        .lines()
        .map(|line| Line::raw(line).width().div_ceil(status_width).max(1))
        .sum::<usize>() as u16;
    let input_width = f.area().width.saturating_sub(6).max(1) as usize;
    let (input_lines, cursor_row, cursor_col) = editor.layout(input_width);
    let input_height = (input_lines.len() + 2).clamp(3, 8) as u16;
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(status_height.min(f.area().height.saturating_sub(14).max(2))),
        Constraint::Min(1),
        Constraint::Length(input_height),
        Constraint::Length(2),
    ])
    .margin(1)
    .split(f.area());
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    brand::NAME,
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  v{}", env!("CARGO_PKG_VERSION")),
                    Style::default().fg(MUTED),
                ),
            ]),
            Line::raw("What would you like to build or fix?"),
        ]),
        rows[0],
    );
    f.render_widget(
        Paragraph::new(safe.text(&status))
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(MUTED)),
        rows[1],
    );
    let text = if messages.is_empty() {
        "Describe a coding task below and press Enter.\n\nFor example: Fix the failing parser test and verify the change.\n\n/provider claude    Native Claude API; compatible with Jev\n/login             Sign in with ChatGPT through official Codex\n/demo              Offline demonstration with real file tools\n/help              All commands and authentication options\n\nEach prompt starts a saved task. Use /resume ID for existing work.\nExecution and patch approvals appear when needed.".into()
    } else {
        messages
            .iter()
            .rev()
            .take(8)
            .rev()
            .map(|s| safe.text(s))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let lines = text.lines().count();
    let scroll = lines
        .saturating_sub(rows[2].height as usize)
        .min(u16::MAX as usize) as u16;
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        rows[2],
    );
    let scroll = cursor_row.saturating_sub(rows[3].height.saturating_sub(3) as usize);
    let input = if editor.text.is_empty() {
        "Describe your task, or /help".into()
    } else {
        input_lines.join("\n")
    };
    f.render_widget(
        Paragraph::new(input)
            .scroll((scroll.min(u16::MAX as usize) as u16, 0))
            .block(
                Block::bordered()
                    .title(" You ")
                    .border_style(Style::default().fg(ACCENT)),
            ),
        rows[3],
    );
    if rows[3].height >= 3 && rows[3].width >= 4 {
        f.set_cursor_position((
            rows[3].x + 1 + cursor_col.min(input_width - 1) as u16,
            rows[3].y + 1 + cursor_row.saturating_sub(scroll) as u16,
        ));
    }
    f.render_widget(Paragraph::new("Enter Send · Shift-Enter New line · Ctrl-U Clear · Ctrl-C Exit\nF2 Provider · F3 Decisions (native) · /help Commands").style(Style::default().fg(MUTED)), rows[4]);
}

pub async fn prompt(settings: &mut Settings, messages: &mut Vec<String>) -> Result<Command> {
    enable_raw_mode()?;
    let _restore = crate::ui::Restore;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut events = EventStream::new();
    let mut editor = Editor::default();
    loop {
        terminal.draw(|f| draw(f, settings, &editor, messages))?;
        match events.next().await.context("terminal input closed")?? {
            Event::Paste(text) => editor.insert(&text),
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc => return Ok(Command::Exit),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(Command::Exit);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    editor.take();
                }
                KeyCode::F(2) => settings.next_provider(),
                KeyCode::F(3) => {
                    if settings.provider == "codex" {
                        messages.push(
                            "S1Code: Codex owns its decisions. Use a native provider for Jev."
                                .into(),
                        );
                    } else {
                        settings.decision = match settings.decision.as_str() {
                            "rules" => "jev",
                            "jev" => "generative",
                            _ => "rules",
                        }
                        .into();
                    }
                }
                KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    editor.insert("\n")
                }
                KeyCode::Enter => {
                    let line = editor.take();
                    match interpret(&line, settings) {
                        Ok(Some(command)) => return Ok(command),
                        Ok(None) => {}
                        Err(error) => messages.push(format!("S1Code: {error:#}")),
                    }
                }
                other
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    editor.key(other)
                }
                _ => {}
            },
            _ => {}
        }
        if messages.len() > 40 {
            messages.drain(..messages.len() - 40);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn home_distinguishes_native_decisions_and_delegated_auth() {
        let mut s = Settings::default();
        assert!(interpret("/jev openrouter", &mut s).is_err());
        interpret("/provider claude", &mut s).unwrap();
        interpret("/jev openrouter", &mut s).unwrap();
        assert!(s.ownership().contains("jev via openrouter"));
        assert_eq!(
            interpret("fix parser", &mut s).unwrap(),
            Some(Command::Task("fix parser".into()))
        );
        assert_eq!(
            interpret("/claude-code", &mut s).unwrap(),
            Some(Command::ClaudeCode)
        );
        interpret("/provider codex", &mut s).unwrap();
        assert!(s.ownership().contains("Jev not active"));
        for width in [40, 100] {
            let mut terminal =
                Terminal::new(ratatui::backend::TestBackend::new(width, 30)).unwrap();
            terminal
                .draw(|f| draw(f, &s, &Editor::default(), &[]))
                .unwrap();
            let buffer: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(
                buffer.contains("S1Code") && buffer.contains("You") && buffer.contains("/help")
            );
            assert!(!buffer.contains("OFFLINE SIMULATION"));
        }
    }
    #[test]
    fn preferences_remember_provider_without_persisting_workspace_or_consent() {
        let home = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        interpret("/provider claude", &mut settings).unwrap();
        interpret("/model claude-opus-5", &mut settings).unwrap();
        interpret("/jev typesafe", &mut settings).unwrap();
        interpret("/effort medium", &mut settings).unwrap();
        interpret("/permissions full-access", &mut settings).unwrap();
        settings.save(home.path()).unwrap();
        let saved = std::fs::read_to_string(home.path().join("preferences-v1.json")).unwrap();
        assert!(!saved.contains("workspace") && !saved.contains("auto_approve"));
        let restored = Settings::load(home.path()).unwrap();
        assert_eq!(restored.provider, "claude");
        assert_eq!(restored.decision, "jev");
        assert_eq!(restored.effort.as_deref(), Some("medium"));
        assert!(!restored.auto_approve);
        settings.next_provider();
        assert!(!settings.auto_approve && settings.effort.is_none());
    }
    #[test]
    fn claude_model_aliases_expand_and_incomplete_ids_fail_before_network_use() {
        let mut settings = Settings::default();
        interpret("/provider claude", &mut settings).unwrap();
        interpret("/model opus", &mut settings).unwrap();
        assert_eq!(settings.model.as_deref(), Some("claude-opus-5"));
        interpret("/model sonnet", &mut settings).unwrap();
        assert_eq!(settings.model.as_deref(), Some("claude-sonnet-5"));
        assert!(interpret("/model claude-opus-", &mut settings).is_err());
        assert_eq!(settings.model.as_deref(), Some("claude-sonnet-5"));
    }
    #[test]
    fn home_auto_approval_is_explicit_native_and_reset_on_provider_change() {
        let mut settings = Settings::default();
        assert!(interpret("/permissions full-access", &mut settings).is_err());
        interpret("/provider claude", &mut settings).unwrap();
        assert!(!settings.auto_approve);
        interpret("/permissions full-access", &mut settings).unwrap();
        assert!(settings.auto_approve);
        assert!(settings.ownership().contains("AUTO APPROVE"));
        assert!(matches!(
            interpret("Build a snake game", &mut settings).unwrap(),
            Some(Command::Task(_))
        ));
        interpret("/permissions manual", &mut settings).unwrap();
        assert!(!settings.auto_approve);
        interpret("/permissions full-access", &mut settings).unwrap();
        interpret("/provider codex", &mut settings).unwrap();
        assert!(!settings.auto_approve);
    }
    #[test]
    fn multiline_input_wraps_without_changing_pasted_task() {
        let mut editor = Editor::default();
        editor.insert("123456\nşeker\nfinal");
        let original: String = editor.text.iter().collect();
        let (lines, row, col) = editor.layout(5);
        assert_eq!(lines, vec!["12345", "6", "şeker", "final", ""]);
        assert_eq!((row, col), (4, 0));
        assert_eq!(editor.take(), original);
    }
    #[test]
    fn editor_handles_unicode_paste_without_submitting_or_injecting_controls() {
        let mut editor = Editor::default();
        editor.insert("şu hatayı düzelt");
        editor.key(KeyCode::Home);
        editor.key(KeyCode::Right);
        editor.key(KeyCode::Delete);
        editor.key(KeyCode::End);
        editor.insert("\n/help\u{1b}");
        assert_eq!(editor.take(), "ş hatayı düzelt\n/help");
        assert_eq!(editor.cursor, 0);
    }
}
