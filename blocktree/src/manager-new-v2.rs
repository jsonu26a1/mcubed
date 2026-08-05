/*
so, I began extending FromBytes and ToBytes to support slices &[T], Vec<T>, and Vec<u8>, which
need to encode their length in bytes before their elements. but this ended up making everything
quite complicated, adding ToBytes::len_as_bytes(), ToBytes becoming a super trait of FromBytes,
and I decided it wasn't worth doing in the end.

it was moving more towards being a serialization framework than just a pair of convenience
traits for reading/writing integers. I also didn't like the recursion, for example FromBytes
could end up attempting to allocate a deeply nested structures in one go. I would rather have
a type where you can explore a serialized blob, inspecting the fields and their lengths without
needing to de-serialize everything in one go. but I don't need to build something like that
right now, if at all.

*/

pub trait FromBytes: ToBytes {
    fn from_bytes(buffer: &[u8]) -> Self;
}

pub trait ToBytes {
    fn to_bytes(&self, buffer: &mut [u8]);
    fn len_as_bytes(&self) -> usize;
}


// since ToBytes is the supertrait of FromBytes, this isn't needed.
// pub trait FromToBytes: FromBytes + ToBytes {}
// impl<T: FromBytes + ToBytes> FromToBytes for T {}

impl<T: ToBytes> ToBytes for &T {
    fn to_bytes(&self, buffer: &mut [u8]) {
        self.to_bytes(buffer);
    }
    fn len_as_bytes(&self) -> usize {
        self.len_as_bytes();
    }
}


// this is wrong; I think we need to include the length?
impl<T: ToBytes> ToBytes for &[T] {
    fn to_bytes(&self, buffer: &mut [u8]) {
        let len = self.len() as u32;
        len.to_bytes(buffer);
        let mut offset = len.len_as_bytes();
        for t in *self {
            t.to_bytes(&mut buffer[offset..]);
            offset += t.len_as_bytes();
        }
    }
    fn len_as_bytes(&self) -> usize {
        todo!();
    }
}

/*
maybe we want a way for from_bytes() to report how many bytes it consumed from the buffer?
or, we could make ToBytes a supertrait of FromBytes
*/

impl FromBytes for Vec<u8> {
    fn from_bytes(buffer: &[u8]) -> Self {
        let len = u32::from_bytes(buffer);
        let loff = len.len_as_bytes();
        buffer[loff..loff + (len as usize)].into()
    }
}

impl ToBytes for Vec<u8> {
    fn to_bytes(&self, buffer: &mut [u8]) {
        let len = self.len() as u32;
        len.to_bytes(buffer);
        let loff = len.len_as_bytes();
        buffer[loff..loff + self.len()].copy_from_slice(self)
    }
    fn len_as_bytes(&self) -> usize {
        let len = self.len() as u32;
        len.len_as_bytes() + self.len()
    }
}

// do we want to impl From/ToBytes for Vec<T>, with a special struct `BytesVec(Vec<u8>)`, or have
// this `struct VecT<T>(Vec<T>)`? I think using VecT<T> is the way to go; if we impl From/ToBytes
// for Vec<T>, it's possible to accidentally call it on Vec<u8>, which would iterate and copy
// each u8, instead of the faster copy_from_slice()

pub struct VecT<T>(pub Vec<T>);

