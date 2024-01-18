use error::Error;
use nix::poll::{PollFd, PollFlags};
use nix::sys::eventfd::EfdFlags;
use std::cmp;
use std::collections::HashMap;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, ChangeWindowAttributesAux, ConnectionExt, PropMode, Property, SelectionNotifyEvent,
    Window, SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::Event;
use {AtomEnum, EventMask};
use {Context, SetMap, INCR_CHUNK_SIZE};

macro_rules! try_continue {
    ( $expr:expr ) => {
        match $expr {
            Some(val) => val,
            None => continue,
        }
    };
}

struct IncrState {
    selection: Atom,
    requestor: Window,
    property: Atom,
    pos: usize,
}

#[derive(Clone)]
pub(crate) struct EventFd(pub(crate) Arc<OwnedFd>);

pub(crate) fn create_eventfd() -> Result<EventFd, Error> {
    let raw = nix::sys::eventfd::eventfd(0, EfdFlags::EFD_CLOEXEC).map_err(Error::EventFdCreate)?;
    Ok(EventFd(Arc::new(raw)))
}

pub fn run(
    context: Arc<Context>,
    setmap: SetMap,
    max_length: usize,
    receiver: Receiver<Atom>,
    evt_fd: EventFd,
) {
    let mut incr_map = HashMap::<Atom, Atom>::new();
    let mut state_map = HashMap::<Atom, IncrState>::new();

    let stream_fd = context.connection.stream().as_fd();
    let borrowed_fd = evt_fd.0.as_fd();
    // Poll both stream and eventfd for new Read-ready events
    let mut poll_fds = [
        PollFd::new(&stream_fd, PollFlags::POLLIN),
        PollFd::new(&borrowed_fd, PollFlags::POLLIN),
    ];
    while nix::poll::poll(&mut poll_fds, -1).is_ok() {
        if let Some(PollFlags::POLLIN) = poll_fds[1].revents() {
            // kill-signal
            return;
        }
        loop {
            let evt = if let Ok(evt) = context.connection.poll_for_event() {
                evt
            } else {
                // Exit on error
                return;
            };
            let event = if let Some(event) = evt {
                event
            } else {
                // No event on POLLIN happens, fd being readable doesn't mean theres a complete event ready to read.
                // Poll again.
                break;
            };
            loop {
                match receiver.try_recv() {
                    Ok(selection) => {
                        if let Some(property) = incr_map.remove(&selection) {
                            state_map.remove(&property);
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        if state_map.is_empty() {
                            return;
                        }
                    }
                }
            }

            match event {
                Event::SelectionRequest(event) => {
                    let read_map = try_continue!(setmap.read().ok());
                    let &(target, ref value) = try_continue!(read_map.get(&event.selection));

                    if event.target == context.atoms.targets {
                        let _ = x11rb::wrapper::ConnectionExt::change_property32(
                            &context.connection,
                            PropMode::REPLACE,
                            event.requestor,
                            event.property,
                            Atom::from(AtomEnum::ATOM),
                            &[context.atoms.targets, target],
                        );
                    } else if value.len() < max_length - 24 {
                        let _ = x11rb::wrapper::ConnectionExt::change_property8(
                            &context.connection,
                            PropMode::REPLACE,
                            event.requestor,
                            event.property,
                            target,
                            value,
                        );
                    } else {
                        let _ = context.connection.change_window_attributes(
                            event.requestor,
                            &ChangeWindowAttributesAux::new()
                                .event_mask(EventMask::PROPERTY_CHANGE),
                        );
                        let _ = x11rb::wrapper::ConnectionExt::change_property32(
                            &context.connection,
                            PropMode::REPLACE,
                            event.requestor,
                            event.property,
                            context.atoms.incr,
                            &[0u32; 0],
                        );
                        incr_map.insert(event.selection, event.property);
                        state_map.insert(
                            event.property,
                            IncrState {
                                selection: event.selection,
                                requestor: event.requestor,
                                property: event.property,
                                pos: 0,
                            },
                        );
                    }
                    let _ = context.connection.send_event(
                        false,
                        event.requestor,
                        EventMask::default(),
                        SelectionNotifyEvent {
                            response_type: SELECTION_NOTIFY_EVENT,
                            sequence: 0,
                            time: event.time,
                            requestor: event.requestor,
                            selection: event.selection,
                            target: event.target,
                            property: event.property,
                        },
                    );
                    let _ = context.connection.flush();
                }
                Event::PropertyNotify(event) => {
                    if event.state != Property::DELETE {
                        continue;
                    };

                    let is_end = {
                        let state = try_continue!(state_map.get_mut(&event.atom));
                        let read_setmap = try_continue!(setmap.read().ok());
                        let &(target, ref value) = try_continue!(read_setmap.get(&state.selection));

                        let len = cmp::min(INCR_CHUNK_SIZE, value.len() - state.pos);
                        let _ = x11rb::wrapper::ConnectionExt::change_property8(
                            &context.connection,
                            PropMode::REPLACE,
                            state.requestor,
                            state.property,
                            target,
                            &value[state.pos..][..len],
                        );
                        state.pos += len;
                        len == 0
                    };

                    if is_end {
                        state_map.remove(&event.atom);
                    }
                    let _ = context.connection.flush();
                }
                Event::SelectionClear(event) => {
                    if let Some(property) = incr_map.remove(&event.selection) {
                        state_map.remove(&property);
                    }
                    if let Ok(mut write_setmap) = setmap.write() {
                        write_setmap.remove(&event.selection);
                    }
                }
                _ => (),
            }
        }
    }
}
