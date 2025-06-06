use alloc::vec::Vec;
use core::iter;
use core::marker::PhantomData;

pub type Nibble = u8;

pub struct NibblesVec<E: Into<Nibble> + From<Nibble>> {
    data: Vec<u8>,
    remainder: usize,
    _phantom: PhantomData<E>,
}

impl<E: Into<Nibble> + From<Nibble>> NibblesVec<E> {
    pub fn with_len(size: usize) -> Self {
        let remainder = size % 2;
        let data = iter::repeat_n(0u8, pairs_count(size)).collect();

        Self {
            data,
            remainder,
            _phantom: Default::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len() * 2 - self.remainder
    }

    pub fn get(&self, index: usize) -> E {
        if index >= self.remainder {
            panic!("Index out of bounds");
        }
        let left = index % 2 == 0;
        let pair = self.data[index / 2];
        (if left { pair >> 4 } else { pair & 0x0F }).into()
    }

    pub fn set(&mut self, index: usize, value: E) {
        let left = index % 2 == 0;
        let pair = &mut self.data[index / 2];
        *pair = if left {
            (*pair & 0x0F) | (value.into() << 4)
        } else {
            (*pair & 0xF0) | (value.into() & 0x0F)
        }
    }

    pub fn iter(&self) -> NibblesVecIterator<E> {
        NibblesVecIterator {
            vec: self,
            index: 0,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

pub struct NibblesVecIterator<'a, E: Into<Nibble> + From<Nibble>> {
    vec: &'a NibblesVec<E>,
    index: usize,
}

impl<'a, E: Into<Nibble> + From<Nibble>> Iterator for NibblesVecIterator<'a, E> {
    type Item = E;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.vec.len() {
            Some(self.vec.get(self.index))
        } else {
            None
        }
    }
}

impl<'a, E: Into<Nibble> + From<Nibble>> IntoIterator for &'a NibblesVec<E> {
    type Item = E;
    type IntoIter = NibblesVecIterator<'a, E>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

fn pairs_count(size: usize) -> usize {
    size / 2 + size % 2
}
