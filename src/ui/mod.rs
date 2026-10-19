pub mod cover;
pub mod glyphs;
pub mod keymap;
pub mod layout;
pub mod theme;
pub mod track;

use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    layout::draw(frame, app);
}
