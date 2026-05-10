use std::collections::{HashMap, HashSet};

pub trait BackingStore<K, V> {
    fn save(&mut self, key: K, value: V);
}

pub struct Cache<K, V, S> {
    pub dirty: HashSet<K>,
    pub data: HashMap<K, V>,
    pub store: S,
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone, S: BackingStore<K, V>> Cache<K, V, S> {
    pub fn flush(&mut self) {
        for key in self.dirty.drain() {
            if let Some(val) = self.data.get(&key) {
                self.store.save(key.clone(), val.clone());
            }
        }
    }
}
fn main() {}
