use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, time::Instant};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub const STATES: [&str; 5] = ["working", "idle", "blocked", "done", "unknown"];
pub const EMOJI: [&str; 5] = ["🧠", "💤", "🙋", "✅", "❓"];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Agent {
    pub pane_id: String,
    pub terminal_id: String,
    pub workspace_id: String,
    pub name: Option<String>,
    pub agent: Option<String>,
    pub agent_status: String,
    #[serde(default)]
    pub state_change_seq: u64,
}
impl Agent {
    pub fn id(&self) -> String {
        format!("{}:{}", self.pane_id, self.terminal_id)
    }
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.pane_id)
    }
    pub fn state(&self) -> usize {
        STATES
            .iter()
            .position(|s| *s == self.agent_status)
            .unwrap_or(4)
    }
}
pub fn result(value: &Value) -> Result<&Value> {
    if let Some(error) = value.get("error") {
        return Err(format!("Herdr: {error}").into());
    }
    Ok(value.get("result").unwrap_or(value))
}
pub fn agents(value: &Value, workspace: &str) -> Result<Vec<Agent>> {
    let mut rows: Vec<Agent> = serde_json::from_value(
        result(value)?
            .get("agents")
            .ok_or("missing agents array")?
            .clone(),
    )?;
    let mut seen = HashSet::new();
    for a in &rows {
        if a.pane_id.is_empty()
            || a.pane_id.starts_with('-')
            || a.terminal_id.is_empty()
            || !seen.insert(a.pane_id.clone())
        {
            return Err("empty or duplicate Herdr identity".into());
        }
    }
    rows.retain(|a| (workspace == "*" || a.workspace_id == workspace) && a.agent.is_some());
    rows.sort_by_key(|a| {
        (
            if a.state() == 2 { 0 } else { 1 },
            a.name().to_lowercase(),
            a.id(),
        )
    });
    Ok(rows)
}
// Remove terminal control sequences before displaying untrusted agent output.
pub fn clean(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    for x in chars.by_ref() {
                        if ('@'..='~').contains(&x) {
                            break;
                        }
                    }
                }
                Some(']') | Some('P') | Some('_') | Some('^') => {
                    while let Some(x) = chars.next() {
                        if x == '\n' {
                            out.push('\n');
                            break;
                        }
                        if x == '\u{7}' || (x == '\u{1b}' && chars.peek() == Some(&'\\')) {
                            if x == '\u{1b}' {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                Some('(' | ')' | '*' | '+') => {
                    chars.next();
                }
                _ => {}
            }
        } else if c == '\n'
            || (!c.is_control()
                && !matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        {
            out.push(c);
        }
    }
    out
}
#[cfg(test)]
pub fn read_text_fixture(value: &Value, agent: &Agent) -> Result<String> {
    let r = result(value)?.get("read").ok_or("missing read object")?;
    if r["pane_id"] != agent.pane_id || r["workspace_id"] != agent.workspace_id {
        return Err("read identity mismatch".into());
    }
    Ok(clean(r["text"].as_str().ok_or("missing read text")?))
}
pub struct Card {
    pub agent: Agent,
    pub since: Instant,
    pub lines: Vec<String>,
    pub activity: Vec<u64>,
    pub detection: String,
    pub error: Option<String>,
}
impl Card {
    pub fn new(agent: Agent) -> Self {
        Self {
            agent,
            since: Instant::now(),
            lines: vec![],
            activity: vec![0; 32],
            detection: String::new(),
            error: None,
        }
    }
    pub fn update(&mut self, agent: Agent, text: String, detection: String) {
        if self.agent.agent_status != agent.agent_status
            || self.agent.state_change_seq != agent.state_change_seq
        {
            self.since = Instant::now();
        }
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let delta = if self.lines.is_empty() {
            0
        } else {
            line_delta(&self.lines, &lines)
        };
        self.activity.remove(0);
        self.activity.push(delta as u64);
        self.agent = agent;
        self.lines = lines;
        self.detection = detection;
        self.error = None;
    }
}
pub fn line_delta(old: &[String], new: &[String]) -> usize {
    // ponytail: a 40-line window cannot distinguish identical repeated output; use revision events if exact counts become necessary.
    let overlap = (0..=old.len().min(new.len()))
        .rev()
        .find(|&n| old[old.len() - n..] == new[..n])
        .unwrap_or(0);
    new.len() - overlap
}
