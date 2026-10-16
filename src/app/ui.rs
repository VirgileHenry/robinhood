use ratatui::prelude::*;

pub fn draw(f: &mut Frame, app: &crate::app::App) {
    let constraints = [
        Constraint::Length(5),
        Constraint::Percentage(60),
        Constraint::Percentage(40),
        Constraint::Length(1),
    ];
    let [header, top, bottom, footer] = Layout::vertical(constraints).areas(f.area());

    draw_header(f, header);
    draw_downloading_list(f, top, " DOWNLOADING ", &app.downloading);
    draw_downloaded_list(f, bottom, " SAVED ", &app.downloaded);

    f.render_widget(Line::from(" ctrl-c to quit").style(Style::new().fg(Color::DarkGray)), footer);
}

fn draw_header(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from("Robin Hood - Spotify song downloader").style(Style::new().fg(Color::Green).bold()),
        Line::from("  How to use:").style(Style::new().fg(Color::White)),
        Line::from("    1 - Connect to your spotify account on the browser").style(Style::new().fg(Color::White)),
        Line::from("    2 - Go on spotify and select the device \"Robin Hood\"").style(Style::new().fg(Color::White)),
        Line::from("    3 - Play any music, they will appear here").style(Style::new().fg(Color::White)),
    ];
    f.render_widget(Text::from(lines), area);
}

/// Left panel: one line per item, each "label [===>   ]" where ratio is 0.0..=1.0.
fn draw_downloading_list(
    f: &mut Frame,
    area: Rect,
    title: &str,
    items: &std::collections::BTreeMap<String, crate::save_track::SaveTrackHandle>,
) {
    let block = ratatui::widgets::Block::bordered().title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // one row per item, each one line tall
    let rows = Layout::vertical(std::iter::repeat(Constraint::Length(1)).take(items.len())).split(inner);

    for ((label, ratio), row) in items.iter().zip(rows.iter()) {
        // split the row 50/50: name | bar
        let [name_area, bar_area] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(*row);

        f.render_widget(Line::from(truncate(label, name_area.width as usize)), name_area);
        f.render_widget(Line::from(bar_spans(ratio.progress, bar_area.width as usize)), bar_area);
    }
}

/// Right panel: one line of plain text per item.
fn draw_downloaded_list(f: &mut Frame, area: Rect, title: &str, items: &[String]) {
    let block = ratatui::widgets::Block::bordered().title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines: Vec<Line> = items.iter().map(|s| Line::from(s.as_str())).collect();
    f.render_widget(Text::from(lines), inner);
}

fn bar_spans(ratio: f32, width: usize) -> Vec<Span<'static>> {
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = ((ratio * width as f32).round() as usize).min(width);
    let color = heat(ratio); // one colour for the whole bar, based on progress

    let mut spans = vec![Span::raw("[")];
    for i in 0..(width - 2) {
        if i < filled {
            let ch = if i + 1 == filled && filled < width { '>' } else { '=' };
            spans.push(Span::styled(ch.to_string(), Style::new().fg(color)));
        } else {
            spans.push(Span::raw(" "));
        }
    }
    spans.push(Span::raw("]"));
    spans
}

/// 0.0 = red, ~0.33 = orange, ~0.66 = yellow, 1.0 = green.
fn heat(t: f32) -> Color {
    // hue 0° (red) → 120° (green); orange and yellow fall out naturally in between.
    let (r, g) = if t < 0.5 {
        (255, (t * 2.0 * 255.0) as u8) // red → yellow
    } else {
        (((1.0 - t) * 2.0 * 255.0) as u8, 255) // yellow → green
    };
    Color::Rgb(r, g, 0)
}

fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
