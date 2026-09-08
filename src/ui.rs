use crate::{
    model::{clean, Card, EMOJI, STATES},
    theme::{blend, Config, Palette},
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Sparkline, Widget, Wrap},
};

pub fn segment(counts: [usize; 5], c: &Config, plain: bool) -> String {
    let p = Palette::new(c);
    counts
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(i, n)| {
            let s = if c.emoji && c.icons != "ascii" {
                format!("{}{n}", EMOJI[i])
            } else {
                format!("{}:{n}", STATES[i])
            };
            if plain {
                s
            } else if let Color::Rgb(r, g, b) = p.states[i] {
                format!("\x1b[38;2;{r};{g};{b}m{s}\x1b[0m")
            } else {
                s
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    cards: &[Card],
    selected: usize,
    c: &Config,
    tick: usize,
    message: &str,
) {
    let p = Palette::new(c);
    Block::new()
        .style(Style::default().bg(p.bg).fg(p.fg))
        .render(area, buf);
    if area.width < 16 || area.height < 6 {
        Paragraph::new("Enlarge pane").render(area, buf);
        return;
    }
    let mut counts = [0; 5];
    for card in cards {
        counts[card.agent.state()] += 1;
    }
    let narrow = area.width < 49;
    Paragraph::new(Line::from(vec![
        Span::styled(
            if narrow {
                " GLANCE "
            } else {
                " HERDR / GLANCE "
            },
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if narrow {
                format!(" {} agents", cards.len())
            } else {
                format!(" {} agents   {}", cards.len(), c.theme)
            },
            Style::default().fg(p.muted),
        ),
    ]))
    .render(Rect::new(area.x, area.y, area.width, 1), buf);
    Paragraph::new(if narrow {
        format!(" {}  {}", segment(counts, c, true), c.theme)
    } else {
        format!(" {}", segment(counts, c, true))
    })
    .style(Style::default().fg(p.fg))
    .render(Rect::new(area.x, area.y + 1, area.width, 1), buf);
    let footer = if message.is_empty() {
        if narrow {
            "j/k select  Enter focus\nt theme  e emoji  q quit"
        } else {
            "j/k select  Enter focus  t theme  e emoji  q quit"
        }
    } else {
        message
    };
    Paragraph::new(clean(footer))
        .style(Style::default().fg(if message.is_empty() {
            p.muted
        } else {
            p.states[2]
        }))
        .render(
            Rect::new(
                area.x + 1,
                area.bottom() - if narrow { 2 } else { 1 },
                area.width - 2,
                if narrow { 2 } else { 1 },
            ),
            buf,
        );
    let content = Rect::new(
        area.x + 1,
        area.y + 3,
        area.width - 2,
        area.height - if narrow { 6 } else { 5 },
    );
    if cards.is_empty() {
        Paragraph::new("No live agents in this scope.\nPolling read-only every second.")
            .style(Style::default().fg(p.muted))
            .render(content, buf);
        return;
    }
    let cols = (content.width / 43).max(1) as usize;
    let width = content.width / cols as u16;
    let mut rows = Vec::new();
    for start in (0..cards.len()).step_by(cols) {
        let blocked = cards[start..(start + cols).min(cards.len())]
            .iter()
            .any(|c| c.agent.state() == 2);
        rows.push((start, if blocked { 12u16 } else { 8u16 }));
    }
    let selected_row = selected.min(cards.len() - 1) / cols;
    let mut first = 0;
    while first < selected_row
        && rows[first..=selected_row]
            .iter()
            .map(|(_, h)| h + 1)
            .sum::<u16>()
            .saturating_sub(1)
            > content.height
    {
        first += 1;
    }
    let mut y = content.y;
    for &(start, height) in &rows[first..] {
        if y >= content.bottom() {
            break;
        }
        let h = height.min(content.bottom() - y);
        for col in 0..cols {
            let i = start + col;
            if i >= cards.len() {
                break;
            }
            let x = content.x + col as u16 * width;
            let w = if col + 1 == cols {
                content.right() - x
            } else {
                width - 1
            };
            card(
                Rect::new(x, y, w, h),
                buf,
                &cards[i],
                i == selected,
                c,
                &p,
                tick,
            );
        }
        y = y.saturating_add(h + 1);
    }
}
fn card(
    r: Rect,
    buf: &mut Buffer,
    card: &Card,
    selected: bool,
    c: &Config,
    p: &Palette,
    tick: usize,
) {
    if r.width < 4 || r.height < 3 {
        return;
    }
    let state = card.agent.state();
    let color = if state == 2 {
        blend(
            p.states[2],
            p.surface,
            0.12 + 0.28 * ((tick as f32 * 0.32).sin() + 1.) / 2.,
        )
    } else {
        p.states[state]
    };
    let border = if selected { p.accent } else { color };
    Block::new()
        .borders(Borders::ALL)
        .border_type(if c.icons == "ascii" {
            BorderType::Plain
        } else {
            BorderType::Rounded
        })
        .style(Style::default().bg(p.surface).fg(p.fg))
        .border_style(Style::default().fg(border))
        .render(r, buf);
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            if y == r.y || y == r.bottom() - 1 || x == r.x || x == r.right() - 1 {
                buf[(x, y)].set_fg(blend(
                    border,
                    p.muted,
                    (x - r.x) as f32 / r.width as f32 * 0.65,
                ));
                if c.icons == "ascii" {
                    buf[(x, y)].set_symbol(
                        if (x == r.x || x == r.right() - 1) && (y == r.y || y == r.bottom() - 1) {
                            "+"
                        } else if y == r.y || y == r.bottom() - 1 {
                            "-"
                        } else {
                            "|"
                        },
                    );
                }
            }
        }
    }
    let inner = Rect::new(r.x + 2, r.y + 1, r.width.saturating_sub(4), r.height - 2);
    let kind = card.agent.agent.as_deref().unwrap_or("unknown");
    let title = format!(
        "{} {}  {}",
        if selected {
            if c.icons == "ascii" {
                ">"
            } else {
                "›"
            }
        } else {
            " "
        },
        c.icon(kind),
        clean(card.agent.name())
    );
    Paragraph::new(title)
        .style(Style::default().fg(p.fg).add_modifier(Modifier::BOLD))
        .render(Rect::new(inner.x, inner.y, inner.width, 1), buf);
    if inner.height < 2 {
        return;
    }
    let secs = card.since.elapsed().as_secs();
    let badge = if c.emoji && c.icons != "ascii" {
        EMOJI[state]
    } else {
        ""
    };
    let spinner = if state == 0 { c.spinner(tick) } else { ' ' };
    Paragraph::new(format!(
        "{badge} {} {spinner}  {:02}:{:02}  {}",
        STATES[state],
        secs / 60,
        secs % 60,
        clean(kind)
    ))
    .style(Style::default().fg(color))
    .render(Rect::new(inner.x, inner.y + 1, inner.width, 1), buf);
    if inner.height >= 3 {
        if c.icons == "ascii" {
            let s: String = card
                .activity
                .iter()
                .map(|n| if *n == 0 { '.' } else { '#' })
                .collect();
            Paragraph::new(s)
                .style(Style::default().fg(color))
                .render(Rect::new(inner.x, inner.y + 2, inner.width, 1), buf);
        } else {
            Sparkline::default()
                .data(&card.activity)
                .max(12)
                .style(Style::default().fg(color))
                .render(Rect::new(inner.x, inner.y + 2, inner.width, 1), buf);
        }
    }
    if inner.height >= 5 {
        let last = card.lines[card.lines.len().saturating_sub(2)..].join("\n");
        Paragraph::new(last)
            .style(Style::default().fg(p.muted))
            .render(Rect::new(inner.x, inner.y + 3, inner.width, 2), buf);
    }
    if inner.height > 5 && state == 2 {
        let text = if card.detection.trim().is_empty() {
            "Detection source has no question text."
        } else {
            &card.detection
        };
        Paragraph::new("NEEDS YOU - Enter for full context")
            .style(Style::default().fg(color))
            .render(Rect::new(inner.x, inner.y + 5, inner.width, 1), buf);
        let question = Paragraph::new(text.trim())
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: false });
        let height = inner.height - 6;
        let scroll = question
            .line_count(inner.width)
            .saturating_sub(height as usize);
        question
            .scroll((scroll.min(u16::MAX as usize) as u16, 0))
            .render(Rect::new(inner.x, inner.y + 6, inner.width, height), buf);
    }
    if let Some(e) = &card.error {
        Paragraph::new(clean(&format!("Read failed: {e}")))
            .style(Style::default().fg(p.states[2]))
            .render(Rect::new(inner.x, r.bottom() - 2, inner.width, 1), buf);
    }
}
