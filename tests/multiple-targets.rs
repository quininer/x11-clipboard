extern crate x11_clipboard;

use std::collections::HashMap;
use std::iter::FromIterator;
use std::time::{ Instant };
use x11_clipboard::Clipboard;
use x11_clipboard::xcb::{Atom, intern_atom};


#[test]
fn test_multiple_targets() {
    let utf8_data = format!("{:?}", Instant::now());
    let alt_data = "foobar";
    let clipboard = Clipboard::new().unwrap();

    let atom_clipboard = clipboard.setter.atoms.clipboard;
    let atom_utf8string = clipboard.setter.atoms.utf8_string;
    let atom_property = clipboard.setter.atoms.property;

    let atom_foobar = intern_atom(&clipboard.setter.connection, false, "foobar").get_reply().unwrap().atom();

    let data: HashMap<Atom, Vec<u8>> = HashMap::from_iter([
        (atom_utf8string, utf8_data.as_bytes().to_owned()),
        (atom_foobar, alt_data.as_bytes().to_owned())
    ]);

    clipboard.store_many(atom_clipboard, data).unwrap();

    let stored_utf8_data = clipboard.load(atom_clipboard, atom_utf8string, atom_property, None).unwrap();
    assert_eq!(utf8_data.as_bytes(), &stored_utf8_data);

    let stored_foobar_data = clipboard.load(atom_clipboard, atom_foobar, atom_property, None).unwrap();
    assert_eq!(alt_data.as_bytes(), &stored_foobar_data);
}
