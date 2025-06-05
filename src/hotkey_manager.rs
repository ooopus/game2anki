use once_cell::sync::Lazy;
use rdev::{EventType, Key, listen};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;

// 全局热键管理器，支持多热键注册
pub struct HotKeyManager;

// 全局注册表，支持组合键
static HOTKEY_REGISTRY: Lazy<Arc<Mutex<HashMap<KeyCombo, Vec<Box<dyn Fn() + Send + 'static>>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyCombo(pub HashSet<Key>);

impl std::hash::Hash for KeyCombo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // HashSet is not order-dependent, so hash all keys in sorted order
        let mut keys: Vec<_> = self.0.iter().collect();
        keys.sort_by_key(|k| format!("{:?}", k));
        for key in keys {
            std::mem::discriminant(key).hash(state);
            if let Key::Unknown(val) = key {
                val.hash(state);
            }
        }
    }
}

impl HotKeyManager {
    /// 注册组合键或单键
    pub fn register_hotkey<F>(hotkeys: &[Key], callback: F)
    where
        F: Fn() + Send + 'static,
    {
        let registry = HOTKEY_REGISTRY.clone();
        let mut map = registry.lock().unwrap();
        let key_set: HashSet<Key> = hotkeys.iter().cloned().collect();
        let combo = KeyCombo(key_set);
        map.entry(combo.clone())
            .or_default()
            .push(Box::new(callback));
        log::info!("Hotkey registered: {:?}", combo.0); // 注册成功提示
        // Start listener only once
        if map.len() == 1 {
            Self::start_global_listener();
        }
    }

    fn start_global_listener() {
        let registry = HOTKEY_REGISTRY.clone();
        thread::spawn(move || {
            let mut pressed: HashSet<Key> = HashSet::new();
            listen(move |event| match event.event_type {
                EventType::KeyPress(key) => {
                    if pressed.insert(key) {
                        let map = registry.lock().unwrap();
                        for (combo, callbacks) in map.iter() {
                            if combo.0.len() > 0
                                && combo.0.is_subset(&pressed)
                                && pressed.len() == combo.0.len()
                            {
                                log::info!(
                                    "Triggering {} callback(s) for combo: {:?}",
                                    callbacks.len(),
                                    combo
                                );
                                for cb in callbacks {
                                    log::debug!("Callback triggered for combo: {:?}", combo);
                                    cb();
                                }
                            }
                        }
                    }
                }
                EventType::KeyRelease(key) => {
                    pressed.remove(&key);
                }
                _ => {}
            })
            .unwrap();
        });
    }
}
