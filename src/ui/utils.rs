use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn centered_rect_line_height(r: Rect, percent_x: u16, lines_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(lines_y),
            Constraint::Fill(1),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// replaces tabs in a string by spaces
///
/// ratatui doesn't work well displaying tabs, so any
/// string that is rendered and might contain tabs
/// needs to have the tabs converted to spaces.
///
/// this function aligns tabs in the input string to
/// virtual tab stops 4 spaces apart, taking care
/// to count ansi control sequences as zero width.
pub fn tabs_to_spaces(line: &str) -> String {
    const TAB_WIDTH: usize = 4;

    enum AnsiState {
        Neutral,
        Escape,
        Csi,
    }

    let mut out = String::new();
    let mut x = 0;
    let mut ansi_state = AnsiState::Neutral;
    for c in line.chars() {
        match ansi_state {
            AnsiState::Neutral => {
                if c == '\t' {
                    loop {
                        out.push(' ');
                        x += 1;
                        if x % TAB_WIDTH == 0 {
                            break;
                        }
                    }
                } else {
                    out.push(c);
                    if c == '\x1b' {
                        ansi_state = AnsiState::Escape;
                    } else {
                        x += 1;
                    }
                }
                if c == '\r' || c == '\n' {
                    x = 0;
                }
            }
            AnsiState::Escape => {
                out.push(c);
                ansi_state = if c == '[' {
                    AnsiState::Csi
                } else {
                    AnsiState::Neutral
                };
            }
            AnsiState::Csi => {
                out.push(c);
                if ('\x40'..='\x7f').contains(&c) {
                    ansi_state = AnsiState::Neutral;
                }
            }
        }
    }
    out
}
