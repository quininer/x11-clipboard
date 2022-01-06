use std::cmp;
use std::iter::once;
use std::sync::Arc;
use std::sync::mpsc::{ Receiver, TryRecvError };
use std::collections::HashMap;
use xcb::{ self, Atom };
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
    selection: Atom,
    requestor: Atom,
    property: Atom,
    target: Atom,
    pos: usize
}

pub fn run(context: &Arc<Context>, setmap: &SetMap, max_length: usize, receiver: &Receiver<Atom>) {
    let mut incr_map = HashMap::new();
    let mut state_map = HashMap::new();

    while let Some(event) = context.connection.wait_for_event() {
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

        match event.response_type() & !0x80 {
            xcb::SELECTION_REQUEST => {
                let event = unsafe { xcb::cast_event::<xcb::SelectionRequestEvent>(&event) };
                let read_map = try_continue!(setmap.read().ok());
                let target_values_map = try_continue!(read_map.get(&event.selection()));

                if event.target() == context.atoms.targets {
                    let all_targets: Vec<_> = target_values_map.keys().copied().chain(once(context.atoms.targets)).collect();
                    xcb::change_property(
                        &context.connection, xcb::PROP_MODE_REPLACE as u8,
                        event.requestor(), event.property(), xcb::ATOM_ATOM, 32,
                        &all_targets
                    );
                } else if let Some(value) = target_values_map.get(&event.target()) {
                    if value.len() < max_length - 24 {
                        xcb::change_property(
                            &context.connection, xcb::PROP_MODE_REPLACE as u8,
                            event.requestor(), event.property(), event.target(), 8,
                            value
                        );
                    } else {
                        xcb::change_window_attributes(
                            &context.connection, event.requestor(),
                            &[(xcb::CW_EVENT_MASK, xcb::EVENT_MASK_PROPERTY_CHANGE)]
                        );
                        xcb::change_property(
                            &context.connection, xcb::PROP_MODE_REPLACE as u8,
                            event.requestor(), event.property(), context.atoms.incr, 32,
                            &[0u8; 0]
                        );
    
                        incr_map.insert(event.selection(), event.property());
                        state_map.insert(
                            event.property(),
                            IncrState {
                                selection: event.selection(),
                                requestor: event.requestor(),
                                property: event.property(),
                                target: event.target(),
                                pos: 0
                            }
                        );
                    }
                } else {
                    // Unsupported target type. Return "none"
                    xcb::send_event(
                        &context.connection, false, event.requestor(), 0,
                        &xcb::SelectionNotifyEvent::new(
                            event.time(),
                            event.requestor(),
                            event.selection(),
                            event.target(),
                            xcb::ATOM_NONE
                        )
                    );
                    context.connection.flush();
                    continue;
                }

                xcb::send_event(
                    &context.connection, false, event.requestor(), 0,
                    &xcb::SelectionNotifyEvent::new(
                        event.time(),
                        event.requestor(),
                        event.selection(),
                        event.target(),
                        event.property()
                    )
                );
                context.connection.flush();
            },
            xcb::PROPERTY_NOTIFY => {
                let event = unsafe { xcb::cast_event::<xcb::PropertyNotifyEvent>(&event) };
                if event.state() != xcb::PROPERTY_DELETE as u8 { continue };

                let is_end = {
                    let state = try_continue!(state_map.get_mut(&event.atom()));
                    let read_setmap = try_continue!(setmap.read().ok());
                    let target_values_map = try_continue!(read_setmap.get(&state.selection));
                    let value = try_continue!(target_values_map.get(&state.target));

                    let len = cmp::min(INCR_CHUNK_SIZE, value.len() - state.pos);
                    xcb::change_property(
                        &context.connection, xcb::PROP_MODE_REPLACE as u8,
                        state.requestor, state.property, state.target, 8,
                        &value[state.pos..][..len]
                    );

                    state.pos += len;
                    len == 0
                };

                if is_end {
                    state_map.remove(&event.atom());
                }
                context.connection.flush();
            },
            xcb::SELECTION_CLEAR => {
                let event = unsafe { xcb::cast_event::<xcb::SelectionClearEvent>(&event) };
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
