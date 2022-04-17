pub extern crate xcb;

pub mod error;
mod run;

use std::thread;
use std::time::{ Duration, Instant };
use std::sync::{ Arc, RwLock };
use std::sync::mpsc::{ Sender, channel };
use std::collections::HashMap;
use xcb::{ ConnError, Connection, Extension, Xid };
use xcb::{ x, xfixes };
use error::Error;

pub const INCR_CHUNK_SIZE: usize = 4000;
const POLL_DURATION: u64 = 50;
type SetMap = Arc<RwLock<HashMap<x::Atom, (x::Atom, Vec<u8>)>>>;

#[derive(Clone, Debug)]
pub struct Atoms {
    pub primary: x::Atom,
    pub clipboard: x::Atom,
    pub property: x::Atom,
    pub targets: x::Atom,
    pub string: x::Atom,
    pub utf8_string: x::Atom,
    pub incr: x::Atom,
}

impl Atoms {
    fn intern_all(conn: &xcb::Connection) -> xcb::Result<Atoms> {
        let clipboard = conn.send_request(&x::InternAtom{
            only_if_exists: false,
            name: b"CLIPBOARD",
        });
        let property = conn.send_request(&x::InternAtom{
            only_if_exists: false,
            name: b"THIS_CLIPBOARD_OUT",
        });
        let targets = conn.send_request(&x::InternAtom{
            only_if_exists: false,
            name: b"TARGETS",
        });
        let utf8_string = conn.send_request(&x::InternAtom{
            only_if_exists: false,
            name: b"UTF8_STRING",
        });
        let incr = conn.send_request(&x::InternAtom{
            only_if_exists: false,
            name: b"INCR",
        });
        Ok(Atoms {
            primary: x::ATOM_PRIMARY,
            clipboard: conn.wait_for_reply(clipboard)?.atom(),
            property: conn.wait_for_reply(property)?.atom(),
            targets: conn.wait_for_reply(targets)?.atom(),
            string: x::ATOM_STRING,
            utf8_string: conn.wait_for_reply(utf8_string)?.atom(),
            incr: conn.wait_for_reply(incr)?.atom(),
        })
    }
}

/// X11 Clipboard
pub struct Clipboard {
    pub getter: Context,
    pub setter: Arc<Context>,
    setmap: SetMap,
    send: Sender<x::Atom>
}

pub struct Context {
    pub connection: Connection,
    pub screen: i32,
    pub window: x::Window,
    pub atoms: Atoms
}

#[inline]
fn get_atom(connection: &Connection, name: &str) -> Result<x::Atom, Error> {
    let cookie = connection.send_request(&x::InternAtom {
        only_if_exists: false,
        name: name.as_bytes()
    });
    let reply = connection.wait_for_reply(cookie)?;
    Ok(reply.atom())
}

impl Context {
    pub fn new(displayname: Option<&str>) -> Result<Self, Error> {
        let (connection, screen) = Connection::connect_with_extensions(displayname,  &[Extension::XFixes], &[])?;
        let window = connection.generate_id();

        {
            let screen = connection.get_setup().roots().nth(screen as usize)
                .ok_or(Error::XcbConn(ConnError::ClosedInvalidScreen))?;
            connection.send_and_check_request(&x::CreateWindow {
                depth: x::COPY_FROM_PARENT as u8,
                wid: window,
                parent: screen.root(),
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                border_width: 0,
                class: x::WindowClass::InputOutput,
                visual: screen.root_visual(),
                value_list: &[
                    x::Cw::EventMask(x::EventMask::STRUCTURE_NOTIFY | x::EventMask::PROPERTY_CHANGE)
                ],
            })?;
        }

        let atoms = Atoms::intern_all(&connection)?;

        Ok(Context { connection, screen, window, atoms })
    }

    pub fn get_atom(&self, name: &str) -> Result<x::Atom, Error> {
        get_atom(&self.connection, name)
    }
}


impl Clipboard {
    /// Create Clipboard.
    pub fn new() -> Result<Self, Error> {
        let getter = Context::new(None)?;
        let setter = Arc::new(Context::new(None)?);
        let setter2 = Arc::clone(&setter);
        let setmap = Arc::new(RwLock::new(HashMap::new()));
        let setmap2 = Arc::clone(&setmap);

        let (sender, receiver) = channel();
        let max_length = setter.connection.get_maximum_request_length() as usize * 4;
        thread::spawn(move || run::run(&setter2, &setmap2, max_length, &receiver));

        Ok(Clipboard { getter, setter, setmap, send: sender })
    }

