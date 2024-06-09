use crate::{make_hash, make_hasher};
use allocator_api2::alloc::{Allocator, Global};
use core::hash::Hash;
use hashbrown::hash_map::DefaultHashBuilder;
use hashbrown::raw::{Bucket, RawTable};
use hashbrown::Equivalent;

pub struct Interner<'a, K: ?Sized, A: Allocator = Global> {
    builder: DefaultHashBuilder,
    table: RawTable<&'a K, A>,
}

#[derive(Debug)]
pub struct Interned<'a, K: ?Sized> {
    inner: &'a K,
}

impl<'a, K: ?Sized> Interned<'a, K> {
    fn from_bucket(bucket: Bucket<&'a K>) -> Self {
        // SAFETY: we're comparing inital &'a K here that was inserted
        // into the interner, which is bound to hold
        // for the lifetime of the interner.
        Self {
            inner: unsafe { *bucket.as_ref() },
        }
    }

    pub fn val(&self) -> &'a K {
        self.inner
    }
}

impl<'a, K: ?Sized> PartialEq for Interned<'a, K> {
    fn eq(&self, rhs: &Self) -> bool {
        core::ptr::eq(self.inner, rhs.inner)
    }
}
impl<'a, K: ?Sized> Eq for Interned<'a, K> {}

impl<'a, K: Eq + Hash + ?Sized> Interner<'a, K> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<'a, K: Eq + Hash + ?Sized> Default for Interner<'a, K> {
    fn default() -> Self {
        Self::new_in(Global::default())
    }
}

impl<'a, K: Eq + Hash + ?Sized, A: Allocator> Interner<'a, K, A> {
    pub fn new_in(alloc: A) -> Self {
        Self {
            builder: DefaultHashBuilder::default(),
            table: RawTable::new_in(alloc),
        }
    }

    pub fn intern(&mut self, internee: &'a K) -> Interned<'a, K> {
        let hash = make_hash(&self.builder, internee);
        let hasher = make_hasher::<&K, _>(&self.builder);

        let bucket =
            match self
                .table
                .find_or_find_insert_slot(hash, equivalent_key(internee), hasher)
            {
                Ok(bucket) => bucket,
                Err(slot) => unsafe { self.table.insert_in_slot(hash, slot, internee) },
            };
        Interned::from_bucket(bucket)
    }

    pub fn intern_once(&mut self, internee: &'a K) -> Option<Interned<'a, K>> {
        let hash = make_hash(&self.builder, internee);
        let hasher = make_hasher::<&K, _>(&self.builder);

        match self
            .table
            .find_or_find_insert_slot(hash, equivalent_key(internee), hasher)
        {
            Ok(_) => None,
            Err(slot) => {
                let bucket = unsafe { self.table.insert_in_slot(hash, slot, internee) };
                Some(Interned::from_bucket(bucket))
            }
        }
    }
}

fn equivalent_key<K: ?Sized + Equivalent<K>>(k: &K) -> impl Fn(&&K) -> bool + '_ {
    move |x| (*x).equivalent(k)
}
