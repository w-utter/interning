use crate::{make_hash, make_hasher};
use allocator_api2::alloc::{Allocator, Global};
use core::alloc::Layout;
use core::hash::Hash;
use core::marker::PhantomData;
use core::ptr::{NonNull, Pointee};
use hashbrown::hash_map::DefaultHashBuilder;
use hashbrown::raw::{Bucket, RawTable};
use hashbrown::Equivalent;

struct StackBuf<K: ?Sized, const NULL_TERMINATOR: bool = false> {
    _pd: PhantomData<K>,
    chunk: Option<(Layout, NonNull<[u8]>)>,
    offset: usize,
}

impl<K: ?Sized, const NT: bool> StackBuf<K, NT> {
    fn new() -> Self {
        Self {
            _pd: PhantomData,
            chunk: None,
            offset: 0,
        }
    }

    //resizes the buf to make it so that its at least K, otherwise doubles the size
    fn new_layout(&self, val: &K) -> Layout {
        let min_size =
            core::mem::size_of_val(val) + core::mem::size_of::<<K as Pointee>::Metadata>();
        let old_size = match self.chunk {
            Some((layout, _)) => layout.size(),
            _ => {
                let align = core::mem::align_of_val(val);
                return unsafe { Layout::from_size_align_unchecked(min_size, align) };
            }
        };

        let min = old_size
            .checked_add(min_size)
            .filter(|n| *n <= isize::MIN as usize)
            .expect("overflow");

        let clamped_size = old_size
            .checked_mul(2)
            .map(|d| std::cmp::max(d, min))
            .unwrap_or(min)
            .next_power_of_two();

        // SAFETY: verified all preconditions of layout
        unsafe {
            Layout::from_size_align_unchecked(clamped_size, self.chunk.unwrap_unchecked().0.align())
        }
    }

    //used when reading from/writing to data
    const METADATA_SIZE: usize = core::mem::size_of::<<K as Pointee>::Metadata>();

    unsafe fn get_unchecked<'a>(&'a self, offset: usize) -> &'a K {
        let mtd = self.get_metadata(offset);
        let ptr = self.chunk_start().add(offset + Self::METADATA_SIZE) as *const ();

        let p: *const K = core::ptr::from_raw_parts(ptr, mtd);
        &*p
    }

    unsafe fn get_metadata(&self, offset: usize) -> <K as Pointee>::Metadata {
        //SAFETY: idk if this works
        let m = self.chunk_start().add(offset);
        let b = m as *const <K as Pointee>::Metadata;
        *b
    }

    // returns the starting offset where the val was written
    unsafe fn write_to_buf<A: Allocator>(&mut self, val: &K, alloc: &A) -> usize {
        let dyn_size = core::mem::size_of_val(val);
        let mtd_size = Self::METADATA_SIZE;

        let total_size = dyn_size + mtd_size + usize::from(NT);

        if self.should_resize(total_size) {
            self.resize(alloc, val)
        }
        let start = self.next_aligned_idx(val);

        let (ptr, mtd) = (val as *const K).to_raw_parts();
        let mtd_ptr = &mtd as *const _ as *const u8;

        //SAFETY: im not too sure about this one.
        let mut write_ptr = self.chunk_start_mut().add(start);
        core::ptr::copy_nonoverlapping(mtd_ptr, write_ptr, Self::METADATA_SIZE);
        write_ptr = write_ptr.add(Self::METADATA_SIZE);
        core::ptr::copy_nonoverlapping(ptr as *const u8, write_ptr, dyn_size);

        if NT {
            write_ptr = write_ptr.add(dyn_size);
            write_ptr.write(0);
        }

        self.offset += total_size;
        start
    }

    fn should_resize(&self, new_size: usize) -> bool {
        new_size > self.remaining_size()
    }

    fn next_aligned_idx(&self, _: &K) -> usize {
        // FIXME: the alignment for trait objects can
        // vary depending on each of the types that are added
        // here. Otherwise, most of their pointers will be unaligned
        self.offset
    }

    fn remaining_size(&self) -> usize {
        self.chunk.map(|(_, c)| c.len() - self.offset).unwrap_or(0)
    }

    fn resize<A: Allocator>(&mut self, alloc: &A, val: &K) {
        let new_layout = self.new_layout(val);

        match self.chunk.as_mut() {
            Some((layout, chunk)) => {
                let ptr: NonNull<u8> = chunk.cast();
                *chunk =
                    unsafe { alloc.grow(ptr, *layout, new_layout) }.expect("could not realloc");
                *layout = new_layout
            }
            _ => {
                let chunk = alloc.allocate(new_layout).expect("could not alloc");
                self.chunk = Some((new_layout, chunk));
            }
        }
    }

    fn chunk_start(&self) -> *const u8 {
        unsafe { self.chunk.unwrap_unchecked().1.as_ptr() as *const u8 }
    }

    fn chunk_start_mut(&mut self) -> *mut u8 {
        unsafe { self.chunk.unwrap_unchecked().1.as_ptr() as *mut u8 }
    }
}

pub struct Interner<K: ?Sized, A: Allocator = Global, const NULL_TERMINATOR: bool = false> {
    builder: DefaultHashBuilder,
    index_table: RawTable<usize, A>,
    buf: StackBuf<K, NULL_TERMINATOR>,
}

