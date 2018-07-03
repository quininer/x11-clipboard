use std::sync::mpsc::SendError;
use xcb::{Atom, base::{ConnError, GenericError}};
use std::fmt;

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
            Set(atom) => write!(f, "XCB - couldn't set atom: {:?}", atom),
            XcbConn(conn_err) => write!(f, "XCB connection error: {:?}", conn_err),
            XcbGeneric(generic) => write!(f, "XCB generic error: {:?}", generic),
            Lock => write!(f, "XCB: Lock is poisoned"),
            Timeout => write!(f, "Selection timed out"),
            Owner => write!(f, "Failed to set new owner of XCB selection"),
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
