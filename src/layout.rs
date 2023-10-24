use crate::rect::Rect;

#[derive(Debug)]
pub enum Cell {
    Hide,
    Show(Rect),
    Focus(Rect),
}

pub trait Layout {
    fn arrange(&mut self, index: usize, count: usize, focus: bool, scope: Rect) -> Cell;
}

#[derive(Debug, Clone)]
pub struct Monacle { }

impl Monacle {
    pub fn new() -> Self {
        Monacle {}
    }
}

impl Layout for Monacle {
    fn arrange(&mut self, _: usize, _: usize, focus: bool, scope: Rect) -> Cell {
        if focus {
            Cell::Focus(scope)
        } else {
            Cell::Hide
        }
    }
}
