use std::collections::HashMap;

use xcb::x::{self, Keycode, Keysym};
use bitflags::bitflags;

use crate::keysym;
use crate::error::Error;
use crate::manager::{Connection, Event};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct KeyModifier: u32 {
        const SHIFT = 0x00000001;
        const LOCK = 0x00000002;
        const CONTROL = 0x00000004;
        const MOD1 = 0x00000008;
        const MOD2 = 0x00000010;
        const MOD3 = 0x00000020;
        const MOD4 = 0x00000040;
        const MOD5 = 0x00000080;
        const ANY = 0x00008000;
    }
}

struct KeyMap {
    min: u32,
    keymap: x::GetKeyboardMappingReply,
    modmap: x::GetModifierMappingReply,
}

struct KeycodeIterator<'a> {
    min: usize,
    per: usize,
    index: usize,
    target: Keysym,
    keysyms: &'a [Keysym],
}

impl<'a> Iterator for KeycodeIterator<'a> {
    type Item = Keycode;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.keysyms.len() {
            let i = self.index;

            /* next keysym in keycode */
            self.index += 1;

            if &self.keysyms[i] == &self.target {
                /* seek to next keycode */
                self.index = match self.index % self.per {
                    0 => self.index,
                    r => self.index + (self.per - r),
                };

                /* return keycode */
                return Some(((i / self.per) + self.min) as Keycode);
            }
        }

        None
    }
}

impl KeyMap {
    fn new(conn: &xcb::Connection) -> Result<Self, Error> {
        let setup = conn.get_setup();
        let min = setup.min_keycode();
        let max = setup.max_keycode();

        let key_cookie = conn.send_request(&x::GetKeyboardMapping {
            first_keycode: min,
            count: max - min + 1,
        });

        let mod_cookie = conn.send_request(&x::GetModifierMapping {});

        let keymap = conn.wait_for_reply(key_cookie)?;
        let modmap = conn.wait_for_reply(mod_cookie)?;

        Ok(KeyMap {
            min: min.into(),
            keymap: keymap,
            modmap: modmap,
        })
    }

    fn keycodes(&self, keysym: Keysym) -> KeycodeIterator {
        let per = self.keymap.keysyms_per_keycode() as usize;
        let keysyms = self.keymap.keysyms();

        KeycodeIterator {
            min: self.min as usize,
            per: per,
            index: 0,
            target: keysym,
            keysyms: keysyms,
        }
    }

    fn mask(&mut self, keysym: Keysym) -> Result<KeyModifier, Error> {
        /* taken from i3 */
        let modmap = self.modmap.keycodes();
        let keycodes_per_modifier = modmap.len() / 8;

        for modifier in 0..8 {
            for j in 0..keycodes_per_modifier {
                let modcode = modmap[(modifier * keycodes_per_modifier) + j];

                for keycode in self.keycodes(keysym) {
                    if keycode == modcode {
                        return Ok(KeyModifier::from_bits_truncate(1 << modifier));
                    }
                }
            }
        }

        Ok(KeyModifier::empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyPress {
    Press,
    Release,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    pub keysym: Keysym,
    pub mask: KeyModifier,
    pub press: KeyPress,
}

pub struct Keyboard {
    conn: Connection,
    keymap: KeyMap,
    num_lock: KeyModifier,
    caps_lock: KeyModifier,
    scroll_lock: KeyModifier,
    map: HashMap<(x::Window, KeyModifier, Keycode, bool), Key>,
}

impl Keyboard {
    pub fn new(conn: Connection) -> Result<Self, Error> {
        let mut keymap = KeyMap::new(conn.raw())?;

        let num_lock = keymap.mask(keysym::Num_Lock)?;
        let caps_lock = keymap.mask(keysym::Caps_Lock)?;
        let scroll_lock = keymap.mask(keysym::Scroll_Lock)?;

        Ok(Keyboard {
            conn: conn,
            keymap: keymap,
            num_lock: num_lock,
            caps_lock: caps_lock,
            scroll_lock: scroll_lock,
            map: HashMap::new(),
        })
    }

    #[inline]
    fn grab(&self,
            modifiers: KeyModifier,
            keycode: Keycode)
        -> xcb::VoidCookieChecked
    {
        let m = unsafe { x::ModMask::from_bits_unchecked(modifiers.bits()) };

        self.conn.send_request_checked(&x::GrabKey {
            owner_events: true,
            grab_window: self.conn.root,
            modifiers: m,
            key: keycode as u8,
            pointer_mode: x::GrabMode::Async,
            keyboard_mode: x::GrabMode::Async,
        })
    }

    pub fn bind(&mut self, bind: Key) -> Result<(), Error> {
        let mut cookies = Vec::with_capacity(8);

        for kc in self.keymap.keycodes(bind.keysym) {

            match bind.press {
                KeyPress::Press => {
                    self.map.entry((self.conn.root, bind.mask, kc, true))
                        .or_insert(bind.clone());
                }
                KeyPress::Release => {
                    self.map.entry((self.conn.root, bind.mask, kc, false))
                        .or_insert(bind.clone());
                }
                KeyPress::Both => {
                    self.map.entry((self.conn.root, bind.mask, kc, true))
                        .or_insert(bind.clone());

                    self.map.entry((self.conn.root, bind.mask, kc, false))
                        .or_insert(bind.clone());
                }
            }

            match bind.mask {
                KeyModifier::ANY => {
                    self.grab(bind.mask, kc);
                }
                _ => {
                    let mut cookie = self.grab(bind.mask, kc);
                    cookies.push(cookie);
                    cookie = self.grab(bind.mask | self.num_lock, kc);
                    cookies.push(cookie);
                    cookie = self.grab(bind.mask | self.caps_lock, kc);
                    cookies.push(cookie);
                    cookie = self.grab(bind.mask | self.scroll_lock, kc);
                    cookies.push(cookie);
                    cookie = self.grab(bind.mask | self.caps_lock | self.num_lock, kc);
                    cookies.push(cookie);
                    cookie = self.grab(bind.mask | self.scroll_lock | self.num_lock, kc);
                    cookies.push(cookie);
                    cookie = self.grab(bind.mask | self.scroll_lock | self.caps_lock, kc);
                    cookies.push(cookie);
                    cookie = self.grab(bind.mask | self.num_lock | self.scroll_lock | self.caps_lock, kc);
                    cookies.push(cookie);
                }
            }
        }

        for cookie in cookies {
            self.conn.check_request(cookie)?;
        }

        Ok(())
    }

    pub fn get(&self, root: x::Window, mask: x::KeyButMask, code: Keycode, press: bool) -> Option<&Key> {
        let mut modifiers = KeyModifier::from_bits_truncate(mask.bits());
        modifiers.remove(self.num_lock | self.caps_lock | self.scroll_lock);

        self.map
            .get(&(root, modifiers, code, press))
            .or_else(|| self.map.get(&(root, KeyModifier::ANY, code, press)))
    }

    pub fn press(&self, root: x::Window, mask: x::KeyButMask, code: Keycode, press: bool) {
        let key = self.get(root, mask, code, press);
        if let Some(x) = key {
            self.conn.produce(Event::Binding { key: x.clone() });
        }
    }
}
