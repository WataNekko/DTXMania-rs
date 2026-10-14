use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, hash_map::Entry},
    fmt::Write,
};

use bevy::prelude::*;

use crate::assets::{config::SectionParser, song::DrumNote};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputTrigger {
    Keyboard(SlimDxKeyCode),
}

impl TryFrom<&str> for InputTrigger {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut chars = value.chars();

        let kind = chars.next().ok_or("String empty")?.to_ascii_uppercase();

        let _id = chars
            .next()
            .ok_or("Missing ID")?
            .to_digit(36)
            .ok_or("Invalid ID")?;

        let code = chars.as_str().parse().map_err(|_| "Invalid code")?;

        let trigger = match kind {
            'K' => Self::Keyboard(SlimDxKeyCode(code)),
            _ => return Err("Invalid type"),
        };

        Ok(trigger)
    }
}

impl InputTrigger {
    pub const fn from_key_code(value: KeyCode) -> Self {
        Self::Keyboard(SlimDxKeyCode::from_key_code(value))
    }

    /// Returns a struct that impl [std::fmt::Display] to format an in-game text for [InputTrigger].
    pub fn in_game_display(&self) -> InputTriggerInGameDisplay<'_> {
        InputTriggerInGameDisplay(self)
    }
}

impl From<KeyCode> for InputTrigger {
    fn from(value: KeyCode) -> Self {
        Self::from_key_code(value)
    }
}

pub struct InputTriggerInGameDisplay<'a>(&'a InputTrigger);

impl<'a> std::fmt::Display for InputTriggerInGameDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            InputTrigger::Keyboard(code) => write!(f, "Key {}", code),
        }
    }
}

/// Based on the old DirectInput key code mapping, for backwards compatibility.
/// https://github.com/limyz/DTXmaniaNX/blob/1.4.2/FDK/Code/02.Input/SlimDX.DirectInput.Key.cs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlimDxKeyCode(pub u32);

