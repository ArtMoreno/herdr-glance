use super::*;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fixture() -> Vec<Card> {
    let v: Value = serde_json::from_str(include_str!("../tests/fixtures/agents.json")).unwrap();
    let reads: Value = serde_json::from_str(include_str!("../tests/fixtures/reads.json")).unwrap();
    let detection: Value =
        serde_json::from_str(include_str!("../tests/fixtures/detection.json")).unwrap();
    model::agents(&v, "w-demo")
        .unwrap()
        .into_iter()
        .map(|a| {
            let text = model::read_text_fixture(&reads[&a.pane_id], &a).unwrap();
            let question = if a.state() == 2 {
                model::read_text_fixture(&detection, &a).unwrap()
            } else {
                String::new()
            };
            let mut c = Card::new(a.clone());
            c.update(a, text, question);
            c.activity = (0..32)
                .map(|i| {
                    if c.agent.state() == 0 {
                        [0, 2, 5, 3, 8, 4, 12, 6][i % 8]
                    } else {
                        [0, 0, 1, 0, 2, 0, 0, 0][i % 8]
                    }
                })
                .collect();
            c
        })
        .collect()
}
fn snapshot(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in buf.area.y..buf.area.bottom() {
        for x in buf.area.x..buf.area.right() {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    // Include each styled cell, so palette and border regressions are visible in snapshots.
    for cell in &buf.content {
        out.push_str(&format!("{:?}/{:?};", cell.fg, cell.bg));
    }
    out.push('\n');
    out
}
fn hex(c: Color) -> String {
    if let Color::Rgb(r, g, b) = c {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        "#ffffff".into()
    }
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn svg(buf: &Buffer) -> String {
    let mut s=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><title>herdr-glance fixture render</title>",buf.area.width*10,buf.area.height*20,buf.area.width*10,buf.area.height*20);
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let c = &buf[(x, y)];
            s.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"10\" height=\"20\" fill=\"{}\"/>",
                x * 10,
                y * 20,
                hex(c.bg)
            ));
        }
    }
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let c = &buf[(x, y)];
            if c.symbol().trim().is_empty() {
                continue;
            }
            s.push_str(&format!("<text x=\"{}\" y=\"{}\" fill=\"{}\" font-family=\"Cascadia Mono,DejaVu Sans Mono,monospace\" font-size=\"14\">{}</text>",x*10,y*20+15,hex(c.fg),xml(c.symbol())));
        }
    }
    s.push_str("</svg>");
    s
}
#[test]
fn rendering_snapshots() {
    let cards = fixture();
    for theme in THEMES {
        for (w, h, label) in [(100, 34, "wide"), (38, 25, "narrow")] {
            let c = Config {
                theme: theme.into(),
                ..Config::default()
            };
            let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
            ui::render(buf.area, &mut buf, &cards, 0, &c, 0, "");
            let path = format!("tests/snapshots/{theme}-{label}.snap");
            if env::var_os("UPDATE_SNAPSHOTS").is_some() {
                fs::create_dir_all("tests/snapshots").unwrap();
                fs::write(&path, snapshot(&buf)).unwrap();
                if label == "wide" {
                    fs::write(format!("docs/screenshots/{theme}.svg"), svg(&buf)).unwrap();
                }
            }
            assert_eq!(
                snapshot(&buf),
                fs::read_to_string(&path).expect("run UPDATE_SNAPSHOTS=1 cargo test once"),
                "{path}"
            );
        }
    }
}
#[test]
fn parsing_delta_and_safety() {
    let cards = fixture();
    assert_eq!(cards.len(), 5);
    assert_eq!(cards[0].agent.state(), 2);
    assert!(model::agents(&json!({"error":{"message":"unavailable"}}), "w-demo").is_err());
    assert!(model::agents(&json!({"agents":[{"pane_id":"x"}]}), "w-demo").is_err());
    assert!(model::read_text_fixture(
        &json!({"read":{"pane_id":"wrong","workspace_id":"w-demo","text":"bad"}}),
        &cards[0].agent
    )
    .is_err());
    let lines = |s: &str| s.lines().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(model::line_delta(&lines("a\nb\nc"), &lines("b\nc\nd")), 1);
    assert_eq!(model::line_delta(&lines("a\nb"), &lines("a\nb")), 0);
    assert_eq!(model::line_delta(&lines("a\nb"), &lines("x\ny")), 2);
    assert_eq!(
        model::clean("\x1b[31mred\x1b[0m\x1b]52;c;secret\x07\nhello\r"),
        "red\nhello"
    );
    assert_eq!(
        model::clean("\x1b]unterminated\nWhich icon set?"),
        "\nWhich icon set?"
    );
    assert_eq!(model::clean("\x1b(Bplain\u{200e}"), "plain");
    let mut c = Config::default();
    assert_eq!(ui::segment([2, 0, 1, 3, 0], &c, true), "🧠2 🙋1 ✅3");
    c.emoji = false;
    assert_eq!(
        ui::segment([2, 0, 1, 3, 0], &c, true),
        "working:2 blocked:1 done:3"
    );
    c.colors.insert("blocked".into(), "invalid".into());
    assert!(c.validate().is_err());
}
#[test]
fn tiny_scrolling_and_state_changes() {
    let mut cards = fixture();
    let old = Instant::now() - Duration::from_secs(60);
    cards[0].since = old;
    let mut a = cards[0].agent.clone();
    a.state_change_seq += 1;
    cards[0].update(a, "changed".into(), "question".into());
    assert!(cards[0].since > old);
    for (w, h) in [(0, 0), (1, 1), (15, 5), (30, 10), (38, 25), (170, 50)] {
        for selected in 0..cards.len() {
            let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
            ui::render(
                buf.area,
                &mut buf,
                &cards,
                selected,
                &Config::default(),
                12,
                "",
            );
            if w == 38 && h == 25 {
                let text = snapshot(&buf);
                assert!(text.contains(cards[selected].agent.name()));
            }
        }
    }
}

#[test]
fn blocked_excerpt_keeps_latest_prompt_and_narrow_controls() {
    let mut cards = fixture();
    cards[0].detection = format!(
        "{}\nWhich icon set?\n\n",
        "Old terminal history\n".repeat(50)
    );
    let mut buf = Buffer::empty(Rect::new(0, 0, 38, 25));
    ui::render(buf.area, &mut buf, &cards, 0, &Config::default(), 0, "");
    let text = snapshot(&buf);
    assert!(text.contains("Which icon set?"));
    assert!(text.contains("t theme  e emoji  q quit"));
}
#[test]
fn cache_paths_are_scoped() {
    let h = Herdr {
        session: "fixture".into(),
        workspace: "w-demo".into(),
        binary: "fake-herdr".into(),
    };
    let mut other = h.clone();
    other.workspace = "other".into();
    assert_ne!(h.cache_path(), other.cache_path());
    assert!(Path::new(&h.cache_path()).extension().is_some());
}
