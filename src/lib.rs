#[macro_use] extern crate error_chain;
extern crate xcb;
extern crate xcb_util;

#[macro_use] pub mod error;

use std::thread;
use std::sync::Arc;
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
    connection: Connection,
    window: Window,
    atoms: Atoms
}

impl Clipboard {
    pub fn new() -> error::Result<Arc<Self>> {
        let (connection, screen) = Connection::connect(None)
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

        let clipboard = Arc::new(Clipboard { connection, window, atoms });

        run(clipboard.clone());

        Ok(clipboard)
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

    fn store(&self, selection: Atom) -> error::Result<()> {
        unimplemented!()
    }
}

fn run(clipboard: Arc<Clipboard>) {
    thread::spawn(move || {
        while let Some(event) = clipboard.connection.wait_for_event() {
            match event.response_type() {
                xcb::DESTROY_WINDOW => {
                    let event = xcb::cast_event::<xcb::DestroyNotifyEvent>(&event);
                    if event.window() == clipboard.window {
                        break
                    }
                },
                xcb::SELECTION_CLEAR => unimplemented!(),
                xcb::SELECTION_NOTIFY => unimplemented!(),
                xcb::SELECTION_REQUEST => unimplemented!(),
                xcb::PROPERTY_NOTIFY => unimplemented!(),
                _ => ()
            }
        }
    });
}
