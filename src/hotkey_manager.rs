use once_cell::sync::Lazy;
use rdev::{EventType, Key, listen};
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

// 全局热键管理器，支持多热键注册
pub struct HotKeyManager;

// 全局注册表，支持组合键
static HOTKEY_REGISTRY: Lazy<Arc<Mutex<HashMap<KeyCombo, Vec<Box<dyn Fn() + Send + 'static>>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

// 监听器启动状态
static LISTENER_STARTED: AtomicBool = AtomicBool::new(false);

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
    /// 初始化热键监听器
    pub fn init() {
        if LISTENER_STARTED.swap(true, Ordering::SeqCst) {
            return;
        }
        Self::start_global_listener();
    }

    /// 注册组合键或单键
    pub fn register_hotkey<F>(hotkeys: &[Key], callback: F)
    where
        F: Fn() + Send + 'static,
    {
        if !LISTENER_STARTED.load(Ordering::Relaxed) {
            log::warn!("HotKeyManager not initialized. Call HotKeyManager::init() first.");
            return;
        }

        let registry = HOTKEY_REGISTRY.clone();
        let mut map = registry.lock().unwrap();
        let key_set: HashSet<Key> = hotkeys.iter().cloned().collect();
        let combo = KeyCombo(key_set);
        map.entry(combo.clone())
            .or_default()
            .push(Box::new(callback));
        log::info!("Hotkey registered: {:?}", combo.0);
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
                            // 优化匹配逻辑：支持部分组合键匹配
                            if combo.0.iter().all(|k| pressed.contains(k)) {
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
