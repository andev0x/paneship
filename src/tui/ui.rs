//! Ratatui rendering for the configurator (Phase 2, Module C).
//!
//! Split-pane layout: the left pane lists configuration sections; the right
//! pane shows a **live preview** of the rendered prompt on top of the fields
//! for the selected section. The preview converts the renderer's ANSI output
//! into styled ratatui spans.

use super::app::{App, FieldKind, SECTIONS};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

/// Draw one frame of the configurator.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let rows = Layout::vertical([Constraint::Min(6), Constraint::Length(3)]).split(frame.area());
    let body = rows[0];
    let footer = rows[1];

    let columns = Layout::horizontal([Constraint::Length(22), Constraint::Min(24)]).split(body);
    draw_sections(frame, app, columns[0]);

    let right = Layout::vertical([Constraint::Length(6), Constraint::Min(3)]).split(columns[1]);
    // Reserve room for the block borders so the preview renders at its true width.
    app.preview_width = right[0].width.saturating_sub(2).max(20);
    draw_preview(frame, app, right[0]);
    draw_fields(frame, app, right[1]);
    draw_footer(frame, app, footer);
}

fn draw_sections(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = SECTIONS
        .iter()
        .map(|section| ListItem::new(section.title))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Sections"))
        .highlight_symbol("▶ ")
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        );

    let mut state = ListState::default();
    state.select(Some(app.section));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_fields(frame: &mut Frame, app: &App, area: Rect) {
    let section = app.current_section();
    let items: Vec<ListItem> = section
        .fields
        .iter()
        .map(|def| {
            let value = match &app.edit {
                Some(edit) if path_matches(&edit.path, def.path) => {
                    format!("{}\u{258f}", edit.buffer)
                }
                _ => {
                    let raw = app.field_value(def);
                    if matches!(def.kind, FieldKind::Choice(_)) {
                        match app.choice_label(def, &raw) {
                            Some(label) => format!("{raw}  ({label})"),
                            None => format!("{raw}  (Enter to cycle)"),
                        }
                    } else {
                        raw
                    }
                }
            };
            ListItem::new(format!("{}: {}", def.label, value))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Values"))
        .highlight_symbol("> ")
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        );

    let mut state = ListState::default();
    state.select(Some(app.field));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_preview(frame: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = app.preview().iter().map(|line| styled_line(line)).collect();

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title("Live Preview"))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let key = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);

    let hint = Line::from(vec![
        Span::styled(" ↑/↓ ", key),
        Span::styled("field", dim),
        Span::styled("  ←/→ ", key),
        Span::styled("section", dim),
        Span::styled("  Enter ", key),
        Span::styled("edit", dim),
        Span::styled("  Ctrl+S ", key),
        Span::styled("save", dim),
        Span::styled("  q ", key),
        Span::styled("quit", dim),
    ]);

    let status_color = if app.dirty {
        Color::Yellow
    } else {
        Color::Green
    };
    let dirty = if app.dirty { " ●" } else { "" };
    let status = Line::from(Span::styled(
        format!(" {}{}", app.status, dirty),
        Style::default().fg(status_color),
    ));

    let text = Text::from(vec![hint, status]);
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::TOP)),
        area,
    );
}

fn path_matches(path: &[String], def: &[&str]) -> bool {
    path.len() == def.len() && path.iter().zip(def).all(|(a, b)| a == b)
}

// ---------------------------------------------------------------------------
// Minimal ANSI SGR → ratatui span converter (foreground + bold/dim/underline)
// ---------------------------------------------------------------------------

/// Convert a renderer line (which may contain ANSI SGR sequences) into a
/// ratatui [`Line`] with coloured spans.
pub fn styled_line(input: &str) -> Line<'static> {
    let mut style = Style::default();
    let mut spans = Vec::new();
    let mut buffer = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            let mut params = String::new();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
                params.push(c);
            }
            if !buffer.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut buffer), style));
            }
            style = apply_sgr(style, &params);
            continue;
        }
        buffer.push(ch);
    }

    if !buffer.is_empty() {
        spans.push(Span::styled(buffer, style));
    }

    Line::from(spans)
}

fn apply_sgr(mut style: Style, params: &str) -> Style {
    let codes: Vec<u16> = params
        .split(';')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect();

    if codes.is_empty() {
        return Style::default();
    }

    let mut index = 0;
    while index < codes.len() {
        match codes[index] {
            0 => style = Style::default(),
            1 => style = style.add_modifier(Modifier::BOLD),
            2 => style = style.add_modifier(Modifier::DIM),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            30..=37 => style = style.fg(basic_color(codes[index] - 30)),
            39 => style = style.fg(Color::Reset),
            90..=97 => style = style.fg(bright_color(codes[index] - 90)),
            38 => {
                if let Some((color, extra)) = extended_foreground(&codes, index) {
                    style = style.fg(color);
                    index += extra;
                }
            }
            _ => {}
        }
        index += 1;
    }

    style
}

/// Decode `38;5;n` (256-colour) and `38;2;r;g;b` (truecolour) foreground codes.
/// Returns the colour and how many trailing codes it consumed.
fn extended_foreground(codes: &[u16], index: usize) -> Option<(Color, usize)> {
    match *codes.get(index + 1)? {
        5 => Some((Color::Indexed(*codes.get(index + 2)? as u8), 2)),
        2 => Some((
            Color::Rgb(
                *codes.get(index + 2)? as u8,
                *codes.get(index + 3)? as u8,
                *codes.get(index + 4)? as u8,
            ),
            4,
        )),
        _ => None,
    }
}

fn basic_color(code: u16) -> Color {
    match code {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        _ => Color::White,
    }
}

fn bright_color(code: u16) -> Color {
    match code {
        0 => Color::DarkGray,
        1 => Color::LightRed,
        2 => Color::LightGreen,
        3 => Color::LightYellow,
        4 => Color::LightBlue,
        5 => Color::LightMagenta,
        6 => Color::LightCyan,
        _ => Color::White,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_is_converted_to_colored_spans() {
        let line = styled_line("\u{1b}[1;36mmain\u{1b}[0m plain");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "main plain");
        assert!(line.spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn plain_text_is_single_span() {
        let line = styled_line("hello");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "hello");
    }
}
