#![feature(ptr_metadata)]
use core::hash::{BuildHasher, Hash, Hasher};

pub mod copying;
pub mod nocopy;
pub mod string_copy;
pub mod string_nocopy;

pub(crate) fn make_hash<Q, S>(hash_builder: &S, val: &Q) -> u64
where
    Q: Hash + ?Sized,
    S: BuildHasher,
{
    let mut state = hash_builder.build_hasher();
    val.hash(&mut state);
    state.finish()
}

pub(crate) fn make_hasher<Q, S>(hash_builder: &S) -> impl Fn(&Q) -> u64 + '_
where
    Q: Hash,
    S: BuildHasher,
{
    move |val| make_hash::<Q, S>(hash_builder, val)
}
