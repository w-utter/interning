fast, generic type interning.

in computer science, interning is re-using objects of equal value instead of creating new objects. this allows for: 

* extremely fast assignment and comparisons.

* fast ffi - if collections are copied on interning, they have the option to append a null byte, which allows them to be passed to c directly.

* optional copyless interning - use rust's type system to enable unique interning without having to copy the value.

* no mutex by default - use interners in whatever context suits you best.

note that values in the interner are only freed once the interner is dropped. it is not possible to remove previously interned values without invalidating references to the interned objects.

## usage

```rust
use interning::string_nocopy::{stringinterner, istr};

// creation is quick and easy
let s1 = String::from("string");
let s2 = "string";

let mut interner = StringInterner::new();

let i1 = interner.intern(&h1);
let i2 = interner.intern(h2);


// comparisons and copies are extremely cheap
let i3 = i1;
assert_eq!(i2, i3);
```

## motivation
all existing interning crates in rust either have slow interning cycles (string-interner / string-cache), or can only be used in a static context (ustr), do not support the usage of a non-global allocator, and aren't generic over what they can intern. I also wanted to see if it was possible to make lookup quicker by using a RawTable for a hashmap implementation and avoiding as much copying as possible.

## safety and compatibility

there is still a lot of testing and ci that needs to be added into this project as it is relatively new. moreover, the copying interner is much less tested than the copyless interner as that is what i primarily use.
