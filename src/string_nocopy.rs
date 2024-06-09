use crate::nocopy::{Interned, Interner};
use allocator_api2::alloc::{Allocator, Global};

pub struct StringInterner<'a, A: Allocator = Global> {
    inner: Interner<'a, str, A>,
}

#[derive(PartialEq, Eq)]
pub struct IStr<'a> {
    inner: Interned<'a, str>,
}

impl<'a> IStr<'a> {
    pub fn val(&self) -> &'a str {
        self.inner.val()
    }
}

impl<'a> From<Interned<'a, str>> for IStr<'a> {
    fn from(inner: Interned<'a, str>) -> IStr<'a> {
        IStr { inner }
    }
}

impl<'a> StringInterner<'a> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<'a> Default for StringInterner<'a> {
    fn default() -> Self {
        Self::new_in(Global::default())
    }
}

impl<'a, A: Allocator> StringInterner<'a, A> {
    pub fn new_in(alloc: A) -> Self {
        Self {
            inner: Interner::new_in(alloc),
        }
    }

    pub fn intern(&mut self, str: &'a str) -> IStr<'a> {
        self.inner.intern(str).into()
    }

    pub fn intern_once(&mut self, str: &'a str) -> Option<IStr<'a>> {
        self.inner.intern_once(str).map(|i| i.into())
    }
}
