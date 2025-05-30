use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::Style,
    symbols::scrollbar::{HORIZONTAL, VERTICAL},
    widgets::Widget,
    Frame,
};

pub enum Orientation {
    Vertical,
    #[expect(unused)]
    Horizontal,
}

/// Widget that will render a scrollbar on the left or bottom edge
/// of a frame. To use, you must call [`draw_scrollbar()`]
struct Scrollbar {
    max: u16,
    pos: u16,
    orientation: Orientation,
}

impl Scrollbar {
    pub fn new(max: usize, pos: usize, orientation: Orientation) -> Self {
        Self {
            max: u16::try_from(max).unwrap_or_default(),
            pos: u16::try_from(pos).unwrap_or_default(),
            orientation,
        }
    }

    fn calc_pos(&self, scrollbar_lenght: u16) -> u16 {
        let progress = f32::from(self.pos) / f32::from(self.max);
        let progress = progress.min(1.0);
        let pos = f32::from(scrollbar_lenght) * progress;

        let pos: u16 = (pos + 0.5) as u16;
        pos.saturating_sub(1)
    }

    fn render_vertical(self, area: Rect, buf: &mut Buffer) {
        let area_right = area.right().saturating_sub(1);
        if area.height <= 2 || area_right <= area.left() {
            return;
        }

        let (bar_top, bar_height) = {
            let scrollbar_area = area.inner(Margin::new(0, 1));
            (scrollbar_area.top(), scrollbar_area.height)
        };

        let style = Style::default();
        for y in bar_top..(bar_top + bar_height) {
            buf.set_string(area_right, y, VERTICAL.track, style);
        }
        buf.set_string(
            area_right,
            bar_top + self.calc_pos(bar_height),
            VERTICAL.thumb,
            style,
        );
    }

    fn render_horizontal(self, area: Rect, buf: &mut Buffer) {
        let area_bottom = area.bottom().saturating_sub(1);
        if area.width <= 2 || area_bottom <= area.top() {
            return;
        }

        let (bar_left, bar_width) = {
            let scrollbar_area = area.inner(Margin::new(1, 0));
            (scrollbar_area.left(), scrollbar_area.width)
        };

        let style = Style::default();
        for x in bar_left..(bar_left + bar_width) {
            buf.set_string(x, area_bottom, HORIZONTAL.track, style);
        }
        buf.set_string(
            bar_left + self.calc_pos(bar_width),
            area_bottom,
            HORIZONTAL.thumb,
            style,
        );
    }
}

impl Widget for Scrollbar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.max == 0 {
            return;
        }

        match &self.orientation {
            Orientation::Vertical => self.render_vertical(area, buf),
            Orientation::Horizontal => {
                self.render_horizontal(area, buf);
            }
        }
    }
}

/// Draw a scrollbar on top of the border.
///
/// The scrollbar is a double line and the locator is a filled block,
/// so it works best with single-line border.
///
/// The rect you provide is the same as that used
/// to draw the border. The scrollbar will be draw on top of the left
/// or bottom border, dependent on the orientation of the scrollbar.
pub fn draw_scrollbar(f: &mut Frame, r: Rect, max: usize, pos: usize, orientation: Orientation) {
    let widget = Scrollbar::new(max, pos, orientation);
    f.render_widget(widget, r);
}
