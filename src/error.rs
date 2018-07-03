use xcb::Atom;
use xcb::base::{ConnError, GenericError};
use std::fmt;
use std::sync::mpsc::SendError;
use std::error::Error as StdError;

#[must_use]
#[derive(Debug)]
pub enum Error {
    Set(SendError<Atom>),
    XcbConn(ConnError),
    XcbGeneric(GenericError),
    Lock,
    Timeout,
    Owner
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match self {
            Set(e) => write!(f, "{}: {:?}", self.description(), e),
            XcbConn(e) => write!(f, "{}: {:?}", self.description(), e),
            XcbGeneric(e) => write!(f, "{}: {:?}", self.description(), e),
            Lock | Timeout | Owner => write!(f, "{}", self.description()),
        }
    }
}

impl StdError for Error {

    fn description(&self) -> &str {
        use self::Error::*;
        match self {
            Set(_) => "XCB - couldn't set atom",
            XcbConn(_) => "XCB connection error",
            XcbGeneric(_) => "XCB generic error",
            Lock => "XCB: Lock is poisoned",
            Timeout => "Selection timed out",
            Owner => "Failed to set new owner of XCB selection",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use self::Error::*;
        match self {
            Set(e) => e.cause(),
            XcbConn(e) => e.cause(),
            XcbGeneric(e) => e.cause(),
            Lock | Timeout | Owner => None,
        }
    }
}

macro_rules! define_from {
    ( $item:ident from $err:ty ) => {
        impl From<$err> for Error {
            fn from(err: $err) -> Error {
                Error::$item(err)
            }
        }
    }
}

define_from!(Set from SendError<Atom>);
define_from!(XcbConn from ConnError);
define_from!(XcbGeneric from GenericError);
