use crate::domain::{RunEvent, UiInput};
use anyhow::Result;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
};
use serde_json::Value;
use std::{collections::VecDeque, io};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

pub async fn terminal(
    mut events: mpsc::UnboundedReceiver<RunEvent>,
    input: mpsc::UnboundedSender<UiInput>,
    cancel: CancellationToken,
    label: String,
) -> Result<Option<RunEvent>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let _restore = Restore;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut keys = EventStream::new();
    let mut entries = VecDeque::new();
    let mut selected = 0usize;
    let mut expanded = false;
    let mut tab = 0usize;
    let mut scroll = 0u16;
    let mut candidates = String::new();
    let mut context = String::from("No evidence captured yet.");
    let mut diff = String::from("No patch proposed yet.");
    let mut stream = String::new();
    let mut pending: Option<String> = None;
    let mut status = "running".to_owned();
    let mut done = false;
    let mut summary = None;
    loop {
        let activity = if expanded {
            entries
                .get(selected)
                .map(|e: &RunEvent| serde_json::to_string_pretty(&e.data).unwrap_or_default())
                .unwrap_or_default()
        } else {
            entries
                .iter()
                .enumerate()
                .map(|(i, e): (usize, &RunEvent)| {
                    format!(
                        "{} {}  {}",
                        if i == selected { "›" } else { " " },
                        e.kind,
                        event_preview(e)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let body = match tab {
            0 => {
                if stream.is_empty() {
                    activity
                } else {
                    format!("{activity}\n\nStreaming proposal (structured data):\n{stream}")
                }
            }
            1 => candidates.clone(),
            2 => context.clone(),
            _ => diff.clone(),
        };
        terminal.draw(|f|{
            let areas=Layout::vertical([Constraint::Length(2),Constraint::Length(2),Constraint::Min(1),Constraint::Length(3)]).split(f.area());
            f.render_widget(Paragraph::new(format!("Nerve  ·  {label}\n{status}")).style(Style::default().fg(Color::Cyan)),areas[0]);
            f.render_widget(Tabs::new(["Activity [1]","Decision [2]","Context [3]","Diff [4]"]).select(tab).highlight_style(Style::default().fg(Color::Yellow)),areas[1]);
            f.render_widget(Paragraph::new(body.as_str()).wrap(Wrap{trim:false}).scroll((scroll,0)).block(Block::default().borders(Borders::TOP)),areas[2]);
            let footer=if pending.is_some(){"Approval required: y approve exact action · n deny\n1–4 inspect · ↑↓ select · Enter expand · PgUp/PgDn scroll · Esc cancel"}else if done{"Run stopped. q / Esc close · 1–4 inspect · Enter expand\nText is copyable; use nerve export for a sanitized JSONL trace."}else{"↑↓ select · Enter expand/collapse · 1–4 views · PgUp/PgDn scroll\nEsc / Ctrl-C cancel · Mouse selection remains available for copying"};
            f.render_widget(Paragraph::new(footer).style(Style::default().fg(if pending.is_some(){Color::Yellow}else{Color::DarkGray})),areas[3]);
        })?;
        tokio::select! {
            event=events.recv(),if !done=>match event{
                Some(e)=>{
                    match e.kind.as_str(){
                        "stream"=>{stream.push_str(e.data["delta"].as_str().unwrap_or(""));stream=crate::tools::bound(&stream,16_000);},
                        "proposal"=>stream.clear(),
                        "candidates"|"selection"=>{if e.kind=="candidates"{candidates=serde_json::to_string_pretty(&e.data)?;}else{candidates=format!("Selection:\n{}\n\n{candidates}",serde_json::to_string_pretty(&e.data)?);}},
                        "context_evicted"|"rehydrated"|"eviction_decision"=>context=format!("{}\n{}\n\n{context}",e.kind,serde_json::to_string_pretty(&e.data)?),
                        "tool_result"=>{context=format!("Captured {} bytes · artifact {}\nRevision {}\n\n{context}",e.data["artifact"]["bytes"],e.data["artifact"]["hash"],e.data["artifact"]["revision"]);},
                        "approval_required"=>{pending=e.data["candidate"]["id"].as_str().map(str::to_owned);status="awaiting approval".into();if !e.data["diff"].is_null(){diff=if let Some(d)=e.data["diff"].as_str(){d.into()}else{serde_json::to_string_pretty(&e.data["diff"])?};tab=3;}else{tab=0;expanded=true;}scroll=0;},
                        "approved"|"denied"=>{pending=None;status=e.kind.clone();expanded=false;},
                        "summary"=>{status=e.data["status"].as_str().unwrap_or("stopped").into();summary=Some(e.clone());pending=None;},
                        "upstream"=>{if e.data["method"]=="item/agentMessage/delta"{stream.push_str(e.data["params"]["delta"].as_str().unwrap_or(""));}},
                        _=>{}
                    }
                    context=crate::tools::bound(&context,32_000);
                    if e.kind!="stream"{entries.push_back(e);if entries.len()>300{entries.pop_front();}selected=entries.len().saturating_sub(1);}
                },None=>{done=true;status=format!("{status} · q to close");}
            },
            key=keys.next()=>if let Some(Ok(Event::Key(key)))=key {if key.kind!=KeyEventKind::Press{continue}match key.code{
                KeyCode::Esc|KeyCode::Char('q')=>{if done{break}cancel.cancel();status="cancelling; waiting for cleanup".into();},
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL)=>{if done{break}cancel.cancel();status="cancelling; waiting for cleanup".into();},
                KeyCode::Char('y')=>if let Some(id)=pending.take(){let _=input.send(UiInput::Approve(id));status="approval sent; revalidating workspace".into();},
                KeyCode::Char('n')=>if let Some(id)=pending.take(){let _=input.send(UiInput::Deny(id));},
                KeyCode::Char(c @ '1'..='4')=>{tab=(c as u8-b'1')as usize;scroll=0;},
                KeyCode::Up=>selected=selected.saturating_sub(1),
                KeyCode::Down=>selected=(selected+1).min(entries.len().saturating_sub(1)),
                KeyCode::Enter=>{expanded = !expanded;scroll=0;},
                KeyCode::PageDown=>scroll=scroll.saturating_add(12),
                KeyCode::PageUp=>scroll=scroll.saturating_sub(12),
                KeyCode::Home=>scroll=0,
                _=>{}
            }},
        }
    }
    terminal.show_cursor()?;
    Ok(summary)
}
fn event_preview(e: &RunEvent) -> String {
    let v = &e.data;
    let text = if let Some(s) = v["message"].as_str() {
        s.to_owned()
    } else if e.kind == "tool_result" {
        format!(
            "artifact {} · exit {}",
            v["artifact"]["hash"].as_str().unwrap_or(""),
            v["exit_code"]
        )
    } else if e.kind == "approval_required" {
        format!("{}", v["candidate"]["action"])
    } else {
        match v {
            Value::Object(_) => serde_json::to_string(v).unwrap_or_default(),
            _ => v.to_string(),
        }
    };
    text.chars().take(180).collect()
}
