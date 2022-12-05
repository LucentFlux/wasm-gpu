use lazy_init::Lazy;
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::RwLock;

/// A small (read: linear read/write time complexity - backed by an array) lazy map
pub struct LazySmallMap<K, V> {
    datas: Lazy<(K, V, Box<LazySmallMap<K, V>>)>,
}

impl<K, V> LazySmallMap<K, V> {
    pub fn empty() -> Self {
        Self { datas: Lazy::new() }
    }
}

impl<K: Eq, V> LazySmallMap<K, V> {
    pub fn get_or_create<F>(&self, key: K, gen: F) -> &V
    where
        F: FnOnce() -> V,
    {
        // Key (and gen) might not be clonable, so we use an option to track if we created a new node
        let key_gen = Rc::new(RwLock::new(Some((key, gen))));

        let local_key_gen = key_gen.clone();
        let creation_func = move || {
            // If we're creating, the key wasn't found so we're inserting
            let (key, gen) = local_key_gen.write().unwrap().take().unwrap();
            let val: V = gen();
            (key, val, Box::new(Self::empty()))
        };

        // Iterate down the line
        let mut ls = &self.datas;
        // SAFETY: calling our creation_func causes key_gen to be None, leaving the loop after finite time
        loop {
            let (k1, v, tail) = ls.get_or_create(&creation_func);

            match key_gen.read().unwrap().as_ref() {
                // If key is gone, we created a new node, so this is the one we want to return
                None => return v,
                // Otherwise check and break
                Some((k2, _)) => {
                    if k1 == k2 {
                        return v;
                    } else {
                        ls = &tail.datas
                    }
                }
            }
        }
    }
}

impl<K: Debug, V: Debug> Debug for LazySmallMap<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();

        let mut ls = self;
        while let Some((k, v, tail)) = ls.datas.get() {
            map.entry(k, v);
            ls = tail.as_ref()
        }

        map.finish()
    }
}
