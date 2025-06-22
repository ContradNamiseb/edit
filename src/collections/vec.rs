use std::alloc::Allocator;
use std::borrow::Borrow;
use std::marker::PhantomData;
use std::ops::{Bound, Deref, DerefMut, Range, RangeBounds};
use std::ptr::{self, NonNull};

/// [`Vec<T>`] but specialized for "Micosoft Edit" (ME = Me).
/// Features performance optimizations (TODO: ...and allocator support in stable Rust).
pub struct MeVec<T, A: Allocator = std::alloc::Global> {
    ptr: NonNull<T>,
    cap: usize,
    len: usize,

    alloc: A,
    _marker: PhantomData<T>,
}

impl<T> MeVec<T> {
    /// Creates a new empty `MeVec<T>`.
    pub const fn new() -> Self {
        MeVec {
            ptr: NonNull::dangling(),
            cap: 0,
            len: 0,
            alloc: std::alloc::Global,
            _marker: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let ptr = unsafe {
            let layout = std::alloc::Layout::array::<T>(cap).unwrap();
            NonNull::new(std::alloc::alloc(layout)).expect("Failed to allocate memory").cast()
        };
        Self { ptr, cap, len: 0, alloc: std::alloc::Global, _marker: PhantomData }
    }

    pub fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self
    where
        T: Clone,
    {
        let mut vec = Self::new();
        for item in iter {
            vec.push(item);
        }
        vec
    }

    pub fn new_repeated(value: T, count: usize) -> Self
    where
        T: Clone,
    {
        let mut vec = Self::new();
        vec.reserve(count);
        unsafe {
            let ptr: *mut T = vec.as_mut_ptr();
            for i in 0..count {
                ptr::write(ptr.add(i), value.clone());
            }
            vec.set_len(count);
        }
        vec
    }
}

impl<T, A: Allocator> MeVec<T, A> {
    pub const fn new_in(alloc: A) -> Self {
        MeVec { ptr: NonNull::dangling(), cap: 0, len: 0, alloc, _marker: PhantomData }
    }

    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        let cap = capacity.max(1);
        let ptr = unsafe {
            let layout = std::alloc::Layout::array::<T>(cap).unwrap();
            NonNull::new(std::alloc::alloc(layout)).expect("Failed to allocate memory").cast()
        };
        Self { ptr, cap, len: 0, alloc, _marker: PhantomData }
    }

    pub const fn allocator(&self) -> &A {
        &self.alloc
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.cap
    }

    /// Returns the number of elements in the vector.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the vector is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    pub fn reserve_exact(&mut self, additional: usize) {
        if self.len + additional > self.cap {
            let new_cap = (self.len + additional).next_power_of_two();
            let new_ptr = unsafe {
                let layout = std::alloc::Layout::array::<T>(new_cap).unwrap();
                NonNull::new(std::alloc::alloc(layout)).expect("Failed to allocate memory").cast()
            };
            if !self.ptr.as_ptr().is_null() {
                unsafe {
                    std::ptr::copy_nonoverlapping(self.ptr.as_ptr(), new_ptr.as_ptr(), self.len);
                    std::alloc::dealloc(
                        self.ptr.as_ptr() as *mut u8,
                        std::alloc::Layout::array::<T>(self.cap).unwrap(),
                    );
                }
            }
            self.ptr = new_ptr;
            self.cap = new_cap;
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        if self.len + additional > self.cap {
            let new_cap = (self.len + additional).next_power_of_two();
            let new_ptr = unsafe {
                let layout = std::alloc::Layout::array::<T>(new_cap).unwrap();
                NonNull::new(std::alloc::alloc(layout)).expect("Failed to allocate memory").cast()
            };
            if !self.ptr.as_ptr().is_null() {
                unsafe {
                    std::ptr::copy_nonoverlapping(self.ptr.as_ptr(), new_ptr.as_ptr(), self.len);
                    std::alloc::dealloc(
                        self.ptr.as_ptr() as *mut u8,
                        std::alloc::Layout::array::<T>(self.cap).unwrap(),
                    );
                }
            }
            self.ptr = new_ptr;
            self.cap = new_cap;
        }
    }

    pub fn shrink_to_fit(&mut self) {
        if self.len < self.cap {
            let new_cap = self.len.max(1);
            if new_cap < self.cap {
                let new_ptr = unsafe {
                    let layout = std::alloc::Layout::array::<T>(new_cap).unwrap();
                    NonNull::new(std::alloc::realloc(
                        self.ptr.as_ptr() as *mut u8,
                        layout,
                        new_cap * std::mem::size_of::<T>(),
                    ))
                    .expect("Failed to reallocate memory")
                    .cast()
                };
                self.ptr = new_ptr;
                self.cap = new_cap;
            }
        }
    }

    pub fn clear(&mut self) {
        if !self.ptr.as_ptr().is_null() {
            unsafe {
                std::ptr::drop_in_place(self.ptr.as_ptr());
                self.len = 0;
            }
        }
    }

    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
        T: Clone,
    {
        let mut iter = iter.into_iter();
        if let Some(first) = iter.next() {
            self.push(first);
            for item in iter {
                self.push(item);
            }
        }
    }

    pub fn extend_from_within<R: RangeBounds<usize>>(&mut self, range: R)
    where
        T: Clone,
    {
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(start) => start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(end) => end + 1,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.len,
        };
        if start < end && end <= self.len {
            let slice =
                unsafe { std::slice::from_raw_parts(self.ptr.as_ptr().add(start), end - start) };
            self.extend_from_slice(slice);
        }
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        if self.len == 0 {
            return;
        }

