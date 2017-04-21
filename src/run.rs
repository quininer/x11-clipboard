use std::sync::Arc;
use std::sync::mpsc::Receiver;
use xcb::{ self, Connection, Atom };
use ::{ InnerContext, Data };


#[inline]
pub fn run(context: InnerContext, max_length: usize, receiver: Receiver<Data>) {
    for (data, selection, target) in receiver.iter() {
        xcb::set_selection_owner(
            &context.connection,
            context.window, selection,
            xcb::CURRENT_TIME
        );
        context.connection.flush();
        let use_incr = data.len() > max_length - 24;

        if let Some(owner) = xcb::get_selection_owner(&context.connection, selection)
            .get_reply()
            .into_iter()
            .map(|reply| reply.owner())
            .filter(|&owner| owner == context.window)
            .next()
        {
            owner
        } else {
            continue
        };

        while let Some(event) = context.connection.wait_for_event() {
            match event.response_type() & !0x80 {
                xcb::SELECTION_REQUEST => {
                    let event = xcb::cast_event::<xcb::SelectionRequestEvent>(&event);

                    if event.selection() != selection { continue };

                    if event.target() == context.atoms.targets {
                        xcb::change_property(
                            &context.connection, xcb::PROP_MODE_REPLACE as u8,
                            event.requestor(), event.property(), xcb::ATOM_ATOM, 32,
                            &[context.atoms.targets, target]
                        );
                    } else if event.target() == target {
                        if !use_incr {
                            xcb::change_property(
                                &context.connection, xcb::PROP_MODE_REPLACE as u8,
                                event.requestor(), event.property(), target, 8,
                                &data
                            );
                        } else {
                            unimplemented!()
                        }
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
                xcb::PROPERTY_NOTIFY => { /* TODO */ },
                xcb::SELECTION_CLEAR => unimplemented!(),
                _ => ()
            }
        }
    }
}
