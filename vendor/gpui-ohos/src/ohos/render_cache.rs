use rustc_hash::FxHashMap;
use std::{
    hash::Hash,
    sync::{Arc, Mutex},
};
/// Device-scoped GPU objects are reused only for a complete compatibility key.
pub(crate) struct RenderCache<K, V> {
    entries: Mutex<FxHashMap<K, Arc<V>>>,
}
impl<K, V> Default for RenderCache<K, V> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(FxHashMap::default()),
        }
    }
}
impl<K: Hash + Eq, V> RenderCache<K, V> {
    pub(crate) fn get_or_insert_with(&self, key: K, create: impl FnOnce() -> V) -> Arc<V> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(key)
            .or_insert_with(|| Arc::new(create()))
            .clone()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_keys_reuse_resources_and_variants_remain_separate() {
        let cache = RenderCache::default();
        let first = cache.get_or_insert_with((1, 4), || 12);
        let same = cache.get_or_insert_with((1, 4), || panic!("recreated shared pipeline"));
        assert!(Arc::ptr_eq(&first, &same));
        let other = cache.get_or_insert_with((2, 4), || 14);
        assert!(!Arc::ptr_eq(&first, &other));
    }
}
