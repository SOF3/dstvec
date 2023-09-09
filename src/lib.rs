//! Compact trait object vector storage.

#![no_std]

#![feature(slice_ptr_get)]

extern crate alloc;

use core::{iter, mem::{self, MaybeUninit}, ops, slice, ptr};

use alloc::vec::Vec;

const MAX_ALIGN: usize = 8;
const LOG_MAX_ALIGN: usize = MAX_ALIGN.trailing_zeros() as usize;

pub struct DstVec<Trait: ?Sized + AnyTrait> {
    buffers: [Vec<MaybeUninit<u8>>; LOG_MAX_ALIGN + 1],

    buffer_ids: Vec<u8>,
    buffer_ranges: Vec<ops::Range<usize>>,
    converters: Vec<fn(*mut [MaybeUninit<u8>]) -> *mut Trait>, // TODO: change buffers to UnsafeCell so that we can reuse mut_converters
}

impl<Trait: ?Sized + AnyTrait> Default for DstVec<Trait> {
    fn default() -> Self {
        Self {
            buffers: Default::default(),
            buffer_ids: Vec::new(),
            buffer_ranges: Vec::new(),
            converters: Vec::new(),
        }
    }
}

impl<Trait: ?Sized + AnyTrait> Drop for DstVec<Trait> {
    fn drop(&mut self) {
        if !mem::needs_drop::<Trait>() {
            return;
        }

        // swap out buffers to avoid poisoned state caused by panics during drop
        let buffer_ids = mem::replace(&mut self.buffer_ids, Vec::new());
        let buffer_ranges = mem::replace(&mut self.buffer_ranges, Vec::new());
        let converters = mem::replace(&mut self.converters, Vec::new());

        for ((&buffer_id, buffer_range), converter) in iter::zip(
            iter::zip(buffer_ids.iter(), buffer_ranges.iter()),
            converters.iter(),
        ) {
            let bytes = &mut self.buffers[buffer_id as usize][buffer_range.clone()];
            let object = converter(bytes as *mut [MaybeUninit<u8>]);
            unsafe { ptr::drop_in_place(object) };
        }
    }
}

impl<Trait: ?Sized + AnyTrait> DstVec<Trait> {
    pub fn push<Impl: TraitImpl<Trait>>(&mut self, obj: Impl) {
        let obj = mem::ManuallyDrop::new(obj);

        let align = obj.align();
        let buffer_id = align.trailing_zeros();

        let buffer_range = {
            let buffer = &mut self.buffers[buffer_id as usize];
            let buffer_offset = buffer.len();
            buffer.extend(obj.raw_bytes());
            buffer_offset..buffer.len()
        };

        self.buffer_ids.push(buffer_id as u8);
        self.buffer_ranges.push(buffer_range);
        self.converters.push(|bytes| {
            // Safety: this function is only safe to call if `bytes` is safe to be read
            let slice_ptr = bytes.as_mut_ptr();
            let impl_ref = slice_ptr as *mut Impl;
            unsafe {
                // Safety: impl_cell is expected to contain valid value of Impl,
                // and the cell is read-safe.
                Impl::upcast(impl_ref)
            }
        });
    }

    pub fn get(&self, index: usize) -> Option<&Trait> {
        let &buffer_id = self.buffer_ids.get(index)?;
        let buffer_range = self.buffer_ranges.get(index)?;
        let converter = self.converters.get(index)?;
        let slice = &self.buffers[buffer_id as usize][buffer_range.clone()];
        let trait_ptr = converter(slice as *const [MaybeUninit<u8>] as *mut _);
        Some(unsafe {
            // Safety: the pointer was derived from a shared reference that is still in scope.
            &*trait_ptr
        })
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Trait> {
        let &buffer_id = self.buffer_ids.get(index)?;
        let buffer_range = self.buffer_ranges.get(index)?;
        let converter = self.converters.get(index)?;
        let slice = &mut self.buffers[buffer_id as usize][buffer_range.clone()];
        let trait_ptr = converter(slice as *mut [MaybeUninit<u8>]);
        Some(unsafe {
            // Safety: this is the only place who can access the cell,
            // and the borrow is derived from a mutable borrow.
            &mut *trait_ptr
        })
    }
}

pub trait AnyTrait: 'static {
    // TODO; remove the 'static bound
    fn align(&self) -> usize;

    fn raw_bytes(&self) -> &[MaybeUninit<u8>];
}

impl<Impl: 'static> AnyTrait for Impl {
    fn align(&self) -> usize {
        mem::align_of::<Self>()
    }

    fn raw_bytes(&self) -> &[MaybeUninit<u8>] {
        let size = mem::size_of::<Impl>();
        let ptr = self as *const Impl as *const MaybeUninit<u8>;
        unsafe {
            // Safety:
            // 1. `ptr` up to `size` bytes are all the same allocated object for `Impl`.
            // 2. All values are properly initialized for `MaybeUninit<u8>`.
            // 3. We hold a shared reference to `self`, so no mutation may occur.
            // 4. Wrapping around address space is impossible as it is a single allocated object.
            slice::from_raw_parts(ptr, size)
        }
    }
}

pub unsafe trait TraitImpl<Trait: ?Sized + AnyTrait>: Sized + AnyTrait {
    unsafe fn upcast(this: *mut Self) -> *mut Trait;
}
