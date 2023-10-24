use std::sync::mpsc;
use std::mem;

use xcb::x;

use crate::error::Error;
use crate::monitor::{Monitors, MonitorId};
use crate::keyboard::{Keyboard, Key};
use crate::container::{Container, ContainerId};

/// Required xcb extensions
static REQUIRED: &'static [xcb::Extension] = &[xcb::Extension::RandR];
/// Optional xcb extensions
static OPTIONAL: &'static [xcb::Extension] = &[];


#[derive(Debug)]
pub enum Event {
    MonitorConnect { monitor: MonitorId, x: i16, y: i16, width: u16, height: u16 },
    MonitorDisconnect { monitor: MonitorId },
    MonitorPrimary { monitor: MonitorId },
    MonitorTransform { monitor: MonitorId, x: i16, y: i16, width: u16, height: u16 },
    WindowCreate { window: ContainerId, x: i16, y: i16, width: u16, height: u16 },
    WindowResize { window: ContainerId, x: i16, y: i16, width: u16, height: u16 },
    WindowShow { window: ContainerId },
    Binding { key: Key },
}

/// This atrocity is to force Rust to allow us to pass around a cloned
/// xcb::Connection handle without encountering borrow checker type issues. xcb
/// *should* be fully thread safe so its. The only issue is dropping, we can't
/// ever drop a Handle because its just a copy of the main pointer, held by the
/// Manager struct.
struct Handle {
    xcb: mem::ManuallyDrop<xcb::Connection>
}

impl Handle {
    fn new(xcb: &xcb::Connection) -> Self {
        let raw = xcb.get_raw_conn();
        let copy = unsafe {
            xcb::Connection::from_raw_conn_and_extensions(
                raw, REQUIRED, OPTIONAL)
        };

        Handle {
            xcb: mem::ManuallyDrop::new(copy)
        }
    }
}

impl Clone for Handle {
    fn clone(&self) -> Self {
        Handle::new(&self.xcb)
    }
}

/// Sender side of the producer/consumer model for events
pub struct Connection {
    handle: Handle,
    screen: usize,
    root: x::Window,
    events: mpsc::Sender<Event>,
}

impl Clone for Connection {
    fn clone(&self) -> Self {
        Self {
            screen: self.screen,
            root: self.root,
            handle: self.handle.clone(),
            events: self.events.clone(),
        }
    }
}

impl Connection {
    fn new(xcb: &xcb::Connection, screen: usize, sender: &mpsc::Sender<Event>) -> Self {
        let setup = xcb.get_setup();
        let root = setup.roots().nth(screen).unwrap().root();

        Self {
            screen: screen,
            root: root,
            handle: Handle::new(xcb),
            events: sender.clone(),
        }
    }

    #[inline]
    pub fn root(&self) -> x::Window {
        self.root
    }

    #[inline]
    pub fn screen(&self) -> usize {
        self.screen
    }

    pub fn raw<'a>(&'a self) -> &'a xcb::Connection {
        &self.handle.xcb
    }

    pub fn produce(&self, event: Event) {
        /* this should never fail, due to being allocated/deallocated internally */
        self.events.send(event)
            .expect("mpsc::Receiver disconnected!");
    }

    #[inline]
    #[must_use]
    pub fn send_request<R>(&self, req: &R) -> R::Cookie
    where
        R: xcb::Request + std::fmt::Debug,
    {
        self.handle.xcb.send_request(req)
    }

    #[inline]
    pub fn wait_for_reply<C>(&self, cookie: C) -> Result<C::Reply, Error>
    where
        C: xcb::CookieWithReplyChecked,
    {
        Ok(self.handle.xcb.wait_for_reply(cookie)?)
    }

    #[inline]
    #[must_use]
    pub fn send_request_checked<R>(&self, req: &R) -> xcb::VoidCookieChecked
    where
        R: xcb::RequestWithoutReply + std::fmt::Debug,
    {
        self.handle.xcb.send_request_checked(req)
    }

    pub fn send_and_check_request<R>(&self, req: &R) -> xcb::ProtocolResult<()>
    where
        R: xcb::RequestWithoutReply + std::fmt::Debug,
    {
        self.handle.xcb.send_and_check_request(req)
    }

    #[inline]
    pub fn check_request(&self, cookie: xcb::VoidCookieChecked)
        -> xcb::ProtocolResult<()>
    {
        self.handle.xcb.check_request(cookie)
    }
}

