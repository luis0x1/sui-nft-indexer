use std::{borrow::Borrow, collections::BTreeMap};

#[derive(Debug)]
pub struct BTreeMapLimit<K, V> {
    map: BTreeMap<K, V>,
    size: u64,
    limit: u64,
}

impl<K, V> BTreeMapLimit<K, V> {
    pub fn new(limit: u64) -> Self {
        Self { map: BTreeMap::<K, V>::new(), size: 0, limit }
    }
    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool where K: Borrow<Q> + Ord, Q: Ord {
        self.map.contains_key(key)
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V> where K: Borrow<Q> + Ord, Q: Ord {
        self.map.get(key)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> where K: Ord {
        let inserted = self.map.insert(key, value);
        if inserted.is_none() {
            self.size += 1;
        }

        if self.size > self.limit {
            self.map.pop_first();
            self.size -= 1;
        }

        inserted
    }

    pub fn insert_many(&mut self, values: Vec<(K, V)>) where K: Ord {
        for value in values {
            self.insert(value.0, value.1);
        }
    }
}