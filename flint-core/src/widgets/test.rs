use ratatui::Frame;
use throbber_widgets_tui::ThrobberState;

use super::{AppStatus, AppWidget};

#[derive(Debug)]
pub struct TestWidget {
    _throbber_state: ThrobberState,
}

impl Default for TestWidget {
    fn default() -> Self {
        Self {
            _throbber_state: ThrobberState::default(),
        }
    }
}

impl AppWidget for TestWidget {
    fn draw(&mut self, _frame: &mut Frame) -> AppStatus {
        AppStatus::Ok
    }
}
