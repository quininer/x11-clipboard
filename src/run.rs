use std::sync::Arc;
use std::sync::mpsc::Receiver;
use ::{ xcb, InnerContext, Data };


#[inline]
pub fn run(context: Arc<InnerContext>, max_length: usize, receiver_data: Receiver<Data>, receiver_clear: Receiver<()>) {
    let _ = receiver_clear.recv();

    for (data, selection, target) in receiver_data.iter() {
        let use_incr = data.len() > max_length - 24;

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
                xcb::SELECTION_CLEAR => break,
                _ => ()
            }
            if let Ok(()) = receiver_clear.try_recv() {
                break
            }
        }
    }
}
