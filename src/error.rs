use xcb::{Atom, ReplyError};
use xcb::base::{ ConnError, GenericError };
use std::fmt;
use std::sync::mpsc::SendError;
use std::error::Error as StdError;

#[must_use]
#[derive(Debug)]
pub enum Error {
    Set(SendError<Atom>),
    XcbConn(ConnError),
    XcbGeneric(GenericError),
    XcbReply(ReplyError),
    Lock,
    Timeout,
    Owner,
    UnexpectedType(Atom),

    #[doc(hidden)]
    __Unknown
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match self {
            Set(e) => write!(f, "XCB - couldn't set atom: {:?}", e),
            XcbConn(e) => write!(f, "XCB connection error: {:?}", e),
            XcbGeneric(e) => write!(f, "XCB generic error: {:?}", e),
            XcbReply(e) => write!(f, "XCB reply error: {:?}", e),
            Lock => write!(f, "XCB: Lock is poisoned"),
            Timeout => write!(f, "Selection timed out"),
            Owner => write!(f, "Failed to set new owner of XCB selection"),
            UnexpectedType(target) => write!(f, "Unexpected Reply type: {}", target),
            __Unknown => unreachable!()
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        use self::Error::*;
        match self {
            Set(e) => Some(e),
            XcbConn(e) => Some(e),
            XcbGeneric(e) => Some(e),
            XcbReply(e) => Some(e),
            Lock | Timeout | Owner | UnexpectedType(_) => None,
            __Unknown => unreachable!()
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
define_from!(XcbReply from ReplyError);
