use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::sync::LazyLock;

use regex::Regex;

/// Regex to match OSC 8 hyperlink escape sequences.
/// Format: ESC ] 8 ; ; URL ST  where ST is either ESC \ or BEL (\x07)
/// This matches both the opening tag (with URL) and closing tag (empty URL)
static OSC8_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\]8;[^;]*;[^\x07\x1b]*(?:\x07|\x1b\\)").unwrap());

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

/// Center a rect of fixed width and height within an outside rect
pub fn centered_rect_fixed(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

/// Strip OSC 8 hyperlink escape sequences from a string.
///
/// OSC 8 is used by terminals to create clickable hyperlinks:
/// `\x1b]8;;URL\x1b\\text\x1b]8;;\x1b\\`
///
/// These sequences are emitted by tools like delta but don't render
/// well in TUI applications, so we strip them while preserving the
/// visible text content.
pub fn strip_osc8_hyperlinks(text: &str) -> String {
    OSC8_REGEX.replace_all(text, "").into_owned()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_osc8_hyperlinks_with_st_terminator() {
        // OSC 8 with ST (ESC \) terminator
        let input = "\x1b]8;;https://example.com\x1b\\Click here\x1b]8;;\x1b\\";
        let result = strip_osc8_hyperlinks(input);
        assert_eq!(result, "Click here");
    }

    #[test]
    fn test_strip_osc8_hyperlinks_with_bel_terminator() {
        // OSC 8 with BEL (\x07) terminator
        let input = "\x1b]8;;https://example.com\x07Click here\x1b]8;;\x07";
        let result = strip_osc8_hyperlinks(input);
        assert_eq!(result, "Click here");
    }

    #[test]
    fn test_strip_osc8_hyperlinks_preserves_other_ansi() {
        // Should preserve CSI color codes while stripping OSC 8
        let input = "\x1b[32m\x1b]8;;url\x1b\\green link\x1b]8;;\x1b\\\x1b[0m";
        let result = strip_osc8_hyperlinks(input);
        assert_eq!(result, "\x1b[32mgreen link\x1b[0m");
    }

    #[test]
    fn test_strip_osc8_hyperlinks_no_links() {
        let input = "plain text with \x1b[31mcolors\x1b[0m";
        let result = strip_osc8_hyperlinks(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_strip_osc8_hyperlinks_delta_style() {
        // Real delta output style: ESC ] 8 ; ; URL ESC \ text ESC ] 8 ; ; ESC \
        // Using raw bytes to match actual delta output
        let input = "\x1b]8;;vscode-insiders://file/path.rs\x1b\\file.rs\x1b]8;;\x1b\\";
        let result = strip_osc8_hyperlinks(input);
        // Should strip the OSC 8 sequences, keeping just the text
        assert_eq!(result, "file.rs");
    }

    #[test]
    fn test_strip_osc8_hyperlinks_multiline() {
        // Multiple hyperlinks across lines
        let input = "\x1b]8;;url1\x1b\\line1\x1b]8;;\x1b\\\n\x1b]8;;url2\x1b\\line2\x1b]8;;\x1b\\";
        let result = strip_osc8_hyperlinks(input);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_strip_osc8_real_delta_output() {
        // Real delta output with OSC 8 and ANSI colors mixed
        // Format: CSI color code, then OSC 8 hyperlink with vscode URL
        let input = "\x1b[34mΔ \x1b]8;;vscode://file/path.rs:\x1b\\left/path.rs\x1b]8;;\x1b\\ ⟶ right\x1b[0m";
        let result = strip_osc8_hyperlinks(input);
        // Should keep ANSI colors but strip OSC 8 hyperlinks
        assert_eq!(result, "\x1b[34mΔ left/path.rs ⟶ right\x1b[0m");
        assert!(!result.contains("]8;;"));
    }
}
