use std::sync::{Arc, RwLock};

/// A value that readers snapshot and writers replace whole.
///
/// Readers clone an `Arc` and never hold a lock while they work, so a slow
/// search cannot block an index rebuild, and a rebuild cannot block a search.
pub struct Shared<T>(RwLock<Arc<T>>);

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Self(RwLock::new(Arc::new(value)))
    }

    pub fn get(&self) -> Arc<T> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, value: T) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(value);
    }
}

impl<T: Clone> Shared<T> {
    /// Copy, change, and publish under one write lock, so concurrent updates
    /// cannot overwrite each other.
    pub fn update<R>(&self, change: impl FnOnce(&mut T) -> R) -> R {
        let mut guard = self.0.write().unwrap_or_else(|e| e.into_inner());
        let mut value = T::clone(&guard);
        let result = change(&mut value);
        *guard = Arc::new(value);
        result
    }
}

impl<T: Default> Default for Shared<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_keep_their_snapshot_after_a_write() {
        let shared = Shared::new(vec![1]);
        let before = shared.get();
        shared.update(|value| value.push(2));
        assert_eq!(*before, [1]);
        assert_eq!(*shared.get(), [1, 2]);
    }
}
