//! We take inspiration from projects such as CHERI to provide each pointer into a GPU object with
//! a set of capabilities describing the limits of the pointer. Ultimately this is bounds information,
//! coupled with object identifiers - a pointer into a buffer should only be valid for that buffer,
//! and for the range of the buffer at the time that the pointer was created.
//! This allows snapshotting an instance into a builder because we can add to the capability set and not
//! give more power to past pointers, and not give new pointers power to dereference invalid memory
//! in old instances and builders

use crate::atomic_counter::AtomicCounter;
use std::collections::HashSet;
use std::sync::Arc;

// Used for UUID generation
static COUNTER: AtomicCounter = AtomicCounter::new();

/// This stores capability information of a buffer, to then be checked against on pointer dereference.
/// It must be told when the buffer changes size.
#[derive(Clone, Debug)]
pub struct CapabilityStoreData {
    valid_ids: HashSet<usize>,
    // The sizes of the buffer at the time of each id in valid_ids, stored as (size, id)
    id_sizes: Vec<(usize, usize)>,

    current_id: usize,
}

#[derive(Clone, Debug)]
pub struct CapabilityStore(Arc<CapabilityStoreData>);

impl CapabilityStore {
    pub fn new(initial_size: usize) -> Self {
        let new_id = COUNTER.next();
        Self(Arc::new(CapabilityStoreData {
            valid_ids: HashSet::from([new_id]),
            id_sizes: vec![(initial_size, new_id)],
            current_id: new_id,
        }))
    }

    pub fn resize_ref(&self, new_size: usize) -> Self {
        let mut valid_ids = HashSet::new();
        let mut id_sizes = Vec::new();
        for (size, id) in &self.0.id_sizes {
            if *size <= new_size {
                valid_ids.insert(*id);
                id_sizes.push((*size, *id));
            }
        }

        let new_id = COUNTER.next();

        valid_ids.insert(new_id);
        id_sizes.push((new_size, new_id));

        Self(Arc::new(CapabilityStoreData {
            valid_ids,
            id_sizes,
            current_id: new_id,
        }))
    }

    pub fn get_cap(&self) -> Capability {
        Capability {
            id: self.0.current_id,
        }
    }

    pub fn check(&self, cap: &Capability) -> bool {
        self.0.valid_ids.contains(&cap.id)
    }
}

/// This is the capability that a pointer stores into a buffer, to be checked against a store.
#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug, Hash)]
pub struct Capability {
    id: usize,
}
