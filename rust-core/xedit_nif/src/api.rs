//! nifly C wrapper API — function pointer types and high-level NIF handle.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

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

// Write/query function pointer types (optional — available when wrapper is rebuilt)
pub(crate) type NiflyCreateFn = unsafe extern "C" fn(game_version: c_int) -> *mut c_void;
pub(crate) type NiflyAddShapeFn = unsafe extern "C" fn(
    handle: *mut c_void,
    name: *const c_char,
    verts: *const f32,
    vert_count: c_int,
    tris: *const u16,
    tri_count: c_int,
    uvs: *const f32,
    normals: *const f32,
) -> c_int;
pub(crate) type NiflySetTextureFn = unsafe extern "C" fn(
    handle: *mut c_void,
    shape_idx: c_int,
    slot: c_int,
    path: *const c_char,
) -> c_int;
pub(crate) type NiflyGetVerticesFn = unsafe extern "C" fn(
    handle: *mut c_void,
    shape_idx: c_int,
    out_buf: *mut f32,
    max_count: c_int,
) -> c_int;
pub(crate) type NiflyGetTrianglesFn = unsafe extern "C" fn(
    handle: *mut c_void,
    shape_idx: c_int,
    out_buf: *mut u16,
    max_count: c_int,
) -> c_int;
pub(crate) type NiflyGetUvsFn = unsafe extern "C" fn(
    handle: *mut c_void,
    shape_idx: c_int,
    out_buf: *mut f32,
    max_count: c_int,
) -> c_int;
pub(crate) type NiflyGetNormalsFn = unsafe extern "C" fn(
    handle: *mut c_void,
    shape_idx: c_int,
    out_buf: *mut f32,
    max_count: c_int,
) -> c_int;
pub(crate) type NiflyGetVertexCountFn = unsafe extern "C" fn(
    handle: *mut c_void,
    shape_idx: c_int,
) -> c_int;
pub(crate) type NiflyGetTriangleCountFn = unsafe extern "C" fn(
    handle: *mut c_void,
    shape_idx: c_int,
) -> c_int;
pub(crate) type NiflySaveFn = unsafe extern "C" fn(
    handle: *mut c_void,
    path: *const c_char,
) -> c_int;

/// Resolved function pointers from the loaded nifly library.
pub(crate) struct NiflyFunctions {
    pub load: NiflyLoadFn,
    pub destroy: NiflyDestroyFn,
    pub get_block_count: NiflyGetBlockCountFn,
    pub get_block_type: NiflyGetBlockTypeFn,
    pub get_shape_count: NiflyGetShapeCountFn,
    pub get_texture_slot: NiflyGetTextureSlotFn,
    // Optional write/query functions (available when wrapper is rebuilt)
    pub create: Option<NiflyCreateFn>,
    pub add_shape: Option<NiflyAddShapeFn>,
    pub set_texture: Option<NiflySetTextureFn>,
    pub get_vertices: Option<NiflyGetVerticesFn>,
    pub get_triangles: Option<NiflyGetTrianglesFn>,
    pub get_uvs: Option<NiflyGetUvsFn>,
    pub get_normals: Option<NiflyGetNormalsFn>,
    pub get_vertex_count: Option<NiflyGetVertexCountFn>,
    pub get_triangle_count: Option<NiflyGetTriangleCountFn>,
    pub save: Option<NiflySaveFn>,
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

    /// Add a shape with geometry data. Returns the shape index.
    ///
    /// `verts` is a flat slice of [x, y, z, ...] (length = vert_count * 3).
    /// `tris` is a flat slice of [i0, i1, i2, ...] (length = tri_count * 3).
    /// `uvs` is an optional flat slice of [u, v, ...] (length = vert_count * 2).
    /// `normals` is an optional flat slice of [x, y, z, ...] (length = vert_count * 3).
    pub fn add_shape(
        &self,
        name: &str,
        verts: &[f32],
        tris: &[u16],
        uvs: Option<&[f32]>,
        normals: Option<&[f32]>,
    ) -> Result<u32, NifError> {
        let func = unsafe { (*self.funcs).add_shape }
            .ok_or_else(|| NifError::OperationFailed("nifly_add_shape not available".into()))?;
        let c_name = CString::new(name)
            .map_err(|_| NifError::InvalidFile("Shape name contains null bytes".into()))?;
        let vert_count = (verts.len() / 3) as c_int;
        let tri_count = (tris.len() / 3) as c_int;
        let uv_ptr = uvs.map_or(std::ptr::null(), |u| u.as_ptr());
        let norm_ptr = normals.map_or(std::ptr::null(), |n| n.as_ptr());
        let ret = unsafe {
            func(
                self.handle,
                c_name.as_ptr(),
                verts.as_ptr(),
                vert_count,
                tris.as_ptr(),
                tri_count,
                uv_ptr,
                norm_ptr,
            )
        };
        if ret < 0 {
            return Err(NifError::OperationFailed("add_shape failed".into()));
        }
        Ok(ret as u32)
    }

