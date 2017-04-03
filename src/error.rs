error_chain!{
    foreign_links {
        Io(::std::io::Error);
        Utf8(::std::string::FromUtf8Error);
    }

    errors {
        XcbConn(err: ::xcb::base::ConnError) {
            description("xcb connection error")
            display("xcb connection error: {:?}", err)
        }
        XcbGeneric(err: ::xcb::base::GenericError) {
            description("xcb generic error")
            display("xcb generic error code: {}", err.error_code())
        }
        NotSupportType(err: String) {
            description("not support reply type.")
            display("not support reply type: {}", err)
        }
    }
}

macro_rules! err {
    ( $kind:ident ) => {
        $crate::error::Error::from($crate::error::ErrorKind::$kind)
    };
    ( $kind:ident, $err:expr ) => {
        $crate::error::Error::from($crate::error::ErrorKind::$kind($err))
    };
}