impl SlimDxKeyCode {
    pub const fn from_key_code(value: KeyCode) -> Self {
        let code = match value {
            KeyCode::Digit0 => 0,
            KeyCode::Digit1 => 1,
            KeyCode::Digit2 => 2,
            KeyCode::Digit3 => 3,
            KeyCode::Digit4 => 4,
            KeyCode::Digit5 => 5,
            KeyCode::Digit6 => 6,
            KeyCode::Digit7 => 7,
            KeyCode::Digit8 => 8,
            KeyCode::Digit9 => 9,
            KeyCode::KeyA => 10,
            KeyCode::KeyB => 11,
            KeyCode::KeyC => 12,
            KeyCode::KeyD => 13,
            KeyCode::KeyE => 14,
            KeyCode::KeyF => 15,
            KeyCode::KeyG => 16,
            KeyCode::KeyH => 17,
            KeyCode::KeyI => 18,
            KeyCode::KeyJ => 19,
            KeyCode::KeyK => 20,
            KeyCode::KeyL => 21,
            KeyCode::KeyM => 22,
            KeyCode::KeyN => 23,
            KeyCode::KeyO => 24,
            KeyCode::KeyP => 25,
            KeyCode::KeyQ => 26,
            KeyCode::KeyR => 27,
            KeyCode::KeyS => 28,
            KeyCode::KeyT => 29,
            KeyCode::KeyU => 30,
            KeyCode::KeyV => 31,
            KeyCode::KeyW => 32,
            KeyCode::KeyX => 33,
            KeyCode::KeyY => 34,
            KeyCode::KeyZ => 35,
            KeyCode::IntlRo => 36, // AbntC1
            // 37 AbntC2
            KeyCode::Quote => 38,
            KeyCode::ContextMenu => 39, // Applications
            // 40 AT
            // 41 AX
            KeyCode::Backspace => 42,
            KeyCode::Backslash => 43,
            KeyCode::LaunchApp2 => 44, // Calculator
            KeyCode::CapsLock => 45,
            // 46 Colon
            KeyCode::Comma => 47,
            KeyCode::Convert => 48,
            KeyCode::Delete => 49,
            KeyCode::ArrowDown => 50,
            KeyCode::End => 51,
            KeyCode::Equal => 52,
            KeyCode::Escape => 53,
            KeyCode::F1 => 54,
            KeyCode::F2 => 55,
            KeyCode::F3 => 56,
            KeyCode::F4 => 57,
            KeyCode::F5 => 58,
            KeyCode::F6 => 59,
            KeyCode::F7 => 60,
            KeyCode::F8 => 61,
            KeyCode::F9 => 62,
            KeyCode::F10 => 63,
            KeyCode::F11 => 64,
            KeyCode::F12 => 65,
            KeyCode::F13 => 66,
            KeyCode::F14 => 67,
            KeyCode::F15 => 68,
            KeyCode::Backquote => 69, // Grave
            KeyCode::Home => 70,
            KeyCode::Insert => 71,
            KeyCode::KanaMode => 72, // Kana
            KeyCode::Lang2 => 73,    // Kanji
            KeyCode::BracketLeft => 74,
            KeyCode::ControlLeft => 75,
            KeyCode::ArrowLeft => 76,
            KeyCode::AltLeft => 77,
            KeyCode::ShiftLeft => 78,
            KeyCode::SuperLeft => 79, // LeftWindowsKey
            KeyCode::LaunchMail => 80,
            KeyCode::MediaSelect => 81,
            KeyCode::MediaStop => 82,
            KeyCode::Minus => 83,
            KeyCode::AudioVolumeMute => 84,
            KeyCode::LaunchApp1 => 85, // MyComputer
            KeyCode::MediaTrackNext => 86,
            KeyCode::NonConvert => 87,
            KeyCode::NumLock => 88,
            KeyCode::Numpad0 => 89,
            KeyCode::Numpad1 => 90,
            KeyCode::Numpad2 => 91,
            KeyCode::Numpad3 => 92,
            KeyCode::Numpad4 => 93,
            KeyCode::Numpad5 => 94,
            KeyCode::Numpad6 => 95,
            KeyCode::Numpad7 => 96,
            KeyCode::Numpad8 => 97,
            KeyCode::Numpad9 => 98,
            KeyCode::NumpadComma => 99,
            KeyCode::NumpadEnter => 100,
            KeyCode::NumpadEqual => 101,
            KeyCode::NumpadSubtract => 102, // NumberPadMinus
            KeyCode::NumpadDecimal => 103,  // NumberPadPeriod
            KeyCode::NumpadAdd => 104,      // NumberPadPlus
            KeyCode::NumpadDivide => 105,   // NumberPadSlash
            KeyCode::NumpadMultiply => 106, // NumberPadStar
            KeyCode::IntlBackslash => 107,  // Oem102
            KeyCode::PageDown => 108,
            KeyCode::PageUp => 109,
            KeyCode::Pause => 110,
            KeyCode::Period => 111,
            KeyCode::MediaPlayPause => 112,
            KeyCode::Power => 113,
            KeyCode::MediaTrackPrevious => 114,
            KeyCode::BracketRight => 115,
            KeyCode::ControlRight => 116,
            KeyCode::Enter => 117, // Return
            KeyCode::ArrowRight => 118,
            KeyCode::AltRight => 119,
            KeyCode::ShiftRight => 120,
            KeyCode::SuperRight => 121, // RightWindowsKey
            KeyCode::ScrollLock => 122,
            KeyCode::Semicolon => 123,
            KeyCode::Slash => 124,
            KeyCode::Sleep => 125,
            KeyCode::Space => 126,
            // 127 Stop
            KeyCode::PrintScreen => 128,
            KeyCode::Tab => 129,
            // 130 Underline
            // 131 Unlabeled
            KeyCode::ArrowUp => 132,
            KeyCode::AudioVolumeDown => 133,
            KeyCode::AudioVolumeUp => 134,
            KeyCode::WakeUp => 135,
            KeyCode::BrowserBack => 136, // WebBack
            KeyCode::BrowserFavorites => 137,
            KeyCode::BrowserForward => 138,
            KeyCode::BrowserHome => 139,
            KeyCode::BrowserRefresh => 140,
            KeyCode::BrowserSearch => 141,
            KeyCode::BrowserStop => 142,
            KeyCode::IntlYen => 143, // Yen
            _ => 144,                // Unknown
        };

        Self(code)
    }
}

impl From<KeyCode> for SlimDxKeyCode {
    fn from(value: KeyCode) -> Self {
        Self::from_key_code(value)
    }
}

