mod layout;

use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    layout::draw(frame, app);
}