pub struct Manager {
    #[allow(dead_code)]
    raw: xcb::Connection, // lifetime only, use conn instead. See Handle comments
    conn: Connection,
    events: mpsc::Receiver<Event>,
    pub monitors: Monitors,
    pub keyboard: Keyboard,
    pub root: Container,
}

impl Manager {
    fn handle(&mut self, event: xcb::Event) -> Result<(), Error> {
        match event {
            xcb::Event::RandR(xcb::randr::Event::ScreenChangeNotify(_)) => {
                self.monitors.update()?;
            }
            xcb::Event::X(xcb::x::Event::KeyPress(ref e)) => {
                self.keyboard.press(e.root(), e.state(), e.detail() as x::Keycode, true);
            }
            xcb::Event::X(xcb::x::Event::KeyRelease(ref e)) => {
                self.keyboard.press(e.root(), e.state(), e.detail() as x::Keycode, false);
            }
            xcb::Event::X(xcb::x::Event::CreateNotify(ref e)) => {
                self.root.create(e);
            }
            xcb::Event::X(xcb::x::Event::ConfigureRequest(ref e)) => {
                self.root.configure(e);
            }
            xcb::Event::X(xcb::x::Event::MapRequest(ref e)) => {
                self.root.map(e);

            }
            _ => {
            }
        }

        Ok(())
    }
}

impl Manager {
    /// Connect the manager to an X server
    pub fn connect(name: Option<&str>, screenopt: Option<usize>) -> Result<Self, Error> {

        let (raw, main) = xcb::Connection::connect_with_extensions(
            name, REQUIRED, OPTIONAL)?;
        let screen = screenopt.unwrap_or(main as usize);
        let (tx, rx) = mpsc::channel();

        let conn = Connection::new(&raw, screen, &tx);

        /* substructure redirect -- the core "window manager" flag.
         * only one process can set this attribute at a time, and it
         * indicates that window structuring events will be redirected
         * through this process. "another window manager is running" style
         * errors are encountered here */
        conn.send_and_check_request(&x::ChangeWindowAttributes {
            window: conn.root,
            value_list: &[xcb::x::Cw::EventMask(
                x::EventMask::STRUCTURE_NOTIFY
                    | x::EventMask::PROPERTY_CHANGE
                    | x::EventMask::SUBSTRUCTURE_NOTIFY
                    | x::EventMask::SUBSTRUCTURE_REDIRECT,
            )],
        }).map_err(|_| Error::AlreadyRunning)?;

        let monitors = Monitors::new(conn.clone())?;
        let keyboard = Keyboard::new(conn.clone())?;
        let container = Container::new(conn.clone())?;

        let mgr = Manager {
            raw: raw,
            conn: conn,
            events: rx,
            monitors: monitors,
            keyboard: keyboard,
            root: container,
        };

        Ok(mgr)
    }


    pub fn next(&mut self) -> Result<Option<Event>, Error> {
        match self.events.try_recv() {
            Ok(event) => {
                return Ok(Some(event))
            },
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("mpsc::Sender disconnected!");
            },
            Err(mpsc::TryRecvError::Empty) => {
            }
        }

        let event = self.conn.handle.xcb.wait_for_event()?;
        self.handle(event)?;

        match self.events.try_recv() {
            Ok(event) => {
                return Ok(Some(event))
            },
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("mpsc::Sender disconnected!");
            },
            Err(mpsc::TryRecvError::Empty) => {
                Ok(None)
            }
        }
    }
}