impl std::fmt::Display for SlimDxKeyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Copied from:
        // https://github.com/limyz/DTXmaniaNX/blob/1.4.2/DTXMania/Code/Stage/04.Config/CActConfigKeyAssign.cs#L221
        let label = match self.0 {
            0x35 => "[ESC]",
            1 => "[ 1 ]",
            2 => "[ 2 ]",
            3 => "[ 3 ]",
            4 => "[ 4 ]",
            5 => "[ 5 ]",
            6 => "[ 6 ]",
            7 => "[ 7 ]",
            8 => "[ 8 ]",
            9 => "[ 9 ]",
            0 => "[ 0 ]",
            0x53 => "[ - ]",
            0x34 => "[ = ]",
            0x2a => "[BSC]",
            0x81 => "[TAB]",
            0x1a => "[ Q ]",
            0x20 => "[ W ]",
            14 => "[ E ]",
            0x1b => "[ R ]",
            0x1d => "[ T ]",
            0x22 => "[ Y ]",
            30 => "[ U ]",
            0x12 => "[ I ]",
            0x18 => "[ O ]",
            0x19 => "[ P ]",
            0x4a => "[ [ ]",
            0x73 => "[ ] ]",
            0x75 => "[Enter]",
            0x4b => "[L-Ctrl]",
            10 => "[ A ]",
            0x1c => "[ S ]",
            13 => "[ D ]",
            15 => "[ F ]",
            0x10 => "[ G ]",
            0x11 => "[ H ]",
            0x13 => "[ J ]",
            20 => "[ K ]",
            0x15 => "[ L ]",
            0x7b => "[ ; ]",
            0x26 => "[ ' ]",
            0x45 => "[ ` ]",
            0x4e => "[L-Shift]",
            0x2b => r"[ \ ]",
            0x23 => "[ Z ]",
            0x21 => "[ X ]",
            12 => "[ C ]",
            0x1f => "[ V ]",
            11 => "[ B ]",
            0x17 => "[ N ]",
            0x16 => "[ M ]",
            0x2f => "[ , ]",
            0x6f => "[ . ]",
            0x7c => "[ / ]",
            120 => "[R-Shift]",
            0x6a => "[ * ]",
            0x4d => "[L-Alt]",
            0x7e => "[Space]",
            0x2d => "[CAPS]",
            0x36 => "[F1]",
            0x37 => "[F2]",
            0x38 => "[F3]",
            0x39 => "[F4]",
            0x3a => "[F5]",
            0x3b => "[F6]",
            60 => "[F7]",
            0x3d => "[F8]",
            0x3e => "[F9]",
            0x3f => "[F10]",
            0x58 => "[NumLock]",
            0x7a => "[Scroll]",
            0x60 => "[NPad7]",
            0x61 => "[NPad8]",
            0x62 => "[NPad9]",
            0x66 => "[NPad-]",
            0x5d => "[NPad4]",
            0x5e => "[NPad5]",
            0x5f => "[NPad6]",
            0x68 => "[NPad+]",
            90 => "[NPad1]",
            0x5b => "[NPad2]",
            0x5c => "[NPad3]",
            0x59 => "[NPad0]",
            0x67 => "[NPad.]",
            0x40 => "[F11]",
            0x41 => "[F12]",
            0x42 => "[F13]",
            0x43 => "[F14]",
            0x44 => "[F15]",
            0x48 => "[Kana]",
            0x24 => "[ ? ]",
            0x30 => "[Henkan]",
            0x57 => "[MuHenkan]",
            0x8f => r"[ \ ]",
            0x25 => "[NPad.]",
            0x65 => "[NPad=]",
            0x72 => "[ ^ ]",
            40 => "[ @ ]",
            0x2e => "[ : ]",
            130 => "[ _ ]",
            0x49 => "[Kanji]",
            0x7f => "[Stop]",
            0x29 => "[AX]",
            100 => "[NPEnter]",
            0x74 => "[R-Ctrl]",
            0x54 => "[Mute]",
            0x2c => "[Calc]",
            0x70 => "[PlayPause]",
            0x52 => "[MediaStop]",
            0x85 => "[Volume-]",
            0x86 => "[Volume+]",
            0x8b => "[WebHome]",
            0x63 => "[NPad,]",
            0x69 => "[ / ]",
            0x80 => "[PrtScn]",
            0x77 => "[R-Alt]",
            110 => "[Pause]",
            70 => "[Home]",
            0x84 => "[Up]",
            0x6d => "[PageUp]",
            0x4c => "[Left]",
            0x76 => "[Right]",
            0x33 => "[End]",
            50 => "[Down]",
            0x6c => "[PageDown]",
            0x47 => "[Insert]",
            0x31 => "[Delete]",
            0x4f => "[L-Win]",
            0x79 => "[R-Win]",
            0x27 => "[APP]",
            0x71 => "[Power]",
            0x7d => "[Sleep]",
            0x87 => "[Wake]",
            144 => "[Unknown]",
            code => return write!(f, "0x{:02X}", code),
        };

        f.write_str(label)
    }
}

pub type InputTriggerList = Vec<Option<InputTrigger>>;

pub struct DrumInputMapParser {
    notes_with_defaults: HashMap<&'static str, (DrumNote, &'static [InputTrigger])>,
    note_to_triggers: HashMap<DrumNote, InputTriggerList>,
    trigger_to_note: HashMap<InputTrigger, DrumNote>,
}

