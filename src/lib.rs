#[macro_use] extern crate error_chain;
extern crate xcb;
extern crate xcb_util;

#[macro_use] pub mod error;

use std::thread;
use xcb::{ Connection, Window, Atom,  };
use xcb_util::icccm;


pub struct Atoms {
    pub primary: Atom,
    pub clipboard: Atom,
    pub property: Atom,
    pub targets: Atom,
    pub string: Atom,
    pub utf8_string: Atom
}

pub struct Clipboard {
    pub connection: Connection,
    pub window: Window,
    pub atoms: Atoms,
}

impl Clipboard {
    pub fn new<'a, D>(displayname: D) -> error::Result<Self>
        where D: Into<Option<&'a str>>
    {
        let (connection, screen) = Connection::connect(displayname.into())
            .map_err(|err| err!(XcbConn, err))?;
        let window = connection.generate_id();

        {
            let screen = connection.get_setup().roots().nth(screen as usize)
                .ok_or(err!(XcbConn, ::xcb::ConnError::ClosedInvalidScreen))?;
            xcb::create_window(
                &connection,
                xcb::COPY_FROM_PARENT as u8,
                window, screen.root(),
                0, 0, 1, 1,
                0,
                xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
                screen.root_visual(),
                &[(
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_STRUCTURE_NOTIFY | xcb::EVENT_MASK_PROPERTY_CHANGE
                )]
            );
            connection.flush();
        }

        Self::from_connection(connection, window)
    }

    pub fn from_connection(connection: Connection, window: Window) -> error::Result<Self> {
        macro_rules! intern_atom {
            ( $name:expr ) => {
                xcb::intern_atom(&connection, false, $name)
                    .get_reply()
                    .map_err(|err| err!(XcbGeneric, err))?.atom()
            }
        }

        let atoms = Atoms {
            primary: xcb::ATOM_PRIMARY,
            clipboard: intern_atom!("CLIPBOARD"),
            property: intern_atom!("THIS_CLIPBOARD_OUT"),
            targets: intern_atom!("TARGETS"),
            string: xcb::ATOM_STRING,
            utf8_string: intern_atom!("UTF8_STRING")
        };

        Ok(Clipboard { connection, window, atoms })
    }

    fn load(&self, selection: Atom, target: Atom, property: Atom) -> error::Result<()> {
        xcb::convert_selection(
            &self.connection, self.window,
            selection, target, property,
            xcb::CURRENT_TIME
        );
        self.connection.flush();

        unimplemented!()
    }

    fn store(&self, selection: Atom, target: Atom, property: Atom) -> error::Result<()> {
        unimplemented!()
    }
}