    fn process_event<T>(&self, buff: &mut Vec<u8>, selection: x::Atom, target: x::Atom, property: x::Atom, timeout: T, use_xfixes: bool)
        -> Result<(), Error>
        where T: Into<Option<Duration>>
    {
        let mut is_incr = false;
        let timeout = timeout.into();
        let start_time =
            if timeout.is_some() { Some(Instant::now()) }
            else { None };

        loop {
            if timeout.into_iter()
                .zip(start_time)
                .next()
                .map(|(timeout, time)| (Instant::now() - time) >= timeout)
                .unwrap_or(false)
            {
                return Err(Error::Timeout);
            }

            let event = match use_xfixes {
                true => self.getter.connection.wait_for_event()?,
                false => {
                    match self.getter.connection.poll_for_event()? {
                        Some(event) => event,
                        None => {
                            thread::park_timeout(Duration::from_millis(POLL_DURATION));
                            continue
                        }
                    }
                }
            };

            match event {
                xcb::Event::XFixes(xfixes::Event::SelectionNotify(event)) if use_xfixes => {
                    self.getter.connection.send_and_check_request(&x::ConvertSelection {
                        requestor: self.getter.window,
                        selection,
                        target,
                        property,
                        time: event.timestamp(),
                    })?;
                },
                xcb::Event::X(x::Event::SelectionNotify(event)) => {
                    if event.selection() != selection { continue };

                    // Note that setting the property argument to None indicates that the
                    // conversion requested could not be made.
                    if event.property().is_none() {
                        break;
                    }

                    let reply = self.getter.connection.wait_for_reply(
                        self.getter.connection.send_request(&x::GetProperty {
                            delete: false,
                            window: self.getter.window,
                            property: event.property(),
                            r#type: x::ATOM_NONE,
                            long_offset: buff.len() as u32,
                            long_length: ::std::u32::MAX,
                        })
                    )?;

                    if reply.r#type() == self.getter.atoms.incr {
                        if let Some(&size) = reply.value::<u32>().get(0) {
                            buff.reserve(size as usize);
                        }
                        self.getter.connection.send_and_check_request(&x::DeleteProperty {
                            window: self.getter.window,
                            property
                        })?;
                        is_incr = true;
                        continue
                    } else if reply.r#type() != target {
                        return Err(Error::UnexpectedType(reply.r#type()));
                    }

                    buff.extend_from_slice(reply.value());
                    break
                },
                xcb::Event::X(x::Event::PropertyNotify(event)) if is_incr => {
                    if event.state() != x::Property::NewValue { continue };

                    let cookie = self.getter.connection.send_request(&x::GetProperty {
                        delete: false,
                        window: self.getter.window,
                        property,
                        r#type: x::ATOM_NONE,
                        long_offset: 0,
                        long_length: 0,
                    });
                    let length = self.getter.connection.wait_for_reply(cookie)?.bytes_after();

                    let cookie = self.getter.connection.send_request(&x::GetProperty {
                        delete: true,
                        window: self.getter.window,
                        property,
                        r#type: x::ATOM_NONE,
                        long_offset: 0,
                        long_length: length,
                    });
                    let reply = self.getter.connection.wait_for_reply(cookie)?;
                    if reply.r#type() != target { continue };

                    let value = reply.value();

                    if value.len() != 0 {
                        buff.extend_from_slice(value);
                    } else {
                        break
                    }
                },
                _ => ()
            }
        }
        Ok(())
    }

    /// load value.
    pub fn load<T>(&self, selection: x::Atom, target: x::Atom, property: x::Atom, timeout: T)
        -> Result<Vec<u8>, Error>
        where T: Into<Option<Duration>>
    {
        let mut buff = Vec::new();
        let timeout = timeout.into();

        self.getter.connection.send_and_check_request(&x::ConvertSelection {
            requestor: self.getter.window,
            selection,
            target,
            property,
            time: x::CURRENT_TIME,
                // FIXME ^
                // Clients should not use CurrentTime for the time argument of a ConvertSelection request.
                // Instead, they should use the timestamp of the event that caused the request to be made.
        })?;

        self.process_event(&mut buff, selection, target, property, timeout, false)?;

        self.getter.connection.send_and_check_request(&x::DeleteProperty {
            window: self.getter.window,
            property,
        })?;

        Ok(buff)
    }

    /// wait for a new value and load it
    pub fn load_wait(&self, selection: x::Atom, target: x::Atom, property: x::Atom)
        -> Result<Vec<u8>, Error>
    {
        let mut buff = Vec::new();

        let screen = &self.getter.connection.get_setup().roots()
            .nth(self.getter.screen as usize)
            .ok_or(Error::XcbConn(ConnError::ClosedInvalidScreen))?;

        self.getter.connection.send_request(&xfixes::QueryVersion {
            client_major_version: 5,
            client_minor_version: 0,
        });
        // Clear selection sources...
        self.getter.connection.send_request(&xfixes::SelectSelectionInput {
            window: screen.root(),
            selection: self.getter.atoms.primary,
            event_mask: xfixes::SelectionEventMask::empty(),
        });
        self.getter.connection.send_request(&xfixes::SelectSelectionInput {
            window: screen.root(),
            selection: self.getter.atoms.clipboard,
            event_mask: xfixes::SelectionEventMask::empty(),
        });
        // ...and set the one requested now
        self.getter.connection.send_and_check_request(&xfixes::SelectSelectionInput {
            window: screen.root(),
            selection,
            event_mask: xfixes::SelectionEventMask::SET_SELECTION_OWNER |
                xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE |
                xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY,
        })?;

        self.process_event(&mut buff, selection, target, property, None, true)?;

        self.getter.connection.send_and_check_request(&x::DeleteProperty {
            window: self.getter.window,
            property,
        })?;

        Ok(buff)
    }

    /// store value.
    pub fn store<T: Into<Vec<u8>>>(&self, selection: x::Atom, target: x::Atom, value: T)
        -> Result<(), Error>
    {
        self.send.send(selection)?;
        self.setmap
            .write()
            .map_err(|_| Error::Lock)?
            .insert(selection, (target, value.into()));

        self.setter.connection.send_and_check_request(&x::SetSelectionOwner {
            owner: self.setter.window,
            selection,
            time: x::CURRENT_TIME,
        })?;

        let cookie = self.setter.connection.send_request(&x::GetSelectionOwner {
            selection
        });
        if self.setter.connection.wait_for_reply(cookie)
            .map(|reply| reply.owner() == self.setter.window)
            .unwrap_or(false) {
            Ok(())
        } else {
            Err(Error::Owner)
        }
    }
}
