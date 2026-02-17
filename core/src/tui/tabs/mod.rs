pub mod console;
pub mod summary;
pub mod performance;
pub mod warnings;
pub mod history;

use ratatui::{
    layout::Rect,
    Frame,
};

pub trait Tab {
    fn title(&self) -> &str;
    fn render(&self, frame: &mut Frame, area: Rect);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Console = 0,
    Summary = 1,
    Performance = 2,
    Warnings = 3,
    History = 4,
}

impl TabId {
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(TabId::Console),
            1 => Some(TabId::Summary),
            2 => Some(TabId::Performance),
            3 => Some(TabId::Warnings),
            4 => Some(TabId::History),
            _ => None,
        }
    }

    pub fn index(&self) -> usize {
        *self as usize
    }

    pub fn next(&self) -> Self {
        let next_index = (self.index() + 1) % 5;
        Self::from_index(next_index).unwrap()
    }

    pub fn prev(&self) -> Self {
        let prev_index = if self.index() == 0 { 4 } else { self.index() - 1 };
        Self::from_index(prev_index).unwrap()
    }
}
