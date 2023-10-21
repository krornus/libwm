use wm::keysym;
use wm::error::Error;
use wm::manager::{Manager, Event};
use wm::keyboard::{Key, KeyModifier, KeyPress};

use wm::process;

fn handle_key(key: Key) {
    match key {
        Key { keysym: keysym::i, .. } => process::execvp(&["firefox"]),
        Key { keysym: keysym::Return, .. } => process::execvp(&["st"]),
        Key { keysym: keysym::q, .. } => { std::process::exit(0) },
        _ => {
            panic!("unknown key??? {:?}", key);
        },
    }
}

fn handle(mgr: &mut Manager, e: Event) -> Result<(), Error> {
    match dbg!(e) {
        Event::Binding { key } => handle_key(key),
        Event::WindowCreate { window: id } => mgr.windows[id].show()?,
        Event::WindowShow { window: id } => mgr.windows[id].show()?,
        _ => {},
    }

    Ok(())
}

fn run(mut mgr: Manager) {
    loop {
        match mgr.next() {
            Ok(Some(x)) => handle(&mut mgr, x).expect("failed to handle event"),
            Err(e) => eprintln!("error: {}", e),
            _ => {},
        }
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

    run(mgr)
}
