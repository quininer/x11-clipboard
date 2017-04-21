#[macro_use] extern crate error_chain;
extern crate xcb;

#[macro_use] pub mod error;
mod run;

use std::thread;
use std::sync::mpsc::{ Sender, channel };
use xcb::{ Connection, Window, Atom };


#[derive(Clone, Debug)]
pub struct Atoms {
    pub primary: Atom,
    pub clipboard: Atom,
    pub property: Atom,
    pub targets: Atom,
    pub string: Atom,
    pub utf8_string: Atom,
    pub incr: Atom
}

pub type Data = (Vec<u8>, Atom, Atom);

pub struct Clipboard {
    pub getter: InnerContext,
    setter: Sender<Data>
}

pub struct InnerContext {
    pub connection: Connection,
    pub window: Window,
    pub atoms: Atoms
}

impl InnerContext {
    pub fn new() -> error::Result<Self> {
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
                    .map_err(|err| err!(XcbGeneric, err))?
                    .atom()
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

        Ok(InnerContext { connection, window, atoms })
    }
}


impl Clipboard {
    pub fn new() -> error::Result<Self> {
        let getter = InnerContext::new()?;
        let setter = InnerContext::new()?;

        let (sender, receiver) = channel();
        let max_length = setter.connection.get_maximum_request_length() as usize * 4;

        thread::spawn(move || run::run(setter, max_length, receiver));

        Ok(Clipboard { getter, setter: sender })
    }

    pub fn load(&self, selection: Atom, target: Atom, property: Atom) -> error::Result<Vec<u8>> {
        let mut buff = Vec::new();
        let mut is_incr = false;

        xcb::convert_selection(
            &self.getter.connection, self.getter.window,
            selection, target, property,
            xcb::CURRENT_TIME
                // FIXME ^
                // Clients should not use CurrentTime for the time argument of a ConvertSelection request.
                // Instead, they should use the timestamp of the event that caused the request to be made.
        );
        self.getter.connection.flush();

        while let Some(event) = self.getter.connection.wait_for_event() {
            match event.response_type() & !0x80 {
                xcb::SELECTION_NOTIFY => {
                    let event = xcb::cast_event::<xcb::SelectionNotifyEvent>(&event);

                    if event.selection() != selection || event.property() != property {
                        continue
                    }

                    let reply = xcb::get_property(
                        &self.getter.connection, false, self.getter.window,
                        event.property(), xcb::ATOM_ANY, buff.len() as u32, ::std::u32::MAX // FIXME reasonable buffer size
                    )
                        .get_reply()
                        .map_err(|err| err!(XcbGeneric, err))?;

                    if reply.type_() == self.getter.atoms.incr {
                        buff.reserve(reply.value::<i32>()[0] as usize);
                        xcb::delete_property(&self.getter.connection, self.getter.window, property);
                        self.getter.connection.flush();
                        is_incr = true;
                        continue
                    }

                    if reply.type_() != target {
                        let name = xcb::get_atom_name(&self.getter.connection, reply.type_())
                            .get_reply()
                            .map(|reply| reply.name().to_string())
                            .unwrap_or(format!("Unknown({})", reply.type_()));
                        return Err(err!(NotSupportType, name));
                    }

                    buff.extend_from_slice(reply.value());
                    break
                },
                xcb::PROPERTY_NOTIFY if is_incr => {
                    let event = xcb::cast_event::<xcb::PropertyNotifyEvent>(&event);

                    if event.state() != xcb::PROPERTY_NEW_VALUE as u8 {
                        continue
                    }

                    let length = xcb::get_property(
                        &self.getter.connection, false, self.getter.window,
                        property, xcb::ATOM_ANY, 0, 0
                    )
                        .get_reply()
                        .map(|reply| reply.bytes_after())
                        .map_err(|err| err!(XcbGeneric, err))?;

                    let reply = xcb::get_property(
                        &self.getter.connection, true, self.getter.window,
                        property, xcb::ATOM_ANY, 0, length
                    )
                        .get_reply()
                        .map_err(|err| err!(XcbGeneric, err))?;

                    if reply.type_() != target {
                        continue
                    }

                    buff.extend_from_slice(reply.value());

                    if reply.value_len() == 0 {
                        break
                    }
                },
                _ => ()
            }
        }

        xcb::delete_property(&self.getter.connection, self.getter.window, property);
        self.getter.connection.flush();
        Ok(buff)
    }

    pub fn store(&self, selection: Atom, target: Atom, value: &[u8]) -> error::Result<()> {
        self.setter.send((value.into(), selection, target))
            .map_err(Into::into)
    }
}
