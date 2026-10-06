use rustc_hash::FxHashMap as HashMap;

use openharmony_ability::xcomponent::{Action, KeyCode, KeyEventData};

use crate::{
    Capslock, KeyDownEvent, KeyUpEvent, KeybindingKeystroke, Keystroke, Modifiers,
    ModifiersChangedEvent, PlatformInput, PlatformKeyboardLayout, PlatformKeyboardMapper,
};

#[derive(Default)]
pub(crate) struct OhosKeyState {
    pressed: Vec<KeyCode>,
    modifiers: Modifiers,
    capslock: Capslock,
}

impl OhosKeyState {
    pub(crate) fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub(crate) fn capslock(&self) -> Capslock {
        self.capslock
    }

    pub(crate) fn clear_pressed(&mut self) -> Option<PlatformInput> {
        self.pressed.clear();
        if self.modifiers == Modifiers::default() {
            return None;
        }
        self.modifiers = Modifiers::default();
        Some(self.modifiers_changed())
    }

    pub(crate) fn handle(&mut self, event: &KeyEventData) -> Vec<PlatformInput> {
        let held = self.pressed.contains(&event.code);
        match event.action {
            Action::Down if !held => self.pressed.push(event.code),
            Action::Up => self.pressed.retain(|code| *code != event.code),
            Action::Down | Action::Unknown => {}
        }
        if event.action == Action::Unknown {
            return Vec::new();
        }

        let previous_modifiers = self.modifiers;
        let previous_capslock = self.capslock;
        if event.code == KeyCode::CapsLock && event.action == Action::Down && !held {
            self.capslock.on = !self.capslock.on;
        }
        self.modifiers = Modifiers {
            control: self.pressed.contains(&KeyCode::CtrlLeft)
                || self.pressed.contains(&KeyCode::CtrlRight),
            alt: self.pressed.contains(&KeyCode::AltLeft)
                || self.pressed.contains(&KeyCode::AltRight),
            shift: self.pressed.contains(&KeyCode::ShiftLeft)
                || self.pressed.contains(&KeyCode::ShiftRight),
            platform: self.pressed.contains(&KeyCode::MetaLeft)
                || self.pressed.contains(&KeyCode::MetaRight),
            function: self.pressed.contains(&KeyCode::Fn)
                || self.pressed.contains(&KeyCode::Function),
        };

        let mut output = Vec::with_capacity(2);
        if self.modifiers != previous_modifiers || self.capslock != previous_capslock {
            output.push(self.modifiers_changed());
        }
        if is_modifier(event.code) {
            return output;
        }
        let Some(key) = key_name(event.code) else {
            return output;
        };
        let key_char = printable_char(event.code, self.modifiers, self.capslock);
        let keystroke = Keystroke {
            modifiers: self.modifiers,
            key: key.to_owned(),
            key_char,
        };
        match event.action {
            Action::Down => output.push(PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held: held,
                prefer_character_input: false,
            })),
            Action::Up => output.push(PlatformInput::KeyUp(KeyUpEvent { keystroke })),
            Action::Unknown => {}
        }
        output
    }

    fn modifiers_changed(&self) -> PlatformInput {
        PlatformInput::ModifiersChanged(ModifiersChangedEvent {
            modifiers: self.modifiers,
            capslock: self.capslock,
        })
    }
}

fn is_modifier(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::CtrlLeft
            | KeyCode::CtrlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::MetaLeft
            | KeyCode::MetaRight
            | KeyCode::Fn
            | KeyCode::Function
            | KeyCode::CapsLock
    )
}

fn letter(code: KeyCode) -> Option<char> {
    match code {
        KeyCode::A => Some('a'),
        KeyCode::B => Some('b'),
        KeyCode::C => Some('c'),
        KeyCode::D => Some('d'),
        KeyCode::E => Some('e'),
        KeyCode::F => Some('f'),
        KeyCode::G => Some('g'),
        KeyCode::H => Some('h'),
        KeyCode::I => Some('i'),
        KeyCode::J => Some('j'),
        KeyCode::K => Some('k'),
        KeyCode::L => Some('l'),
        KeyCode::M => Some('m'),
        KeyCode::N => Some('n'),
        KeyCode::O => Some('o'),
        KeyCode::P => Some('p'),
        KeyCode::Q => Some('q'),
        KeyCode::R => Some('r'),
        KeyCode::S => Some('s'),
        KeyCode::T => Some('t'),
        KeyCode::U => Some('u'),
        KeyCode::V => Some('v'),
        KeyCode::W => Some('w'),
        KeyCode::X => Some('x'),
        KeyCode::Y => Some('y'),
        KeyCode::Z => Some('z'),
        _ => None,
    }
}

