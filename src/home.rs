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
pub const HELP: &str = "/provider codex|openai|claude  /model MODEL  /decision rules|jev|generative\n/jev openrouter|typesafe  /workspace PATH  /login  /account  /sessions\n/resume ID  /continue ID (Codex)  /demo  /claude-code  /help  /exit\nNative keys: OPENAI_API_KEY or ANTHROPIC_API_KEY; Jev: OPENROUTER_API_KEY or TYPESAFE_API_KEY. Set keys in the environment, never in this prompt.";

#[derive(Clone)]
pub struct Settings {
    pub provider: String,
    pub model: Option<String>,
    pub decision: String,
    pub jev_provider: String,
    pub workspace: PathBuf,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: "codex".into(),
            model: None,
            decision: "rules".into(),
            jev_provider: "openrouter".into(),
            workspace: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        }
    }
}
impl Settings {
    pub fn ownership(&self) -> String {
        if self.provider == "codex" {
            "ChatGPT account · Codex owns execution · Jev not active".into()
        } else {
            format!(
                "Native · S1Code executes tools · decisions: {}{}",
                self.decision,
                if self.decision == "jev" {
                    format!(" via {}", self.jev_provider)
                } else {
                    String::new()
                }
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
        if self.decision == "jev" {
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
    std::env::var(name).is_ok_and(|s| !s.trim().is_empty())
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
            settings.model = None;
        }
        "/model" => {
            ensure!(
                !args.is_empty() && args.len() <= 200 && !args.contains(char::is_whitespace),
                "Use /model MODEL_ID"
            );
            settings.model = Some(args.into());
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
                .insert(self.cursor, if c == '\n' || c == '\t' { ' ' } else { c });
            self.cursor += 1;
        }
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
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(status_height.min(f.area().height.saturating_sub(14).max(2))),
        Constraint::Min(1),
        Constraint::Length(3),
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
    let width = rows[3].width.saturating_sub(4) as usize;
    let start = editor.cursor.saturating_sub(width.saturating_sub(1));
    let visible: String = editor.text.iter().skip(start).take(width).collect();
    f.render_widget(
        Paragraph::new(if editor.text.is_empty() {
            "Describe your task, or /help"
        } else {
            &visible
        })
        .block(
            Block::bordered()
                .title(" You ")
                .border_style(Style::default().fg(ACCENT)),
        ),
        rows[3],
    );
    let prefix: String = editor
        .text
        .iter()
        .skip(start)
        .take(editor.cursor - start)
        .collect();
    let cursor_x = Line::raw(prefix).width().min(width) as u16;
    if rows[3].height >= 3 && rows[3].width >= 4 {
        f.set_cursor_position((rows[3].x + 1 + cursor_x, rows[3].y + 1));
    }
    f.render_widget(Paragraph::new("Enter Send · F2 Provider · Ctrl-U Clear · Ctrl-C Exit\nF3 Decisions (native) · /help Commands").style(Style::default().fg(MUTED)), rows[4]);
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
    fn editor_handles_unicode_paste_without_submitting_or_injecting_controls() {
        let mut editor = Editor::default();
        editor.insert("şu hatayı düzelt");
        editor.key(KeyCode::Home);
        editor.key(KeyCode::Right);
        editor.key(KeyCode::Delete);
        editor.key(KeyCode::End);
        editor.insert("\n/help\u{1b}");
        assert_eq!(editor.take(), "ş hatayı düzelt /help");
        assert_eq!(editor.cursor, 0);
    }
}