impl<K: Eq + Hash + ?Sized> Interner<K> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K: ?Sized, A: Allocator, const NT: bool> Drop for Interner<K, A, NT> {
    fn drop(&mut self) {
        if let Some((layout, chunk)) = self.buf.chunk {
            let alloc = self.alloc();
            //SAFETY: both the chunk and layout are valid
            unsafe { alloc.deallocate(chunk.cast(), layout) }
        }
    }
}

impl<K: Eq + Hash + ?Sized> Default for Interner<K> {
    fn default() -> Self {
        Self::new_in(Global::default())
    }
}

pub struct Interned<'a, K: ?Sized, const NT: bool> {
    offset: usize,
    interner: &'a StackBuf<K, NT>,
}

impl<'a, K: ?Sized, const NT: bool> PartialEq for Interned<'a, K, NT> {
    fn eq(&self, rhs: &Self) -> bool {
        // FIXME: if theres more than one interner
        // this might return the incorrect value
        self.offset == rhs.offset
    }
}

impl<'a, K: ?Sized, const NT: bool> Eq for Interned<'a, K, NT> {}

impl<'a, K: ?Sized, const NT: bool> Interned<'a, K, NT> {
    fn from_bucket(bucket: Bucket<usize>, buf: &'a StackBuf<K, NT>) -> Self {
        Self {
            offset: unsafe { *bucket.as_ref() },
            interner: buf,
        }
    }

    pub fn val(&self) -> &'a K {
        unsafe { self.interner.get_unchecked(self.offset) }
    }
}

//FIXME: this should be sealed in some form or another
pub trait Collection {
    type Output;
}

impl<T> Collection for [T] {
    type Output = T;
}

impl Collection for str {
    type Output = u8;
}

impl<'a, K: ?Sized + Pointee<Metadata = usize> + Collection> Interned<'a, K, true> {
    /// gives C varaint of the collection with a null terminator at the end of the array.
    pub fn as_c_arr(&self) -> *const <K as Collection>::Output {
        let start = self.interner.chunk_start();
        let ptr = unsafe { start.add(self.offset + core::mem::size_of::<usize>()) };
        ptr as *const <K as Collection>::Output
    }
}

impl<K: Eq + Hash + ?Sized, A: Allocator> Interner<K, A> {
    pub fn new_in(alloc: A) -> Self {
        Self {
            builder: DefaultHashBuilder::default(),
            index_table: RawTable::new_in(alloc),
            buf: StackBuf::new(),
        }
    }
}

impl<K: Eq + Hash + ?Sized, A: Allocator, const NT: bool> Interner<K, A, NT> {
    pub fn intern(&mut self, internee: &K) -> Interned<'_, K, NT> {
        let hash = make_hash(&self.builder, internee);
        let hasher = make_hasher::<usize, _>(&self.builder);

        let bucket = match self.index_table.find_or_find_insert_slot(
            hash,
            equivalent_key_from_index(internee, &self.buf),
            hasher,
        ) {
            Ok(bucket) => bucket,
            Err(slot) => {
                let alloc = self.index_table.allocator();
                //SAFETY: verified uniqueness of table entry
                unsafe {
                    let offset = self.buf.write_to_buf(internee, alloc);
                    self.index_table.insert_in_slot(hash, slot, offset)
                }
            }
        };
        Interned::from_bucket(bucket, &self.buf)
    }

    pub fn intern_once(&mut self, internee: &K) -> Option<Interned<'_, K, NT>> {
        let hash = make_hash(&self.builder, internee);
        let hasher = make_hasher::<usize, _>(&self.builder);

        let buf = &self.buf;

        let res = self.index_table.find_or_find_insert_slot(
            hash,
            equivalent_key_from_index(internee, buf),
            hasher,
        );

        match res {
            Ok(_) => None,
            Err(slot) => {
                let alloc = self.index_table.allocator();
                //SAFETY: verified uniqueness of table entry
                let bucket = unsafe {
                    let offset = self.buf.write_to_buf(internee, alloc);
                    self.index_table.insert_in_slot(hash, slot, offset)
                };
                Some(Interned::from_bucket(bucket, &self.buf))
            }
        }
    }
}

impl<K: ?Sized, A: Allocator, const NT: bool> Interner<K, A, NT> {
    pub fn alloc(&self) -> &A {
        self.index_table.allocator()
    }
}

impl<K: Eq + Hash + ?Sized + Pointee<Metadata = usize>, A: Allocator> Interner<K, A> {
    /// used for collection types who would like to interface with C
    /// using the specified allocator
    /// automatically appends a null terminator to the end of a collection.
    pub fn new_in_with_null_delim(alloc: A) -> Interner<K, A, true> {
        Interner {
            builder: DefaultHashBuilder::default(),
            index_table: RawTable::new_in(alloc),
            buf: StackBuf::new(),
        }
    }
}

impl<K: Eq + Hash + ?Sized + Pointee<Metadata = usize>> Interner<K> {
    /// used for collection types who would like to interface with C
    /// automatically appends a null terminator to the end of a collection.
    pub fn new_with_null_delim() -> Interner<K, Global, true> {
        Self::new_in_with_null_delim(Global::default())
    }
}

fn equivalent_key_from_index<'a, K: ?Sized + Equivalent<K>, const NT: bool>(
    k: &'a K,
    buf: &'a StackBuf<K, NT>,
) -> impl Fn(&usize) -> bool + 'a {
    move |offset| {
        // SAFETY: any entry into the map is guaranteed to be a valid index in the stack buf
        let x = unsafe { buf.get_unchecked(*offset) };
        x.equivalent(k)
    }
}