fn key_name(code: KeyCode) -> Option<&'static str> {
    if let Some(letter) = letter(code) {
        return Some(match letter {
            'a' => "a",
            'b' => "b",
            'c' => "c",
            'd' => "d",
            'e' => "e",
            'f' => "f",
            'g' => "g",
            'h' => "h",
            'i' => "i",
            'j' => "j",
            'k' => "k",
            'l' => "l",
            'm' => "m",
            'n' => "n",
            'o' => "o",
            'p' => "p",
            'q' => "q",
            'r' => "r",
            's' => "s",
            't' => "t",
            'u' => "u",
            'v' => "v",
            'w' => "w",
            'x' => "x",
            'y' => "y",
            'z' => "z",
            _ => unreachable!(),
        });
    }
    match code {
        KeyCode::Key0 => Some("0"),
        KeyCode::Key1 => Some("1"),
        KeyCode::Key2 => Some("2"),
        KeyCode::Key3 => Some("3"),
        KeyCode::Key4 => Some("4"),
        KeyCode::Key5 => Some("5"),
        KeyCode::Key6 => Some("6"),
        KeyCode::Key7 => Some("7"),
        KeyCode::Key8 => Some("8"),
        KeyCode::Key9 => Some("9"),
        KeyCode::DpadUp => Some("up"),
        KeyCode::DpadDown => Some("down"),
        KeyCode::DpadLeft => Some("left"),
        KeyCode::DpadRight => Some("right"),
        KeyCode::DpadCenter | KeyCode::Enter | KeyCode::NumpadEnter => Some("enter"),
        KeyCode::Del => Some("backspace"),
        KeyCode::ForwardDel => Some("delete"),
        KeyCode::Tab => Some("tab"),
        KeyCode::Space => Some("space"),
        KeyCode::Escape => Some("escape"),
        KeyCode::PageUp => Some("pageup"),
        KeyCode::PageDown => Some("pagedown"),
        KeyCode::MoveHome => Some("home"),
        KeyCode::MoveEnd => Some("end"),
        KeyCode::Insert => Some("insert"),
        KeyCode::Comma => Some(","),
        KeyCode::Period => Some("."),
        KeyCode::Grave => Some("`"),
        KeyCode::Minus => Some("-"),
        KeyCode::Equals => Some("="),
        KeyCode::LeftBracket => Some("["),
        KeyCode::RightBracket => Some("]"),
        KeyCode::Backslash => Some("\\"),
        KeyCode::Semicolon => Some(";"),
        KeyCode::Apostrophe => Some("'"),
        KeyCode::Slash => Some("/"),
        KeyCode::F1 => Some("f1"),
        KeyCode::F2 => Some("f2"),
        KeyCode::F3 => Some("f3"),
        KeyCode::F4 => Some("f4"),
        KeyCode::F5 => Some("f5"),
        KeyCode::F6 => Some("f6"),
        KeyCode::F7 => Some("f7"),
        KeyCode::F8 => Some("f8"),
        KeyCode::F9 => Some("f9"),
        KeyCode::F10 => Some("f10"),
        KeyCode::F11 => Some("f11"),
        KeyCode::F12 => Some("f12"),
        _ => None,
    }
}

fn printable_char(code: KeyCode, modifiers: Modifiers, capslock: Capslock) -> Option<String> {
    if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }
    let character = if let Some(letter) = letter(code) {
        if modifiers.shift ^ capslock.on {
            letter.to_ascii_uppercase()
        } else {
            letter
        }
    } else {
        let key = key_name(code)?;
        if key.len() != 1 {
            return None;
        }
        let plain = key.chars().next()?;
        if modifiers.shift {
            match plain {
                '1' => '!',
                '2' => '@',
                '3' => '#',
                '4' => '$',
                '5' => '%',
                '6' => '^',
                '7' => '&',
                '8' => '*',
                '9' => '(',
                '0' => ')',
                '-' => '_',
                '=' => '+',
                '[' => '{',
                ']' => '}',
                '\\' => '|',
                ';' => ':',
                '\'' => '"',
                ',' => '<',
                '.' => '>',
                '/' => '?',
                '`' => '~',
                _ => plain,
            }
        } else {
            plain
        }
    };
    Some(character.to_string())
}

pub(crate) struct OhosKeyboardLayout;

impl PlatformKeyboardLayout for OhosKeyboardLayout {
    fn id(&self) -> &str {
        "ohos-default"
    }

    fn name(&self) -> &str {
        "OHOS Default"
    }
}

pub(crate) struct OhosKeyboardMapper;

impl PlatformKeyboardMapper for OhosKeyboardMapper {
    fn map_key_equivalent(
        &self,
        keystroke: Keystroke,
        _use_key_equivalents: bool,
    ) -> KeybindingKeystroke {
        KeybindingKeystroke::from_keystroke(keystroke)
    }

    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>> {
        None
    }
}
