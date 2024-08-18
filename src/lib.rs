#![feature(test)]

use std::{
    alloc::{alloc, realloc, Layout},
    ops::{Deref, DerefMut},
};

pub(crate) const INCREMENTAL_CAPACITY: usize = 1024;
pub(crate) const INITIAL_CAPACITY: usize = INCREMENTAL_CAPACITY * 2;

#[derive(Debug)]
pub(crate) struct DArray {
    array: DSlice,
    begin: usize,
    end: usize,
}

impl DArray {
    pub(crate) fn new() -> Self {
        DArray {
            array: DSlice::new(),
            begin: INITIAL_CAPACITY / 2,
            end: INITIAL_CAPACITY / 2,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.end - self.begin
    }

    pub(crate) fn map_index(&self, index: usize) -> usize {
        self.begin + index
    }

    pub(crate) fn get(&mut self, index: usize) -> u64 {
        let len = self.end - self.begin;
        if index >= len {
            panic!("Tried to index outside the array");
        }

        let index = self.map_index(index);
        unsafe { *self.array.get_unchecked(index) }
    }

    pub(crate) fn push_end(&mut self, value: u64) {
        let index = self.end;
        if index == self.array.len() {
            self.array.grow();
        }

        unsafe {
            *self.array.get_unchecked_mut(index) = value;
        }

        self.end += 1;
    }

    pub(crate) fn push_start(&mut self, value: u64) {
        if self.begin == 0 {
            self.array.grow();
            self.array.shift_right(0, self.end, INCREMENTAL_CAPACITY);
            self.begin = INCREMENTAL_CAPACITY;
            self.end += INCREMENTAL_CAPACITY;
        }

        self.begin -= 1;
        unsafe {
            *self.array.get_unchecked_mut(self.begin) = value;
        }
    }

    pub(crate) fn remove(&mut self, index: usize) {
        let index = self.map_index(index);
        self.array.shift_left(index, self.end - index);
        self.end -= 1;
    }

    pub(crate) fn insert(&mut self, index: usize, value: u64) {
        if index == 0 {
            return self.push_start(value);
        }

        let len = self.end - self.begin;

        if index > len {
            panic!("Tried to insert outside the array bounds");
        }

        if self.end == self.array.len() {
            self.array.grow();
        }

        let index = self.map_index(index);
        self.array.shift_right(index, self.end - index, 1);
        unsafe {
            *self.array.get_unchecked_mut(index) = value;
        }

        self.end = self.end + 1;
    }
}

#[derive(Debug)]
pub(crate) struct DSlice(Box<[u64]>);

impl DSlice {
    pub(crate) fn new() -> Self {
        let size = INITIAL_CAPACITY;
        let layout = Layout::array::<u64>(size).expect("Invalid layout");

        // Allocate the memory
        let ptr: *mut u64 = unsafe { alloc(layout).cast() };
        if ptr.is_null() {
            panic!("Memory allocation failed");
        }

        DSlice(unsafe { Box::from_raw(std::slice::from_raw_parts_mut(ptr, size) as *mut [u64]) })
    }

    pub(crate) fn grow(&mut self) {
        let size = self.0.len() + INCREMENTAL_CAPACITY;
        let size_in_bytes = size * 8; // 64 bits = 8 bytes
        let layout = Layout::array::<u64>(self.0.len()).expect("Invalid layout");

        // Reallocate same memory block.
        let ptr: *mut u64 =
            unsafe { realloc(self.0.as_mut_ptr().cast(), layout, size_in_bytes).cast() };
        if ptr.is_null() {
            panic!("Memory re-allocation failed");
        }
        if !ptr.is_aligned() {
            panic!("Memory re-allocation not aligned");
        }

        // Replace the old Box with a new one pointing to the new address.
        let old_box = std::mem::replace(&mut self.0, unsafe {
            Box::from_raw(std::slice::from_raw_parts_mut(ptr, size) as *mut [u64])
        });

        // Leak the old box, so it doesn't try to drop the now invalid address it used to point to.
        Box::leak(old_box);
    }

