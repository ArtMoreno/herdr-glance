mod herdr;
mod model;
#[cfg(test)]
mod tests;
mod theme;
mod ui;

use crate::{
    herdr::{command, now, Herdr},
    model::{Card, Result},
    theme::{Config, THEMES},
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    env, io,
    path::PathBuf,
    process::Stdio,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const HELP:&str="herdr-glance 0.1.0\n\nUsage: glance [line] [--theme NAME] [--config PATH] [--plain] [--icons nerd|emoji|ascii] [--no-emoji] [--all-workspaces]\n       glance --refresh\n\nDashboard: j/k select, Enter focus selected agent, t theme, e emoji, q quit.\nRequires HERDR_ENV=1 and HERDR_WORKSPACE_ID. All polling is read-only.\n--all-workspaces includes every agent in the session (also supported by line/--refresh).\nline reads a brief counts cache and starts a detached refresh when stale.\n--refresh refreshes counts synchronously (for diagnostics or scheduled use).";
fn main() {
    if let Err(e) = run() {
        eprintln!("glance: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    if args.iter().any(|a| a == "--version") {
        println!("glance {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let mut h = Herdr::from_env()?;
    let default_config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("herdr-glance/config.toml");
    let mut config_path = None;
    for (i, a) in args.iter().enumerate() {
        if a == "--config" {
            config_path = Some(PathBuf::from(
                args.get(i + 1).ok_or("--config needs a path")?,
            ));
        }
    }
    let mut c = if let Some(path) = config_path {
        Config::load(&path)?
    } else if default_config.exists() {
        Config::load(&default_config)?
    } else {
        Config::default()
    };
    let mut line = false;
    let mut plain = false;
    let mut refresh = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "line" => line = true,
            "--plain" => plain = true,
            "--refresh" => refresh = true,
            "--all-workspaces" => h.workspace = "*".into(),
            "--no-emoji" => c.emoji = false,
            "--config" => {
                i += 1;
            }
            "--theme" | "--icons" => {
                let key = &args[i];
                i += 1;
                let value = args.get(i).ok_or("option needs a value")?.clone();
                if key == "--theme" {
                    c.theme = value
                } else {
                    c.icons = value
                }
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
        i += 1;
    }
    c.validate()?;
    if refresh {
        return h.refresh();
    }
    if line {
        let cached = h.cache();
        let age = cached
            .as_ref()
            .map(|c| now().saturating_sub(c.at))
            .unwrap_or(u64::MAX);
        if age >= 2 && herdr::detach_prompt_handles().is_ok() {
            // No Herdr subprocess is awaited by a prompt. Refreshers serialize via a short-lived lock.
            if let Ok(exe) = env::current_exe() {
                let _ = command(exe)
                    .arg("--refresh")
                    .args(if h.workspace == "*" {
                        vec!["--all-workspaces"]
                    } else {
                        vec![]
                    })
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
        }
        if let Some(cache) = cached.filter(|_| age <= 10) {
            println!("{}", ui::segment(cache.counts, &c, plain));
        } else {
            println!(
                "{}",
                if c.emoji && c.icons != "ascii" {
                    "❓"
                } else {
                    "unknown"
                }
            );
        }
        return Ok(());
    }
    dashboard(h, c)
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}
struct Sample {
    agent: model::Agent,
    text: std::result::Result<String, String>,
    detection: std::result::Result<String, String>,
}
enum Update {
    Poll(std::result::Result<Vec<Sample>, String>),
    Focus(std::result::Result<(), String>),
}
fn dashboard(h: Herdr, mut c: Config) -> Result<()> {
    let (tx, rx) = mpsc::sync_channel(1);
    let poll = h.clone();
    let polling = tx.clone();
    thread::spawn(move || loop {
        let sample = poll
            .list()
            .map(|rows| {
                let _ = poll.save(&rows);
                rows.into_iter()
                    .map(|agent| {
                        let text = poll
                            .read(&agent, "recent-unwrapped")
                            .map_err(|e| e.to_string());
                        let detection = if agent.state() == 2 {
                            poll.read(&agent, "detection").map_err(|e| e.to_string())
                        } else {
                            Ok(String::new())
                        };
                        Sample {
                            agent,
                            text,
                            detection,
                        }
                    })
                    .collect()
            })
            .map_err(|e| e.to_string());
        if polling.send(Update::Poll(sample)).is_err() {
            break;
        }
        thread::sleep(Duration::from_secs(1));
    });
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.hide_cursor()?;
    let mut cards: Vec<Card> = vec![];
    let mut selected = 0;
    let mut tick = 0;
    let mut message = "Connecting to Herdr…".to_string();
    let mut last = Instant::now();
    let mut focusing = false;
    loop {
        while let Ok(update) = rx.try_recv() {
            match update {
                Update::Poll(Ok(samples)) => {
                    let selected_id = cards.get(selected).map(|c| c.agent.id());
                    let mut old: std::collections::HashMap<_, _> =
                        cards.drain(..).map(|c| (c.agent.id(), c)).collect();
                    cards = samples
                        .into_iter()
                        .map(|s| {
                            let mut card = old
                                .remove(&s.agent.id())
                                .unwrap_or_else(|| Card::new(s.agent.clone()));
                            match (s.text, s.detection) {
                                (Ok(text), Ok(detection)) => card.update(s.agent, text, detection),
                                (text, detection) => {
                                    let error = text
                                        .as_ref()
                                        .err()
                                        .or_else(|| detection.as_ref().err())
                                        .cloned();
                                    let fallback = card.lines.join("\n");
                                    card.update(
                                        s.agent,
                                        text.unwrap_or(fallback),
                                        detection.unwrap_or_default(),
                                    );
                                    card.error = error;
                                }
                            }
                            card
                        })
                        .collect();
                    selected = selected_id
                        .and_then(|id| cards.iter().position(|c| c.agent.id() == id))
                        .unwrap_or(selected.min(cards.len().saturating_sub(1)));
                    last = Instant::now();
                    message.clear();
                }
                Update::Poll(Err(e)) => {
                    message = format!("Polling failed: {e}");
                    for card in &mut cards {
                        card.error = Some("stale status".into());
                    }
                }
                Update::Focus(r) => {
                    focusing = false;
                    message = match r {
                        Ok(()) => "Focused selected agent".into(),
                        Err(e) => e,
                    };
                }
            }
        }
        let status = if last.elapsed() > Duration::from_secs(5) && message.is_empty() {
            format!(
                "Waiting for Herdr · last sample {}s ago",
                last.elapsed().as_secs()
            )
        } else {
            message.clone()
        };
        terminal.draw(|f| {
            ui::render(
                f.area(),
                f.buffer_mut(),
                &cards,
                selected,
                &c,
                tick,
                &status,
            )
        })?;
        tick += 1;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('c')
                            if k.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            break
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            selected = (selected + 1).min(cards.len().saturating_sub(1))
                        }
                        KeyCode::Char('k') | KeyCode::Up => selected = selected.saturating_sub(1),
                        KeyCode::Char('t') => {
                            let i = THEMES.iter().position(|t| *t == c.theme).unwrap_or(0);
                            c.theme = THEMES[(i + 1) % THEMES.len()].into();
                        }
                        KeyCode::Char('e') => c.emoji = !c.emoji,
                        KeyCode::Enter if !focusing => {
                            if let Some(card) = cards.get(selected) {
                                focusing = true;
                                message = "Focusing selected agent…".into();
                                let a = card.agent.clone();
                                let h = h.clone();
                                let tx = tx.clone();
                                thread::spawn(move || {
                                    let _ = tx.send(Update::Focus(
                                        h.focus(&a).map_err(|e| e.to_string()),
                                    ));
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}
