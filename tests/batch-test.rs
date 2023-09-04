extern crate x11_clipboard;

use std::time::Instant;
use x11_clipboard::Clipboard;

#[test]
fn set_batch() {
    let data = format!("{:?}", Instant::now());
    let data_in_html = format!("<html><body>{}</body></html>", data);

    let clipboard = Clipboard::new().unwrap();

    let atom_clipboard = clipboard.setter.atoms.clipboard;
    let atom_utf8string = clipboard.setter.atoms.utf8_string;
    let atom_property = clipboard.setter.atoms.property;

    let atom_html = clipboard
        .setter
        .get_atom("text/html;charset=utf-8")
        .unwrap();

    let batch = vec![
        (atom_utf8string, data.as_bytes()),
        (atom_html, data_in_html.as_bytes()),
    ];

    clipboard.store_batch(atom_clipboard, batch).unwrap();

    let text = clipboard
        .load(atom_clipboard, atom_utf8string, atom_property, None)
        .unwrap();
    assert_eq!(text, data.as_bytes());
    let html = clipboard
        .load(atom_clipboard, atom_html, atom_property, None)
        .unwrap();
    assert_eq!(html, data_in_html.as_bytes());
}