    pub(crate) fn shift_right(&mut self, offset: usize, count: usize, shift_amount: usize) {
        unsafe {
            let ptr = self.0.as_mut_ptr().offset(offset as isize);
            let dest = ptr.offset(shift_amount as isize);
            std::intrinsics::copy(ptr as *const u64, dest, count);
        }
    }

    pub(crate) fn shift_left(&mut self, offset: usize, count: usize) {
        unsafe {
            let ptr = self.0.as_mut_ptr().offset((offset + 1) as isize);
            let dest = ptr.offset(-1);
            std::intrinsics::copy(ptr as *const u64, dest, count);
        }
    }
}

impl DerefMut for DSlice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

impl Deref for DSlice {
    type Target = [u64];

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut dslice = DSlice::new();
        assert_eq!(dslice.len(), INITIAL_CAPACITY);

        let _ = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
            .iter()
            .enumerate()
            .for_each(|(i, v)| unsafe { *dslice.get_unchecked_mut(i) = *v });

        assert_eq!(dslice.get(0), Some(&1));
        assert_eq!(dslice.get(9), Some(&10));
        assert_eq!(dslice.get(10), Some(&0));

        dslice.grow();
        assert_eq!(dslice.len(), INITIAL_CAPACITY + INCREMENTAL_CAPACITY);

        dslice.shift_right(4, 6, 1);

        assert_eq!(dslice.get(0), Some(&1));
        assert_eq!(dslice.get(4), Some(&5));
        assert_eq!(dslice.get(5), Some(&5));
        assert_eq!(dslice.get(10), Some(&10));
        assert_eq!(dslice.get(11), Some(&0));
    }

    #[test]
    fn test_darray() {
        let mut darray = DArray::new();
        for i in 1..=10 {
            darray.push_end(i);
        }

        assert_eq!(darray.get(0), 1);
        assert_eq!(darray.get(9), 10);

        darray.insert(4, 42);

        assert_eq!(darray.get(4), 42);
        assert_eq!(darray.get(5), 5);
        assert_eq!(darray.get(10), 10);

        darray.insert(0, 42);

        assert_eq!(darray.get(0), 42);
        assert_eq!(darray.get(5), 42);
        assert_eq!(darray.get(11), 10);

        darray.push_start(42);

        assert_eq!(darray.get(0), 42);
        assert_eq!(darray.get(1), 42);
        assert_eq!(darray.get(6), 42);
        assert_eq!(darray.get(12), 10);

        darray.remove(0);

        assert_eq!(darray.get(0), 42);
        assert_eq!(darray.get(5), 42);
        assert_eq!(darray.get(11), 10);

        darray.remove(11);

        assert_eq!(darray.get(0), 42);
        assert_eq!(darray.get(5), 42);
    }

    #[test]
    #[should_panic]
    fn test_darray_out_of_bounds() {
        let mut darray = DArray::new();
        for i in 1..=10 {
            darray.push_end(i);
        }

        darray.get(10);
    }

    #[test]
    #[should_panic]
    fn test_darray_remove_moves_bounds() {
        let mut darray = DArray::new();
        for i in 1..=10 {
            darray.push_end(i);
        }

        darray.remove(0);

        darray.get(9);
    }

    extern crate test;
    use test::Bencher;

    #[bench]
    fn bench_prepends(b: &mut Bencher) {
        b.iter(|| {
            let mut darray = DArray::new();
            for i in 0..=200000 {
                darray.push_start(i);
            }
        })
    }

    #[bench]
    fn bench_appends(b: &mut Bencher) {
        b.iter(|| {
            let mut darray = DArray::new();
            for i in 0..=200000 {
                darray.push_end(i);
            }
        })
    }

    #[bench]
    fn bench_mid_inserts(b: &mut Bencher) {
        b.iter(|| {
            let mut darray = DArray::new();
            for i in 0..=2000 {
                darray.insert(darray.len() / 2, i);
            }
        })
    }
}