    /// Set a texture path for a shape at the given slot.
    ///
    /// Slot indices: 0=diffuse, 1=normal, etc.
    pub fn set_texture(&self, shape_index: u32, slot: u32, path: &str) -> Result<(), NifError> {
        let func = unsafe { (*self.funcs).set_texture }
            .ok_or_else(|| NifError::OperationFailed("nifly_set_texture not available".into()))?;
        let c_path = CString::new(path)
            .map_err(|_| NifError::InvalidFile("Texture path contains null bytes".into()))?;
        let ret = unsafe { func(self.handle, shape_index as c_int, slot as c_int, c_path.as_ptr()) };
        if ret < 0 {
            return Err(NifError::OperationFailed(format!(
                "set_texture({}, {}) failed",
                shape_index, slot
            )));
        }
        Ok(())
    }

    /// Get vertex positions for a shape. Returns a flat vec of [x, y, z, ...].
    pub fn get_vertices(&self, shape_index: u32) -> Result<Vec<f32>, NifError> {
        let func = unsafe { (*self.funcs).get_vertices }
            .ok_or_else(|| NifError::OperationFailed("nifly_get_vertices not available".into()))?;
        // First call to get count
        let count = unsafe { func(self.handle, shape_index as c_int, std::ptr::null_mut(), 0) };
        if count < 0 {
            return Err(NifError::OperationFailed(format!(
                "get_vertices({}) failed",
                shape_index
            )));
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0.0f32; count as usize * 3];
        unsafe { func(self.handle, shape_index as c_int, buf.as_mut_ptr(), count) };
        Ok(buf)
    }

    /// Get triangle indices for a shape. Returns a flat vec of [i0, i1, i2, ...].
    pub fn get_triangles(&self, shape_index: u32) -> Result<Vec<u16>, NifError> {
        let func = unsafe { (*self.funcs).get_triangles }
            .ok_or_else(|| NifError::OperationFailed("nifly_get_triangles not available".into()))?;
        let count = unsafe { func(self.handle, shape_index as c_int, std::ptr::null_mut(), 0) };
        if count < 0 {
            return Err(NifError::OperationFailed(format!(
                "get_triangles({}) failed",
                shape_index
            )));
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0u16; count as usize * 3];
        unsafe { func(self.handle, shape_index as c_int, buf.as_mut_ptr(), count) };
        Ok(buf)
    }

    /// Get UV coordinates for a shape. Returns a flat vec of [u, v, ...].
    pub fn get_uvs(&self, shape_index: u32) -> Result<Vec<f32>, NifError> {
        let func = unsafe { (*self.funcs).get_uvs }
            .ok_or_else(|| NifError::OperationFailed("nifly_get_uvs not available".into()))?;
        let count = unsafe { func(self.handle, shape_index as c_int, std::ptr::null_mut(), 0) };
        if count < 0 {
            return Err(NifError::OperationFailed(format!(
                "get_uvs({}) failed",
                shape_index
            )));
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0.0f32; count as usize * 2];
        unsafe { func(self.handle, shape_index as c_int, buf.as_mut_ptr(), count) };
        Ok(buf)
    }

    /// Get vertex normals for a shape. Returns a flat vec of [x, y, z, ...].
    pub fn get_normals(&self, shape_index: u32) -> Result<Vec<f32>, NifError> {
        let func = unsafe { (*self.funcs).get_normals }
            .ok_or_else(|| NifError::OperationFailed("nifly_get_normals not available".into()))?;
        let count = unsafe { func(self.handle, shape_index as c_int, std::ptr::null_mut(), 0) };
        if count < 0 {
            return Err(NifError::OperationFailed(format!(
                "get_normals({}) failed",
                shape_index
            )));
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0.0f32; count as usize * 3];
        unsafe { func(self.handle, shape_index as c_int, buf.as_mut_ptr(), count) };
        Ok(buf)
    }

    /// Get the vertex count for a shape.
    pub fn vertex_count(&self, shape_index: u32) -> Result<u32, NifError> {
        let func = unsafe { (*self.funcs).get_vertex_count }
            .ok_or_else(|| NifError::OperationFailed("nifly_get_vertex_count not available".into()))?;
        let n = unsafe { func(self.handle, shape_index as c_int) };
        if n < 0 {
            return Err(NifError::OperationFailed(format!(
                "get_vertex_count({}) failed",
                shape_index
            )));
        }
        Ok(n as u32)
    }

    /// Get the triangle count for a shape.
    pub fn triangle_count(&self, shape_index: u32) -> Result<u32, NifError> {
        let func = unsafe { (*self.funcs).get_triangle_count }
            .ok_or_else(|| NifError::OperationFailed("nifly_get_triangle_count not available".into()))?;
        let n = unsafe { func(self.handle, shape_index as c_int) };
        if n < 0 {
            return Err(NifError::OperationFailed(format!(
                "get_triangle_count({}) failed",
                shape_index
            )));
        }
        Ok(n as u32)
    }

    /// Save the NIF to disk.
    pub fn save(&self, path: &Path) -> Result<(), NifError> {
        let func = unsafe { (*self.funcs).save }
            .ok_or_else(|| NifError::OperationFailed("nifly_save not available".into()))?;
        let c_path = CString::new(
            path.to_str()
                .ok_or_else(|| NifError::InvalidFile("Path contains invalid UTF-8".into()))?,
        )
        .map_err(|_| NifError::InvalidFile("Path contains null bytes".into()))?;
        let ret = unsafe { func(self.handle, c_path.as_ptr()) };
        if ret < 0 {
            return Err(NifError::OperationFailed(format!(
                "save({}) failed",
                path.display()
            )));
        }
        Ok(())
    }
}

impl Drop for NifFile {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { ((*self.funcs).destroy)(self.handle) };
        }
    }
}
