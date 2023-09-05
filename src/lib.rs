//! Compact trait object vector storage.

#![no_std]

extern crate alloc;

use core::{iter, mem, ops, slice, ptr};

use alloc::vec::Vec;

const MAX_ALIGN: usize = 8;
const LOG_MAX_ALIGN: usize = MAX_ALIGN.trailing_zeros() as usize;

pub struct DstVec<Trait: ?Sized + AnyTrait> {
    buffers: [Vec<u8>; LOG_MAX_ALIGN + 1],

    buffer_ids: Vec<u8>,
    buffer_ranges: Vec<ops::Range<usize>>,
    ref_converters: Vec<fn(&[u8]) -> &Trait>, // TODO: change buffers to UnsafeCell so that we can reuse mut_converters
    mut_converters: Vec<fn(&mut [u8]) -> &mut Trait>,
}

impl<Trait: ?Sized + AnyTrait> Default for DstVec<Trait> {
    fn default() -> Self {
        Self {
            buffers: Default::default(),
            buffer_ids: Vec::new(),
            buffer_ranges: Vec::new(),
            ref_converters: Vec::new(),
            mut_converters: Vec::new(),
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
        let _ref_converters = mem::replace(&mut self.ref_converters, Vec::new());
        let mut_converters = mem::replace(&mut self.mut_converters, Vec::new());

        for ((&buffer_id, buffer_range), mut_converters) in iter::zip(
            iter::zip(buffer_ids.iter(), buffer_ranges.iter()),
            mut_converters.iter(),
        ) {
            let bytes = &mut self.buffers[buffer_id as usize][buffer_range.clone()];
            let object = mut_converters(bytes);
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
        self.ref_converters.push(|bytes| {
            let impl_ref = unsafe { &*(bytes.as_ptr() as *const Impl) };
            impl_ref.upcast_ref()
        });
        self.mut_converters.push(|bytes| {
            let impl_ref = unsafe { &mut *(bytes.as_mut_ptr() as *mut Impl) };
            impl_ref.upcast_mut()
        });
    }

    pub fn get(&self, index: usize) -> Option<&Trait> {
        let &buffer_id = self.buffer_ids.get(index)?;
        let buffer_range = self.buffer_ranges.get(index)?;
        let converter = self.ref_converters.get(index)?;
        Some(converter(&self.buffers[buffer_id as usize][buffer_range.clone()]))
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Trait> {
        let &buffer_id = self.buffer_ids.get(index)?;
        let buffer_range = self.buffer_ranges.get(index)?;
        let converter = self.mut_converters.get(index)?;
        Some(converter(&mut self.buffers[buffer_id as usize][buffer_range.clone()]))
    }
}

pub trait AnyTrait: 'static {
    // TODO; remove the 'static bound
    fn align(&self) -> usize;

    fn raw_bytes(&self) -> &[u8];
}

impl<Impl: 'static> AnyTrait for Impl {
    fn align(&self) -> usize {
        mem::align_of::<Self>()
    }

    fn raw_bytes(&self) -> &[u8] {
        let size = mem::size_of::<Impl>();
        let ptr = self as *const Impl as *const u8;
        unsafe {
            // Safety:
            // 1. `ptr` up to `size` bytes are all the same allocated object for `Impl`.
            // 2. All values are properly initialized for `u8`.
            // 3. We hold a shared reference to `self`, so no mutation may occur.
            // 4. Wrapping around address space is impossible as it is a single allocated object.
            slice::from_raw_parts(ptr, size)
        }
    }
}

pub unsafe trait TraitImpl<Trait: ?Sized + AnyTrait>: Sized + AnyTrait {
    fn upcast_ref(&self) -> &Trait;
    fn upcast_mut(&mut self) -> &mut Trait;
}
