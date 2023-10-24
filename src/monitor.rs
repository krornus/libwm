use std::collections::HashMap;

use xcb::x;
use xcb::randr::{self, Output};
use slab::Slab;

use crate::rect::Rect;
use crate::error::Error;
use crate::manager::{Connection, Event};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct MonitorId {
    id: usize,
}

pub struct Monitor {
    pub root: x::Window,
    pub name: String,
    pub size: Rect,
}

pub struct Monitors {
    conn: Connection,
    monitors: Slab<Monitor>,
}

impl Monitors {
    pub fn new(conn: Connection) -> Result<Self, Error> {
        let root = conn.root();

        conn.send_and_check_request(&randr::SelectInput {
            window: root,
            enable: randr::NotifyMask::SCREEN_CHANGE
                | randr::NotifyMask::OUTPUT_CHANGE
                | randr::NotifyMask::CRTC_CHANGE
                | randr::NotifyMask::OUTPUT_PROPERTY,
        })?;

        let mut mon = Monitors {
            conn: conn,
            monitors: Slab::new(),
        };

        mon.update()?;

        Ok(mon)
    }

    /// Refresh monitor state for all roots
    pub fn update(&mut self) -> Result<(), Error> {
        self.update_root(self.conn.root())
    }
}

impl Monitors {
    /// Add a monitor to the slab, producing either a MonitorSize or
    /// a MonitorConnect event
    fn add(&mut self, root: x::Window, mon: Monitor) -> MonitorId {
        let id;
        let name = &mon.name;

        for (k, v) in self.monitors.iter_mut() {
            if &v.root == &root && &v.name == name {

                id = MonitorId { id: k };

                if &v.size != &mon.size {
                    self.conn.produce(Event::MonitorTransform {
                        monitor: id,
                        x: mon.size.x,
                        y: mon.size.y,
                        width: mon.size.w,
                        height: mon.size.h,
                    });
                }

                *v = mon;
                return id;
            }
        }

        id = MonitorId { id: self.monitors.vacant_key() };

        self.conn.produce(Event::MonitorConnect {
            monitor: id,
            x: mon.size.x,
            y: mon.size.y,
            width: mon.size.w,
            height: mon.size.h,
        });

        self.monitors.insert(mon);

        id
    }

    /// Get HashMap of xcb Outputs and their associated connection state
    fn root_outputs(&self, root: x::Window) -> Result<HashMap<Output, randr::Connection>, Error> {
        let cookie = self.conn.send_request(&randr::GetScreenResourcesCurrent {
            window: root
        });

        let reply = self.conn.wait_for_reply(cookie)?;

        let mut outputs = HashMap::new();

        for output in reply.outputs() {
            let cookie = self.conn.send_request(&randr::GetOutputInfo {
                output: *output,
                config_timestamp: reply.timestamp(),
            });

            let reply = self.conn.wait_for_reply(cookie)?;
            outputs.insert(*output, reply.connection());
        }

        Ok(outputs)
    }

    /// Refresh monitor state for a given root window
    fn update_root(&mut self, root: x::Window) -> Result<(), Error> {
        self.monitors.shrink_to_fit();

        let cookie = self.conn.send_request(&randr::GetMonitors {
            window: root,
            get_active: true,
        });

        let outputs = self.root_outputs(root)?;

        let reply = self.conn.wait_for_reply(cookie)?;

        let mut disconnected = vec![false; self.monitors.capacity()];
        for (k, _) in self.monitors.iter() {
            disconnected[k] = true;
        }

        for (_, info) in reply.monitors().enumerate() {
            let mut connected = false;

            for output in info.outputs() {
                match outputs.get(output) {
                    Some(randr::Connection::Connected) => {
                        connected = true;
                        break;
                    },
                    _ => { }
                }
            }

            if !connected {
                continue;
            }

            let cookie = self.conn.send_request(&x::GetAtomName { atom: info.name() });

            let size = Rect::new(info.x(), info.y(), info.width(), info.height());

            let reply = self.conn.wait_for_reply(cookie)?;
            let name = String::from(reply.name().to_utf8());

            let id = self.add(root, Monitor {
                root: root,
                name: name,
                size: size,
            });

            if id.id < disconnected.len() {
                disconnected[id.id] = false;
            }

            if info.primary() {
                self.conn.produce(Event::MonitorPrimary {
                    monitor: id,
                });
            }
        }

        for (i, dc) in disconnected.into_iter().enumerate() {
            if dc {
                self.monitors.remove(i);

                self.conn.produce(Event::MonitorDisconnect {
                    monitor: MonitorId { id: i },
                });
            }
        }

        Ok(())
    }
}

impl std::ops::Index<MonitorId> for Monitors {
    type Output = Monitor;

    fn index(&self, id: MonitorId) -> &Self::Output {
        &self.monitors[id.id]
    }
}

impl std::ops::IndexMut<MonitorId> for Monitors {
    fn index_mut(&mut self, id: MonitorId) -> &mut Self::Output {
        &mut self.monitors[id.id]
    }
}
