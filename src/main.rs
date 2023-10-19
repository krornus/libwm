use wm::manager::Manager;
use wm::keyboard::{Key, Keyboard, KeyModifier, KeyPress};
use wm::keysym;

fn main() {
    let mut mgr = Manager::connect(None, None)
        .expect("failed to connect");

    mgr.keyboard.bind(Key {
        mask: KeyModifier::MOD4,
        keysym: keysym::i,
        press: KeyPress::Press,
    }).expect("bind key failed");

    loop {
        dbg!(mgr.next());
    }
}
