use crate::model::Result;
use ratatui::style::Color;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

pub const THEMES: [&str; 5] = [
    "catppuccin-mocha",
    "dracula",
    "tokyo-night",
    "nord",
    "gruvbox",
];
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub theme: String,
    pub emoji: bool,
    pub icons: String,
    pub spinner: String,
    pub colors: BTreeMap<String, String>,
    pub kind_icons: BTreeMap<String, String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            theme: THEMES[0].into(),
            emoji: true,
            icons: "emoji".into(),
            spinner: "dots".into(),
            colors: BTreeMap::new(),
            kind_icons: BTreeMap::new(),
        }
    }
}
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }
    pub fn validate(&self) -> Result<()> {
        if !THEMES.contains(&self.theme.as_str()) {
            return Err(format!("unknown theme: {}", self.theme).into());
        }
        if !["nerd", "emoji", "ascii"].contains(&self.icons.as_str()) {
            return Err("icons must be nerd, emoji, or ascii".into());
        }
        if !["dots", "line", "pulse"].contains(&self.spinner.as_str()) {
            return Err("spinner must be dots, line, or pulse".into());
        }
        for (k, v) in &self.colors {
            if ![
                "background",
                "surface",
                "foreground",
                "muted",
                "accent",
                "working",
                "idle",
                "blocked",
                "done",
                "unknown",
            ]
            .contains(&k.as_str())
            {
                return Err(format!("unknown color: {k}").into());
            }
            color(v)?;
        }
        for v in self.kind_icons.values() {
            if v.is_empty() || v.chars().any(char::is_control) {
                return Err("icons must contain printable text".into());
            }
        }
        Ok(())
    }
    pub fn icon(&self, kind: &str) -> String {
        if self.icons == "ascii" || !self.emoji && self.icons == "emoji" {
            return match kind {
                "claude" => "CL",
                "codex" => "CX",
                "opencode" => "OC",
                _ => "AI",
            }
            .into();
        }
        if let Some(v) = self.kind_icons.get(kind) {
            return v.clone();
        }
        match (self.icons.as_str(), kind) {
            ("nerd", "claude") => "󰚩",
            ("nerd", "codex") => "󰘦",
            ("nerd", "opencode") => "",
            ("nerd", _) => "",
            (_, "claude") => "✳",
            (_, "codex") => "⚙",
            (_, "opencode") => "⌘",
            _ => "🤖",
        }
        .into()
    }
    pub fn spinner(&self, tick: usize) -> char {
        let frames = if self.icons == "ascii" {
            "|/-\\"
        } else {
            match self.spinner.as_str() {
                "line" => "|/-\\",
                "pulse" => "·∙●∙",
                _ => "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏",
            }
        };
        frames.chars().nth(tick % frames.chars().count()).unwrap()
    }
}
pub fn color(s: &str) -> Result<Color> {
    let h = s.strip_prefix('#').ok_or("colors require #RRGGBB")?;
    if h.len() != 6 || !h.is_ascii() {
        return Err("colors require #RRGGBB".into());
    }
    let n = u32::from_str_radix(h, 16)?;
    Ok(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}
pub struct Palette {
    pub bg: Color,
    pub surface: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub states: [Color; 5],
}
impl Palette {
    pub fn new(c: &Config) -> Self {
        let hex = match c.theme.as_str() {
            "dracula" => [
                "282a36", "21222c", "f8f8f2", "6272a4", "bd93f9", "8be9fd", "f1fa8c", "ff5555",
                "50fa7b", "6272a4",
            ],
            "tokyo-night" => [
                "1a1b26", "24283b", "c0caf5", "565f89", "bb9af7", "7dcfff", "e0af68", "f7768e",
                "9ece6a", "565f89",
            ],
            "nord" => [
                "2e3440", "3b4252", "eceff4", "8a98ad", "88c0d0", "81a1c1", "ebcb8b", "bf616a",
                "a3be8c", "8a98ad",
            ],
            "gruvbox" => [
                "282828", "32302f", "ebdbb2", "928374", "d3869b", "83a598", "fabd2f", "fb4934",
                "b8bb26", "928374",
            ],
            _ => [
                "1e1e2e", "181825", "cdd6f4", "7f849c", "cba6f7", "89b4fa", "f9e2af", "f38ba8",
                "a6e3a1", "7f849c",
            ],
        };
        let keys = [
            "background",
            "surface",
            "foreground",
            "muted",
            "accent",
            "working",
            "idle",
            "blocked",
            "done",
            "unknown",
        ];
        let p: Vec<Color> = keys
            .iter()
            .zip(hex)
            .map(|(k, v)| {
                color(
                    c.colors
                        .get(*k)
                        .map(String::as_str)
                        .unwrap_or(&format!("#{v}")),
                )
                .unwrap()
            })
            .collect();
        Self {
            bg: p[0],
            surface: p[1],
            fg: p[2],
            muted: p[3],
            accent: p[4],
            states: [p[5], p[6], p[7], p[8], p[9]],
        }
    }
}
pub fn blend(a: Color, b: Color, amount: f32) -> Color {
    match (a, b) {
        (Color::Rgb(r, g, b), Color::Rgb(x, y, z)) => {
            let m = |a: u8, b: u8| (a as f32 * (1. - amount) + b as f32 * amount) as u8;
            Color::Rgb(m(r, x), m(g, y), m(b, z))
        }
        _ => a,
    }
}