impl<T: FromBytes> FromBytes for VecT<T> {
    fn from_bytes(buffer: &[u8]) -> Self {
        let bytes_len = u32::from_bytes(buffer);
        let mut offset = bytes_len.len_as_bytes();
        let end_offset = offset + (bytes_len as usize);
        let mut out = vec![];
        while offset < end_offset {
            let v = T::from_bytes(&buffer[offset..]);
            offset += v.len_as_bytes();
            out.push(v);
        }
        assert_eq!(offset, end_offset);
        VecT(out)
    }
    // fn from_bytes(buffer: &[u8]) -> Self {
    //     // TODO: this is wrong, it's correct that len is the number of bytes this Vec takes,
    //     // but we can't infer `count` from it, because using `size_of::<T>` is wrong here.
    //     // the number of bytes T takes up is ToBytes::len_as_bytes(), but since we don't have a
    //     // reference to T yet, we stop when `offset` exceeds `len`, which is calcuated by adding
    //     // `ToBytes::len_as_bytes()` from the value returned by each call to `from_bytes()` call.
    //     let len = u32::from_bytes(buffer) as usize;
    //     let count = len / size_of::<T>();
    //     let mut v = Vec::with_capacity(count);
    //     for i in 0..count {
    //         let offset = size_of::<T>() * i;
    //         v.push(buffer[offset..offset + size_of::<u32>()]);
    //     }
    //     VecT(v)
    // }
}

impl<T: ToBytes> ToBytes for VecT<T> {
    fn to_bytes(&self, buffer: &mut [u8]) {
        let count = self.0.len();
        // TODO: use `ToBytes::len_as_bytes()` here
        let full_len = self.len_as_bytes();
        let len = full_len - (full_len as u32).len_as_bytes();
        (len as u32).to_bytes(&mut buffer[0..]);
        let buffer = &mut buffer[size_of::<u32>()..];
        let mut offset = 0;
        for t in self.0.iter() {
            t.to_bytes(&mut buffer[offset..]);
            // TODO: use `ToBytes::len_as_bytes()` here
            offset += t.len_as_bytes();
        }
    }
    fn len_as_bytes(&self) -> usize {
        let
        size_of::<u32>() + self.0.len_as_bytes() * size_of::<T>()
    }
}

// impl<T: FromBytes> FromBytes for Vec<T> {
//     fn from_bytes(buffer: &[u8]) -> Self {
//         todo!();
//     }
// }

macro_rules! impl_numeric_ftb {
    ($($n:ident),+) => {
        $(
            // little-endian makes the most sense here, to be honest.
            impl FromBytes for $n {
                fn from_bytes(buffer: &[u8]) -> Self {
                    Self::from_le_bytes(buffer[0..size_of::<Self>()].try_into().unwrap())
                }
            }
            impl ToBytes for $n {
                fn to_bytes(&self, buffer: &mut [u8]) {
                    buffer[0..size_of::<Self>()].copy_from_slice(self.to_le_bytes().as_slice());
                }
                fn len_as_bytes(&self) -> usize {
                    size_of::<Self>()
                }
            }
        )+
    };
}

// I just realized, we don't want impl for usize or isize, since those aren't portable

impl_numeric_ftb!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

macro_rules! impl_tuple_ftb {
    ($start:ident, $($rem:ident),+) => {
        impl_tuple_ftb!(do_impl $start, $($rem),+);
        impl_tuple_ftb!($($rem),+);
    };
    (do_impl $($n:ident),+) => {
        #[allow(non_snake_case, unused_assignments)]
        impl<$($n: FromBytes),+> FromBytes for ($($n),+ ,) {
            fn from_bytes(buffer: &[u8]) -> Self {
                let mut offset = 0;
                $(
                    let $n = $n::from_bytes(&buffer[offset..]);
                    offset += $n.len_as_bytes();
                )+
                ($($n),+ ,)
            }
        }

        #[allow(non_snake_case, unused_assignments)]
        impl<$($n: ToBytes),+> ToBytes for ($($n),+ ,) {
            fn to_bytes(&self, buffer: &mut [u8]) {
                let ($($n),+ ,) = self;
                let mut offset = 0;
                $(
                    $n.to_bytes(&mut buffer[offset..]);
                    offset += $n.len_as_bytes();
                )+
            }
            fn len_as_bytes(&self) -> usize {
                let ($($n),+ ,) = self;
                let mut len = 0;
                $(
                    len += $n.len_as_bytes();
                )+
                len
            }
        }

    };
    ($start:ident) => {
        impl_tuple_ftb!(do_impl $start);
    };
}

impl_tuple_ftb!(T7, T6, T5, T4, T3, T2, T1, T0);
