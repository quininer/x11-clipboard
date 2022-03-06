use std::cmp;
use std::sync::Arc;
use std::sync::mpsc::{ Receiver, TryRecvError };
use std::collections::HashMap;
use xcb::{ self, x };
use ::{ INCR_CHUNK_SIZE, Context, SetMap };

macro_rules! try_continue {
    ( $expr:expr ) => {
        match $expr {
            Some(val) => val,
            None => continue
        }
    };
}

struct IncrState {
    selection: x::Atom,
    requestor: x::Window,
    property: x::Atom,
    pos: usize
}

pub fn run(context: &Arc<Context>, setmap: &SetMap, max_length: usize, receiver: &Receiver<x::Atom>) {
    let mut incr_map = HashMap::<x::Atom, x::Atom>::new();
    let mut state_map = HashMap::<x::Atom, IncrState>::new();


    while let Ok(event) = context.connection.wait_for_event() {
        loop {
            match receiver.try_recv() {
                Ok(selection) => if let Some(property) = incr_map.remove(&selection) {
                    state_map.remove(&property);
                },
                Err(TryRecvError::Empty) => break(),
                Err(TryRecvError::Disconnected) => if state_map.is_empty() {
                    return
                }
            }
        }

        match event {
            xcb::Event::X(x::Event::SelectionRequest(event)) => {
                let read_map = try_continue!(setmap.read().ok());
                let &(target, ref value) = try_continue!(read_map.get(&event.selection()));

                if event.target() == context.atoms.targets {
                    context.connection.send_request(&x::ChangeProperty {
                        mode: x::PropMode::Replace,
                        window: event.requestor(),
                        property: event.property(),
                        r#type: x::ATOM_ATOM,
                        data: &[context.atoms.targets, target],
                    });
                } else if value.len() < max_length - 24 {
                    context.connection.send_request(&x::ChangeProperty {
                        mode: x::PropMode::Replace,
                        window: event.requestor(),
                        property: event.property(),
                        r#type: target,
                        data: &value,
                    });
                } else {
                    context.connection.send_request(&x::ChangeWindowAttributes {
                        window: event.requestor(),
                        value_list: &[x::Cw::EventMask(x::EventMask::PROPERTY_CHANGE)],
                    });
                    context.connection.send_request(&x::ChangeProperty {
                        mode: x::PropMode::Replace,
                        window: event.requestor(),
                        property: event.property(),
                        r#type: context.atoms.incr,
                        data: &[0u32; 0],
                    });

                    incr_map.insert(event.selection(), event.property());
                    state_map.insert(
                        event.property(),
                        IncrState {
                            selection: event.selection(),
                            requestor: event.requestor(),
                            property: event.property(),
                            pos: 0
                        }
                    );
                }

                context.connection.send_request(&x::SendEvent {
                    propagate: false,
                    destination: x::SendEventDest::Window(event.requestor()),
                    event_mask: x::EventMask::empty(),
                    event: &x::SelectionNotifyEvent::new(
                        event.time(), event.requestor(), event.selection(), event.target(), event.property()
                    )
                });
                let _ = context.connection.flush();
            },
            xcb::Event::X(x::Event::PropertyNotify(event)) => {
                if event.state() != x::Property::Delete { continue };

                let is_end = {
                    let state = try_continue!(state_map.get_mut(&event.atom()));
                    let read_setmap = try_continue!(setmap.read().ok());
                    let &(target, ref value) = try_continue!(read_setmap.get(&state.selection));

                    let len = cmp::min(INCR_CHUNK_SIZE, value.len() - state.pos);
                    context.connection.send_request(&x::ChangeProperty {
                        mode: x::PropMode::Replace,
                        window: state.requestor,
                        property: state.property,
                        r#type: target,
                        data: &value[state.pos..][..len],
                    });

                    state.pos += len;
                    len == 0
                };

                if is_end {
                    state_map.remove(&event.atom());
                }
                let _ = context.connection.flush();
            },
            xcb::Event::X(x::Event::SelectionClear(event)) => {
                if let Some(property) = incr_map.remove(&event.selection()) {
                    state_map.remove(&property);
                }
                if let Ok(mut write_setmap) = setmap.write() {
                    write_setmap.remove(&event.selection());
                }
            },
            _ => ()
        }
    }
}
