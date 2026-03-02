//! nifly C wrapper API — function pointer types and high-level NIF handle.

use std::ffi::{c_char, c_int, c_void, CStr};

use crate::NifError;

/// Function pointer types matching `xedit_nifly_wrapper.h`.
pub(crate) type NiflyLoadFn = unsafe extern "C" fn(path: *const c_char) -> *mut c_void;
pub(crate) type NiflyDestroyFn = unsafe extern "C" fn(handle: *mut c_void);
pub(crate) type NiflyGetBlockCountFn = unsafe extern "C" fn(handle: *mut c_void) -> c_int;
pub(crate) type NiflyGetBlockTypeFn =
    unsafe extern "C" fn(handle: *mut c_void, index: c_int, buf: *mut c_char, buflen: c_int) -> c_int;
pub(crate) type NiflyGetShapeCountFn = unsafe extern "C" fn(handle: *mut c_void) -> c_int;
pub(crate) type NiflyGetTextureSlotFn = unsafe extern "C" fn(
    handle: *mut c_void,
    shape_index: c_int,
    slot: c_int,
    buf: *mut c_char,
    buflen: c_int,
) -> c_int;

/// Resolved function pointers from the loaded nifly library.
pub(crate) struct NiflyFunctions {
    pub load: NiflyLoadFn,
    pub destroy: NiflyDestroyFn,
    pub get_block_count: NiflyGetBlockCountFn,
    pub get_block_type: NiflyGetBlockTypeFn,
    pub get_shape_count: NiflyGetShapeCountFn,
    pub get_texture_slot: NiflyGetTextureSlotFn,
}

/// A loaded NIF file handle, wrapping the opaque pointer from nifly.
///
/// Automatically calls `nifly_destroy` on drop.
pub struct NifFile {
    handle: *mut c_void,
    funcs: *const NiflyFunctions,
}

// NifFile is Send because the opaque pointer is only accessed through
// the C API which manages its own thread safety, and we don't share
// the handle across threads without synchronization.
unsafe impl Send for NifFile {}

impl NifFile {
    /// Create a NifFile from a raw handle and function table pointer.
    ///
    /// # Safety
    /// `handle` must be a valid pointer returned by `nifly_load`.
    /// `funcs` must point to a valid `NiflyFunctions` that outlives this `NifFile`.
    pub(crate) unsafe fn from_raw(handle: *mut c_void, funcs: *const NiflyFunctions) -> Self {
        Self { handle, funcs }
    }

    /// Get the number of blocks in the NIF header.
    pub fn block_count(&self) -> Result<u32, NifError> {
        let n = unsafe { ((*self.funcs).get_block_count)(self.handle) };
        if n < 0 {
            return Err(NifError::OperationFailed("get_block_count returned -1".into()));
        }
        Ok(n as u32)
    }

    /// Get the block type name (e.g. "NiNode", "BSFadeNode") for a given index.
    pub fn block_type(&self, index: u32) -> Result<String, NifError> {
        let mut buf = vec![0u8; 256];
        let len = unsafe {
            ((*self.funcs).get_block_type)(
                self.handle,
                index as c_int,
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as c_int,
            )
        };
        if len < 0 {
            return Err(NifError::OperationFailed(format!(
                "get_block_type({}) returned -1",
                index
            )));
        }
        let cstr = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
        Ok(cstr.to_string_lossy().into_owned())
    }

    /// Get the number of shapes in the NIF.
    pub fn shape_count(&self) -> Result<u32, NifError> {
        let n = unsafe { ((*self.funcs).get_shape_count)(self.handle) };
        if n < 0 {
            return Err(NifError::OperationFailed("get_shape_count returned -1".into()));
        }
        Ok(n as u32)
    }

    /// Get the texture path for a given shape and texture slot.
    ///
    /// Slot indices: 0=diffuse, 1=normal, 2=glow, etc. (game-dependent).
    /// Returns `None` if no texture is assigned to that slot.
    pub fn texture_slot(&self, shape_index: u32, slot: u32) -> Result<Option<String>, NifError> {
        let mut buf = vec![0u8; 512];
        let len = unsafe {
            ((*self.funcs).get_texture_slot)(
                self.handle,
                shape_index as c_int,
                slot as c_int,
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as c_int,
            )
        };
        if len < 0 {
            return Err(NifError::OperationFailed(format!(
                "get_texture_slot({}, {}) returned -1",
                shape_index, slot
            )));
        }
        if len == 0 {
            return Ok(None);
        }
        let cstr = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
        Ok(Some(cstr.to_string_lossy().into_owned()))
    }
}

impl Drop for NifFile {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { ((*self.funcs).destroy)(self.handle) };
        }
    }
}
