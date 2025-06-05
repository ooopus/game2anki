use rdev::Key;
use serde::Deserializer;
use std::collections::HashMap;

pub fn key_from_str(s: &str) -> Option<Key> {
    let map: HashMap<&'static str, Key> = [
        ("Alt", Key::Alt),
        ("AltGr", Key::AltGr),
        ("Backspace", Key::Backspace),
        ("CapsLock", Key::CapsLock),
        ("ControlLeft", Key::ControlLeft),
        ("ControlRight", Key::ControlRight),
        ("Delete", Key::Delete),
        ("DownArrow", Key::DownArrow),
        ("End", Key::End),
        ("Escape", Key::Escape),
        ("F1", Key::F1),
        ("F2", Key::F2),
        ("F3", Key::F3),
        ("F4", Key::F4),
        ("F5", Key::F5),
        ("F6", Key::F6),
        ("F7", Key::F7),
        ("F8", Key::F8),
        ("F9", Key::F9),
        ("F10", Key::F10),
        ("F11", Key::F11),
        ("F12", Key::F12),
        ("Home", Key::Home),
        ("LeftArrow", Key::LeftArrow),
        ("MetaLeft", Key::MetaLeft),
        ("MetaRight", Key::MetaRight),
        ("PageDown", Key::PageDown),
        ("PageUp", Key::PageUp),
        ("Return", Key::Return),
        ("RightArrow", Key::RightArrow),
        ("ShiftLeft", Key::ShiftLeft),
        ("ShiftRight", Key::ShiftRight),
        ("Space", Key::Space),
        ("Tab", Key::Tab),
        ("UpArrow", Key::UpArrow),
        ("PrintScreen", Key::PrintScreen),
        ("ScrollLock", Key::ScrollLock),
        ("Pause", Key::Pause),
        ("NumLock", Key::NumLock),
        ("BackQuote", Key::BackQuote),
        ("Num1", Key::Num1),
        ("Num2", Key::Num2),
        ("Num3", Key::Num3),
        ("Num4", Key::Num4),
        ("Num5", Key::Num5),
        ("Num6", Key::Num6),
        ("Num7", Key::Num7),
        ("Num8", Key::Num8),
        ("Num9", Key::Num9),
        ("Num0", Key::Num0),
        ("Minus", Key::Minus),
        ("Equal", Key::Equal),
        ("KeyQ", Key::KeyQ),
        ("KeyW", Key::KeyW),
        ("KeyE", Key::KeyE),
        ("KeyR", Key::KeyR),
        ("KeyT", Key::KeyT),
        ("KeyY", Key::KeyY),
        ("KeyU", Key::KeyU),
        ("KeyI", Key::KeyI),
        ("KeyO", Key::KeyO),
        ("KeyP", Key::KeyP),
        ("LeftBracket", Key::LeftBracket),
        ("RightBracket", Key::RightBracket),
        ("KeyA", Key::KeyA),
        ("KeyS", Key::KeyS),
        ("KeyD", Key::KeyD),
        ("KeyF", Key::KeyF),
        ("KeyG", Key::KeyG),
        ("KeyH", Key::KeyH),
        ("KeyJ", Key::KeyJ),
        ("KeyK", Key::KeyK),
        ("KeyL", Key::KeyL),
        ("SemiColon", Key::SemiColon),
        ("Quote", Key::Quote),
        ("BackSlash", Key::BackSlash),
        ("IntlBackslash", Key::IntlBackslash),
        ("KeyZ", Key::KeyZ),
        ("KeyX", Key::KeyX),
        ("KeyC", Key::KeyC),
        ("KeyV", Key::KeyV),
        ("KeyB", Key::KeyB),
        ("KeyN", Key::KeyN),
        ("KeyM", Key::KeyM),
        ("Comma", Key::Comma),
        ("Dot", Key::Dot),
        ("Slash", Key::Slash),
        ("Insert", Key::Insert),
        ("KpReturn", Key::KpReturn),
        ("KpMinus", Key::KpMinus),
        ("KpPlus", Key::KpPlus),
        ("KpMultiply", Key::KpMultiply),
        ("KpDivide", Key::KpDivide),
        ("Kp0", Key::Kp0),
        ("Kp1", Key::Kp1),
        ("Kp2", Key::Kp2),
        ("Kp3", Key::Kp3),
        ("Kp4", Key::Kp4),
        ("Kp5", Key::Kp5),
        ("Kp6", Key::Kp6),
        ("Kp7", Key::Kp7),
        ("Kp8", Key::Kp8),
        ("Kp9", Key::Kp9),
        ("KpDelete", Key::KpDelete),
        ("Function", Key::Function),
    ]
    .iter()
    .cloned()
    .collect();
    if let Some(&key) = map.get(s) {
        Some(key)
    } else if let Some(rest) = s.strip_prefix("Unknown(") {
        if let Some(num) = rest.strip_suffix(")") {
            if let Ok(val) = num.parse::<u32>() {
                return Some(Key::Unknown(val));
            }
        }
        None
    } else {
        None
    }
}

// 支持组合键字符串解析，如 "Ctrl+Alt+S"
pub fn keys_from_str(s: &str) -> Option<Vec<Key>> {
    let keys: Option<Vec<Key>> = s.split('+').map(|part| key_from_str(part.trim())).collect();
    keys
}

pub fn keys_from_str_de<'de, D>(deserializer: D) -> Result<Vec<Key>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{Error as _, Unexpected};
    // 支持字符串或字符串数组
    struct KeyVecVisitor;
    impl<'de> serde::de::Visitor<'de> for KeyVecVisitor {
        type Value = Vec<Key>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or a list of strings")
        }
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            keys_from_str(v).ok_or_else(|| E::custom("Unknown key(s) in combo"))
        }
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut keys = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                let k = key_from_str(&s).ok_or_else(|| {
                    A::Error::invalid_value(Unexpected::Str(&s), &"valid key name")
                })?;
                keys.push(k);
            }
            Ok(keys)
        }
    }
    deserializer.deserialize_any(KeyVecVisitor)
}
