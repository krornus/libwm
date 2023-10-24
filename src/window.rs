use xcb::x;

use crate::rect::Rect;
use crate::error::Error;
use crate::manager::Connection;

pub struct Window {
    conn: Connection,
    window: x::Window,
    size: Rect,
    visible: bool,
    managed: bool,
    selectable: bool,
}

impl Window {
    pub fn size(&self) -> Rect {
        self.size
    }

    pub fn window(&self) -> x::Window {
        self.window
    }

    pub fn managed(&self) -> bool {
        self.managed
    }
}

impl Window {
    pub fn new(conn: Connection, window: x::Window, size: Rect, managed: bool, selectable: bool) -> Self {
        Self {
            conn: conn,
            window: window,
            size: size,
            managed: managed,
            visible: false,
            selectable: selectable,
        }
    }

    pub fn show(&mut self) -> Result<(), Error> {
        if !self.visible {
            let cookie = self.conn.send_request_checked(&x::MapWindow {
                window: self.window,
            });

            self.conn.check_request(cookie)?;
        }

        self.visible = true;

        Ok(())
    }

    pub fn hide(&mut self) -> Result<(), Error> {
        if !self.visible {
            let cookie = self.conn.send_request_checked(&x::UnmapWindow {
                window: self.window,
            });

            self.conn.check_request(cookie)?;
        }

        self.visible = false;

        Ok(())
    }

    pub fn resize(&mut self, size: Rect) -> Result<(), Error> {
        if self.size != size {

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

            self.size = size;
        }

        Ok(())
    }

    pub fn focus(&self) -> Result<(), Error> {
        let cookie = self.conn.send_request_checked(&x::SetInputFocus {
            revert_to: x::InputFocus::PointerRoot,
            focus: self.window,
            time: x::CURRENT_TIME,
        });

        self.conn.check_request(cookie)?;

        Ok(())
    }
}
