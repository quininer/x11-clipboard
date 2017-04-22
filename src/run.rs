use std::cmp;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use ::{ xcb, INCR_CHUNK_SIZE, InnerContext, Data };


#[inline]
pub fn run(context: Arc<InnerContext>, max_length: usize, receiver_data: &Receiver<Data>, receiver_clear: &Receiver<()>) {
    let _ = receiver_clear.recv();

    for (data, selection, target) in receiver_data.iter() {
        let mut incr_state = None;
        let mut pos = 0;

        while let Some(event) = context.connection.wait_for_event() {
            if let Ok(()) = receiver_clear.try_recv() {
                break
            }

            match event.response_type() & !0x80 {
                xcb::SELECTION_REQUEST if !data.is_empty() => {
                    let event = xcb::cast_event::<xcb::SelectionRequestEvent>(&event);
                    if event.selection() != selection { continue };

                    if event.target() == context.atoms.targets {
                        xcb::change_property(
                            &context.connection, xcb::PROP_MODE_REPLACE as u8,
                            event.requestor(), event.property(), xcb::ATOM_ATOM, 32,
                            &[context.atoms.targets, target]
                        );
                    } else if event.target() == target {
                        if data.len() < max_length - 24 {
                            xcb::change_property(
                                &context.connection, xcb::PROP_MODE_REPLACE as u8,
                                event.requestor(), event.property(), target, 8,
                                &data
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
                            incr_state = Some((event.requestor(), event.property()));
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
                xcb::PROPERTY_NOTIFY => if let Some((owner, property)) = incr_state {
                    let event = xcb::cast_event::<xcb::PropertyNotifyEvent>(&event);
                    if event.state() != xcb::PROPERTY_DELETE as u8 { continue };

                    let len = cmp::min(INCR_CHUNK_SIZE, data.len() - pos);
                    xcb::change_property(
                        &context.connection, xcb::PROP_MODE_REPLACE as u8,
                        owner, property, target, 8,
                        &data[pos..(pos + len)]
                    );
                    if len == 0 {
                        pos = 0;
                        incr_state = None;
                    } else {
                        pos += len;
                    }
                    context.connection.flush();
                },
                xcb::SELECTION_CLEAR => break,
                _ => ()
            }
        }
    }
}
