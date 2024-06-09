use allocator_api2::alloc::{Allocator, Global};

use crate::copying::{Interned, Interner};

pub struct StringInterner<A: Allocator = Global, const NULL_TERMINATOR: bool = false> {
    inner: Interner<str, A, NULL_TERMINATOR>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self::default()
    }
}

impl StringInterner<Global, true> {
    pub fn new_with_null_terminator() -> Self {
        Self {
            inner: Interner::new_with_null_delim(),
        }
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new_in(Global::default())
    }
}

#[derive(PartialEq, Eq)]
pub struct IStr<'a, const NT: bool> {
    inner: Interned<'a, str, NT>,
}

impl<'a, const NT: bool> IStr<'a, NT> {
    pub fn val(&self) -> &'a str {
        self.inner.val()
    }
}

impl<'a> IStr<'a, true> {
    pub fn as_char_ptr(&self) -> *const u8 {
        self.inner.as_c_arr()
    }
}

impl<'a, const NT: bool> From<Interned<'a, str, NT>> for IStr<'a, NT> {
    fn from(inner: Interned<'a, str, NT>) -> IStr<'a, NT> {
        IStr { inner }
    }
}

impl<A: Allocator> StringInterner<A> {
    pub fn new_in(alloc: A) -> Self {
        Self {
            inner: Interner::new_in(alloc),
        }
    }
}

impl<A: Allocator> StringInterner<A, true> {
    pub fn new_in_with_null_terminator(alloc: A) -> Self {
        Self {
            inner: Interner::new_in_with_null_delim(alloc),
        }
    }
}

impl<A: Allocator, const NT: bool> StringInterner<A, NT> {
    pub fn intern(&mut self, str: &str) -> IStr<'_, NT> {
        self.inner.intern(str).into()
    }

    pub fn intern_once(&mut self, str: &str) -> Option<IStr<'_, NT>> {
        self.inner.intern_once(str).map(|i| i.into())
    }

    pub fn alloc(&self) -> &A {
        self.inner.alloc()
    }
}
