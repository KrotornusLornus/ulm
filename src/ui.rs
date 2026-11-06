use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, Focus, INPUT_WIDTH};
use crate::theme::{BLACK, GRAY, RED, WHITE};

pub fn draw(frame: &mut Frame, app: &App, cursor_on: bool) {
    let full = frame.area();
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GRAY))
        .style(Style::default().bg(BLACK).fg(WHITE));
    let area = outer.inner(full);
    frame.render_widget(outer, full);

    if area.width < 78 || area.height < 22 {
        let message = format!(
            "TERMINAL TOO SMALL\nMIN 80x24 - CURRENT {}x{}",
            area.width, area.height
        );
        frame.render_widget(
            Paragraph::new(message)
                .alignment(Alignment::Center)
                .style(Style::default().fg(RED)),
            area,
        );
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(6),
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Length(3),
    ])
    .split(area);

    draw_header(frame, chunks[0], area);
    draw_divider(frame, chunks[1], full);
    draw_form(frame, chunks[3], app, cursor_on);
    draw_help(frame, chunks[5]);
    draw_warning(frame, chunks[6], full);
}

fn draw_header(frame: &mut Frame, area: Rect, inner: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "UMBRELLA CORPORATION",
            Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "CLASSIFIED TERMINAL ACCESS",
            Style::default().fg(GRAY),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("SYSTEM: ", Style::default().fg(GRAY)),
            Span::styled("BUENOS AIRES-SEC-01", Style::default().fg(WHITE)),
        ]),
        Line::from(vec![
            Span::styled("LEVEL:  ", Style::default().fg(GRAY)),
            Span::styled("4 AUTHORIZED", Style::default().fg(RED)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "AUTHORIZED PERSONNEL ONLY - LEVEL 4 CLEARANCE",
            Style::default().fg(GRAY),
        )),
    ];

    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);

    if inner.width >= 40 {
        let logo_area = Rect {
            x: inner.x + 2,
            y: area.y,
            width: 4,
            height: 2,
        };
        let logo = vec![logo_line(RED, WHITE), logo_line(WHITE, RED)];
        frame.render_widget(Paragraph::new(logo), logo_area);
    }
}

fn logo_line(left: ratatui::style::Color, right: ratatui::style::Color) -> Line<'static> {
    Line::from(vec![
        Span::styled("██", Style::default().fg(left)),
        Span::styled("██", Style::default().fg(right)),
    ])
}

fn draw_divider(frame: &mut Frame, area: Rect, full: Rect) {
    let line = format!("├{}┤", "─".repeat(full.width.saturating_sub(2) as usize));
    let rect = Rect {
        x: full.x,
        y: area.y,
        width: full.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(line).style(Style::default().fg(GRAY)), rect);
}

fn draw_form(frame: &mut Frame, area: Rect, app: &App, cursor_on: bool) {
    let form_width = 14 + 3 + 1 + INPUT_WIDTH as u16 + 1;
    let form_area = centered_rect(area, form_width, 6);

    let mut lines = vec![
        Line::from(Span::styled(
            "PLEASE IDENTIFY YOURSELF",
            Style::default().fg(GRAY),
        )),
        field_line(
            "EMPLOYEE ID",
            &app.user,
            app.focus == Focus::User,
            false,
            cursor_on,
        ),
        Line::from(""),
        field_line(
            "PASSCODE",
            &app.pass,
            app.focus == Focus::Pass,
            true,
            cursor_on,
        ),
        Line::from(""),
    ];

    if app.granted {
        lines.push(Line::from(Span::styled(
            "  ACCESS GRANTED  ",
            Style::default()
                .fg(BLACK)
                .bg(RED)
                .add_modifier(Modifier::BOLD),
        )));
    } else if let Some(error) = &app.error {
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(RED),
        )));
    } else {
        lines.push(Line::from(""));
    }

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        form_area,
    );
}

fn field_line(
    label: &str,
    value: &str,
    focused: bool,
    masked: bool,
    cursor_on: bool,
) -> Line<'static> {
    let shown = if masked {
        "*".repeat(value.chars().count())
    } else {
        value.to_string()
    };

    let cursor_slot = usize::from(focused);
    let limit = INPUT_WIDTH.saturating_sub(cursor_slot);
    let mut chars: Vec<char> = shown.chars().collect();
    if chars.len() > limit {
        chars = chars[chars.len() - limit..].to_vec();
    }
    let text: String = chars.into_iter().collect();
    let text_len = text.chars().count();

    let mut spans = vec![
        Span::styled(
            format!("{label:>14}"),
            Style::default().fg(if focused { WHITE } else { GRAY }),
        ),
        Span::styled(" : [", Style::default().fg(GRAY)),
    ];

    if text_len == 0 {
        if focused {
            if cursor_on {
                spans.push(Span::styled("█", Style::default().fg(RED)));
            } else {
                spans.push(Span::raw(" "));
            }
        }

        spans.push(Span::raw(
            " ".repeat(INPUT_WIDTH.saturating_sub(cursor_slot)),
        ));
    } else {
        spans.push(Span::styled(text, Style::default().fg(WHITE)));
        if focused {
            if cursor_on {
                spans.push(Span::styled("█", Style::default().fg(RED)));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        spans.push(Span::raw(
            " ".repeat(INPUT_WIDTH.saturating_sub(text_len + cursor_slot)),
        ));
    }

    spans.push(Span::styled("]", Style::default().fg(GRAY)));

    Line::from(spans)
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("[ ENTER ]", Style::default().fg(WHITE)),
        Span::styled(" Authenticate    ", Style::default().fg(GRAY)),
        Span::styled("[ TAB ]", Style::default().fg(WHITE)),
        Span::styled(" Next field    ", Style::default().fg(GRAY)),
        Span::styled("[ ESC ]", Style::default().fg(WHITE)),
        Span::styled(" Abort", Style::default().fg(GRAY)),
    ]);

    frame.render_widget(Paragraph::new(help).alignment(Alignment::Center), area);
}

fn draw_warning(frame: &mut Frame, area: Rect, full: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let line = format!("├{}┤", "─".repeat(full.width.saturating_sub(2) as usize));
    let rect = Rect {
        x: full.x,
        y: rows[0].y,
        width: full.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(line).style(Style::default().fg(GRAY)), rect);
    frame.render_widget(
        Paragraph::new("WARNING: UNSANCTIONED ACCESS WILL TRIGGER BIO-CONTAINMENT PROTOCOLS")
            .alignment(Alignment::Center)
            .style(Style::default().fg(RED)),
        rows[1],
    );
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}
