use wm::manager::{Manager, Event};
use wm::keyboard::{Key, Keyboard, KeyModifier, KeyPress};
use wm::keysym;

use wm::process;

fn handle_key(mgr: &mut Manager, key: Key) {
    match key {
        Key { keysym: keysym::i, .. } => process::execvp(&["firefox"]),
        Key { keysym: keysym::Return, .. } => process::execvp(&["st"]),
        Key { keysym: keysym::q, .. } => { std::process::exit(0) },
        _ => {
            panic!("unknown key??? {:?}", key);
        },
    }
}

fn handle(mgr: &mut Manager, e: Event) {
    match dbg!(e) {
        Event::Binding { key } => handle_key(mgr, key),
        _ => {},
    }
}

fn main() {
    let mut mgr = Manager::connect(None, None)
        .expect("failed to connect");

    mgr.keyboard.bind(Key {
        mask: KeyModifier::MOD4,
        keysym: keysym::i,
        press: KeyPress::Press,
    }).expect("bind key failed");

    mgr.keyboard.bind(Key {
        mask: KeyModifier::MOD4,
        keysym: keysym::Return,
        press: KeyPress::Press,
    }).expect("bind key failed");

    mgr.keyboard.bind(Key {
        mask: KeyModifier::MOD4,
        keysym: keysym::q,
        press: KeyPress::Press,
    }).expect("bind key failed");

    loop {
        match mgr.next() {
            Ok(Some(x)) => handle(&mut mgr, x),
            Err(e) => eprintln!("error: {}", e),
            _ => {},
        }
    }
}