        let mut write_index = 0;
        for i in 0..self.len {
            let item = unsafe { &*self.ptr.as_ptr().add(i) };
            if f(item) {
                if write_index != i {
                    unsafe {
                        ptr::copy_nonoverlapping(item, self.ptr.as_ptr().add(write_index), 1);
                    }
                }
                write_index += 1;
            }
        }
        unsafe { self.set_len(write_index) };
    }

    pub fn push(&mut self, value: T) -> &mut T {
        if self.len == self.cap {
            self.reserve(1);
        }
        unsafe {
            let ptr = self.as_mut_ptr().add(self.len);
            ptr::write(ptr, value);
            self.set_len(self.len + 1);
            &mut *ptr
        }
    }

    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len {
            unsafe {
                let ptr = self.as_mut_ptr().add(new_len);
                for i in new_len..self.len {
                    ptr::drop_in_place(ptr.add(i));
                }
            }
            unsafe { self.set_len(new_len) };
        }
    }

    pub fn resize(&mut self, new_len: usize, value: T)
    where
        T: Copy,
    {
        if new_len > self.cap {
            self.reserve(new_len - self.len);
        }
        if new_len > self.len {
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len);
                for i in 0..(new_len - self.len) {
                    ptr::write(ptr.add(i), value);
                }
            }
        }
        unsafe { self.set_len(new_len) };
    }

    pub fn extend_from_slice(&mut self, src: &[T])
    where
        T: Clone,
    {
        let src_len = src.len();
        if src_len > 0 {
            self.reserve(src_len);
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len);
                ptr::copy_nonoverlapping(src.as_ptr(), ptr, src_len);
                self.set_len(self.len + src_len);
            }
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr.as_ptr()
    }

    pub unsafe fn set_len(&mut self, new_len: usize) {
        debug_assert!(new_len <= self.cap);
        self.len = new_len;
    }

    pub fn spare_capacity_mut(&mut self) -> &mut [T] {
        if self.len < self.cap {
            unsafe {
                std::slice::from_raw_parts_mut(self.ptr.as_ptr().add(self.len), self.cap - self.len)
            }
        } else {
            &mut []
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &'_ T> {
        self.deref().iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &'_ mut T> {
        self.deref_mut().iter_mut()
    }

    pub fn leak(self) -> &'static mut [T] {
        let slice = unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) };
        // Prevent the destructor from running.
        std::mem::forget(self);
        slice
    }
}

impl<T: Copy, A: Allocator> MeVec<T, A> {
    pub fn replace_range<R: RangeBounds<usize>>(&mut self, range: R, src: &[T]) {
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(start) => start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(end) => end + 1,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => usize::MAX,
        };
        self.replace_impl(start..end, src);
    }

    pub fn replace_impl(&mut self, range: Range<usize>, src: &[T]) {
        unsafe {
            let dst_len = self.len();
            let src_len = src.len();
            let off = range.start.min(dst_len);
            let del_len = range.end.saturating_sub(off).min(dst_len - off);

            if del_len == 0 && src_len == 0 {
                return; // nothing to do
            }

            let tail_len = dst_len - off - del_len;
            let new_len = dst_len - del_len + src_len;

            if src_len > del_len {
                self.reserve(src_len - del_len);
            }

            // NOTE: drop_in_place() is not needed here, because T is constrained to Copy.

            // SAFETY: as_mut_ptr() must called after reserve() to ensure that the pointer is valid.
            let ptr = self.as_mut_ptr().add(off);

            // Shift the tail.
            if tail_len > 0 && src_len != del_len {
                ptr::copy(ptr.add(del_len), ptr.add(src_len), tail_len);
            }

            // Copy in the replacement.
            ptr::copy_nonoverlapping(src.as_ptr(), ptr, src_len);
            self.set_len(new_len);
        }
    }
}

impl<T> Default for MeVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, A: Allocator> Drop for MeVec<T, A> {
    fn drop(&mut self) {
        if self.ptr != NonNull::dangling() {
            unsafe {
                std::alloc::dealloc(
                    self.ptr.as_ptr() as *mut u8,
                    std::alloc::Layout::array::<u8>(self.cap).unwrap(),
                );
            }
        }
    }
}

impl<T, A: Allocator> Deref for MeVec<T, A> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl<T, A: Allocator> DerefMut for MeVec<T, A> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl<T, A: Allocator> AsRef<[T]> for MeVec<T, A> {
    fn as_ref(&self) -> &[T] {
        self.deref()
    }
}

impl<T, A: Allocator> AsMut<[T]> for MeVec<T, A> {
    fn as_mut(&mut self) -> &mut [T] {
        self.deref_mut()
    }
}

impl<T, A: Allocator> Borrow<[T]> for MeVec<T, A> {
    fn borrow(&self) -> &[T] {
        self.deref()
    }
}

impl<T, A: Allocator + Clone> Clone for MeVec<T, A> {
    fn clone(&self) -> Self {
        let mut new_vec = Self::new_in(self.alloc.clone());
        new_vec.reserve(self.len);
        unsafe {
            ptr::copy_nonoverlapping(self.ptr.as_ptr(), new_vec.as_mut_ptr(), self.len);
            new_vec.set_len(self.len);
        }
        new_vec
    }
}
