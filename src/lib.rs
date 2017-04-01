#[macro_use] extern crate error_chain;
extern crate xcb;

#[macro_use] pub mod error;

use std::thread;
use xcb::{ Connection, Window, Atom,  };


pub struct Atoms {
    pub primary: Atom,
    pub clipboard: Atom,
    pub property: Atom,
    pub targets: Atom,
    pub string: Atom,
    pub utf8_string: Atom,
    pub incr: Atom
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
            utf8_string: intern_atom!("UTF8_STRING"),
            incr: intern_atom!("INCR")
        };

        Ok(Clipboard { connection, window, atoms })
    }

    pub fn load(&self, selection: Atom, target: Atom, property: Atom) -> error::Result<String> {
        let mut output = String::new();

        xcb::convert_selection(
            &self.connection, self.window,
            selection, target, property,
            xcb::CURRENT_TIME
        );
        self.connection.flush();

        while let Some(event) = self.connection.wait_for_event() {
            match (event.response_type() & !0x80) {
                xcb::SELECTION_NOTIFY => {
                    let event = xcb::cast_event::<xcb::SelectionNotifyEvent>(&event);

                    if event.selection() != selection || event.property() != property {
                        continue
                    }

                    let reply = xcb::get_property(
                        &self.connection,
                        true, self.window,
                        event.property(), xcb::ATOM_ANY,
                        0, ::std::u32::MAX // FIXME reasonable buffer size
                    )
                        .get_reply()
                        .map_err(|err| err!(XcbGeneric, err))?;

                    if reply.type_() == self.atoms.incr {
                        xcb::delete_property(&self.connection, self.window, property);
                        unimplemented!()
                    }

                    output = String::from_utf8(reply.value().into())?;
                    break
                },
                _ => ()
            }
        }

        xcb::delete_property(&self.connection, self.window, property);
        Ok(output)
    }

    fn store(&self, selection: Atom, target: Atom, property: Atom) -> error::Result<()> {
        unimplemented!()
    }
}