impl DrumInputMapParser {
    pub const CONFIG_KEYS_AND_DEFAULTS: &[(DrumNote, &str, &[InputTrigger])] = {
        use KeyCode::*;

        const fn k(code: KeyCode) -> InputTrigger {
            InputTrigger::from_key_code(code)
        }

        &[
            (DrumNote::HiHatClose, "HH", &[k(KeyX)]),
            (DrumNote::Snare, "SD", &[k(KeyC), k(KeyD)]),
            (DrumNote::BassDrum, "BD", &[k(Space), k(Convert)]),
            (DrumNote::HighTom, "HT", &[k(KeyV), k(KeyF)]),
            (DrumNote::LowTom, "LT", &[k(KeyB), k(KeyG)]),
            (DrumNote::FloorTom, "FT", &[k(KeyN), k(KeyH)]),
            (DrumNote::Cymbal, "CY", &[k(KeyM), k(KeyJ)]),
            (DrumNote::HiHatOpen, "HO", &[k(KeyS)]),
            (DrumNote::RideCymbal, "RD", &[k(Comma), k(KeyK)]),
            (DrumNote::LeftCymbal, "LC", &[k(KeyZ), k(KeyA)]),
            (DrumNote::LeftPedal, "LP", &[k(NonConvert), k(ControlLeft)]),
            (DrumNote::LeftBass, "LBD", &[k(AltLeft)]),
        ]
    };

    pub fn new() -> Self {
        Self {
            notes_with_defaults: Self::CONFIG_KEYS_AND_DEFAULTS
                .iter()
                .map(|&(note, key, default)| (key, (note, default)))
                .collect(),
            note_to_triggers: HashMap::new(),
            trigger_to_note: HashMap::new(),
        }
    }
}

impl SectionParser for DrumInputMapParser {
    fn parse_entry<'a>(&mut self, key: &'a str, value: &'a str) -> Result<(), Cow<'a, str>> {
        let &(note, _) = self.notes_with_defaults.get(key).ok_or("Unknown key")?;

        let trigger_list = self
            .note_to_triggers
            .entry(note)
            .and_modify(|l| {
                for trigger in l.drain(..).flatten() {
                    self.trigger_to_note.remove(&trigger);
                }
            })
            .or_default();
        let mut trigger_set = HashSet::new();

        let mut result: Result<(), Cow<str>> = Ok(());

        for s in value.split(',').map(str::trim) {
            if s.is_empty() {
                trigger_list.push(None);
                continue;
            }

            match InputTrigger::try_from(s) {
                Ok(trigger) => {
                    let new = trigger_set.insert(trigger);
                    if new {
                        trigger_list.push(Some(trigger));
                    }
                    // list is reduced if the trigger's already bound to the same note.
                }
                Err(e) => {
                    // list is reduced if the trigger cannot be parsed.
                    match result.as_mut() {
                        Ok(()) => {
                            result = Err(format!("Invalid trigger(s) \"{}\" ({})", s, e).into());
                        }
                        Err(existing_err_msg) => {
                            write!(existing_err_msg.to_mut(), ", \"{}\" ({})", s, e).unwrap();
                        }
                    }
                }
            }
        }

        let trimmed_len = trigger_list
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        trigger_list.truncate(trimmed_len);

        for trigger in trigger_set {
            if let Some(old_note) = self.trigger_to_note.insert(trigger, note) {
                let old_list = self
                    .note_to_triggers
                    .get_mut(&old_note)
                    .expect("If bound to old_note then old trigger list should exist.");

                let old_entry = old_list
                    .iter_mut()
                    .find(|t| **t == Some(trigger))
                    .expect("If bound to old_note then trigger should be in old_list.");

                old_entry.take();
            }
        }

        result
    }
}

impl DrumInputMapParser {
    pub fn finalize(self) -> DrumInputMap {
        let Self {
            notes_with_defaults,
            mut note_to_triggers,
            mut trigger_to_note,
        } = self;

        for (note, default) in notes_with_defaults.into_values() {
            note_to_triggers.entry(note).or_insert_with(|| {
                default
                    .iter()
                    .filter_map(|&trigger| match trigger_to_note.entry(trigger) {
                        Entry::Occupied(_) => None,
                        Entry::Vacant(entry) => {
                            entry.insert(note);
                            Some(Some(trigger))
                        }
                    })
                    .collect()
            });
        }

        DrumInputMap {
            note_to_triggers,
            trigger_to_note,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrumInputMap {
    note_to_triggers: HashMap<DrumNote, InputTriggerList>,
    trigger_to_note: HashMap<InputTrigger, DrumNote>,
}

impl DrumInputMap {
    pub fn trigger_list(&self, note: &DrumNote) -> &InputTriggerList {
        &self.note_to_triggers[note]
    }

    pub fn trigger_note(&self, trigger: &InputTrigger) -> Option<DrumNote> {
        self.trigger_to_note.get(trigger).copied()
    }
}
