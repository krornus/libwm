use xcb::x;

use slab::Slab;

use crate::rect::Rect;
use crate::error::Error;
use crate::manager::{Connection, Event};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct WindowId {
    id: usize,
}

pub struct Window {
    conn: Connection,
    window: x::Window,
    size: Option<Rect>,
}

impl Window {
    pub fn new(conn: Connection, window: x::Window) -> Self {
        Self {
            conn: conn,
            window: window,
            size: None,
        }
    }

    pub fn show(&self) -> Result<(), Error> {
        let cookie = self.conn.send_request_checked(&x::MapWindow {
            window: self.window,
        });

        self.conn.check_request(cookie)?;

        Ok(())
    }

    pub fn resize(&mut self, size: Rect) -> Result<(), Error> {
        if self.size != Some(size) {

            let cookie = self.conn.send_request_checked(&x::ConfigureWindow {
                window: self.window,
                value_list: &[
                    x::ConfigWindow::X(size.x as i32),
                    x::ConfigWindow::Y(size.y as i32),
                    x::ConfigWindow::Width(size.w as u32),
                    x::ConfigWindow::Height(size.h as u32),
                ],
            });

            self.conn.check_request(cookie)?;

            self.size = Some(size);
        }

        Ok(())
    }
}

pub struct Windows {
    conn: Connection,
    windows: Slab<Window>,
}

impl Windows {
    pub fn new(conn: Connection) -> Self {
        Windows {
            conn: conn,
            windows: Slab::new(),
        }
    }

    pub fn add(&mut self, window: Window) -> WindowId {
        WindowId {
            id: self.windows.insert(window)
        }
    }

    pub fn get(&self, window: x::Window) -> Option<(WindowId, &Window)> {
        for (id, win) in self.windows.iter() {
            if win.window == window {
                return Some((WindowId { id }, win))
            }
        }

        None
    }

    pub fn create(&mut self, event: &x::CreateNotifyEvent) {
        let win = Window::new(self.conn.clone(), event.window());
        let id = self.add(win);

        self.conn.produce(Event::WindowCreate {
            window: id,
            x: event.x(),
            y: event.y(),
            width: event.width(),
            height: event.height(),
        });
    }

    pub fn configure(&mut self, event: &x::ConfigureRequestEvent) {
        match self.get(event.window()) {
            Some((id, _)) => {
                self.conn.produce(Event::WindowResize {
                    window: id,
                    x: event.x(),
                    y: event.y(),
                    width: event.width(),
                    height: event.height(),
                });
            },
            None => {
                eprintln!("Unknown window configured!");
            }
        }
    }

    pub fn map(&mut self, event: &x::MapRequestEvent) {
        match self.get(event.window()) {
            Some((id, _)) => {
                self.conn.produce(Event::WindowShow {
                    window: id
                });
            },
            None => {
                eprintln!("Unknown window mapped!");
            }
        }
    }
}

impl std::ops::Index<WindowId> for Windows {
    type Output = Window;

    fn index(&self, id: WindowId) -> &Self::Output {
        &self.windows[id.id]
    }
}
