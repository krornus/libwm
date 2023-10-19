use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("another window manager is already running")]
    AlreadyRunning,
    #[error("failed to connect to X11 server")]
    ConnectionError(#[from] xcb::ConnError),
    #[error("xcb error")]
    XCBError(#[from] xcb::Error),
    #[error("xcb protocol error")]
    ProtocolError(#[from] xcb::ProtocolError),
}
