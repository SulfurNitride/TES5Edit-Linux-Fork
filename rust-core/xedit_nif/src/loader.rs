//! Runtime loading of the nifly C wrapper shared library.

use std::ffi::CString;
use std::path::Path;

use crate::api::{NifFile, NiflyFunctions};
use crate::NifError;

/// Handle to the loaded nifly shared library.
///
/// This is loaded once at startup and kept alive for the entire session.
/// If nifly cannot be loaded, the application must not start.
pub struct NiflyLibrary {
    _lib: libloading::Library,
    funcs: Box<NiflyFunctions>,
}

impl NiflyLibrary {
    /// Attempt to load the nifly C wrapper from standard search paths.
    ///
    /// Search order:
    /// 1. Same directory as the executable
    /// 2. System library paths (LD_LIBRARY_PATH on Linux, PATH on Windows)
    /// 3. Explicit path if provided
    pub fn load() -> Result<Self, NifError> {
        let lib_name = if cfg!(target_os = "windows") {
            "nifly_wrapper.dll"
        } else {
            "libnifly_wrapper.so"
        };

        // Try next to the executable first
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join(lib_name);
                if candidate.exists() {
                    return Self::load_from(candidate.to_string_lossy().as_ref());
                }
            }
        }

        Self::load_from(lib_name)
    }

    /// Load from a specific path or library name.
    pub fn load_from(path: impl AsRef<str>) -> Result<Self, NifError> {
        let path_str = path.as_ref();

        let lib = unsafe {
            libloading::Library::new(path_str).map_err(|e| {
                NifError::LoadFailed(format!("Failed to load '{}': {}", path_str, e))
            })?
        };

        // Resolve all function pointers
        let funcs = unsafe {
            // Required symbols (original read-only API)
            let load = *lib
                .get::<crate::api::NiflyLoadFn>(b"nifly_load\0")
                .map_err(|e| NifError::MissingSymbol(format!("nifly_load: {}", e)))?;
            let destroy = *lib
                .get::<crate::api::NiflyDestroyFn>(b"nifly_destroy\0")
                .map_err(|e| NifError::MissingSymbol(format!("nifly_destroy: {}", e)))?;
            let get_block_count = *lib
                .get::<crate::api::NiflyGetBlockCountFn>(b"nifly_get_block_count\0")
                .map_err(|e| NifError::MissingSymbol(format!("nifly_get_block_count: {}", e)))?;
            let get_block_type = *lib
                .get::<crate::api::NiflyGetBlockTypeFn>(b"nifly_get_block_type\0")
                .map_err(|e| NifError::MissingSymbol(format!("nifly_get_block_type: {}", e)))?;
            let get_shape_count = *lib
                .get::<crate::api::NiflyGetShapeCountFn>(b"nifly_get_shape_count\0")
                .map_err(|e| NifError::MissingSymbol(format!("nifly_get_shape_count: {}", e)))?;
            let get_texture_slot = *lib
                .get::<crate::api::NiflyGetTextureSlotFn>(b"nifly_get_texture_slot\0")
                .map_err(|e| NifError::MissingSymbol(format!("nifly_get_texture_slot: {}", e)))?;

            // Optional symbols (write/query API — available when wrapper is rebuilt)
            let create = lib.get::<crate::api::NiflyCreateFn>(b"nifly_create\0").ok().map(|s| *s);
            let add_shape = lib.get::<crate::api::NiflyAddShapeFn>(b"nifly_add_shape\0").ok().map(|s| *s);
            let set_texture = lib.get::<crate::api::NiflySetTextureFn>(b"nifly_set_texture\0").ok().map(|s| *s);
            let get_vertices = lib.get::<crate::api::NiflyGetVerticesFn>(b"nifly_get_vertices\0").ok().map(|s| *s);
            let get_triangles = lib.get::<crate::api::NiflyGetTrianglesFn>(b"nifly_get_triangles\0").ok().map(|s| *s);
            let get_uvs = lib.get::<crate::api::NiflyGetUvsFn>(b"nifly_get_uvs\0").ok().map(|s| *s);
            let get_normals = lib.get::<crate::api::NiflyGetNormalsFn>(b"nifly_get_normals\0").ok().map(|s| *s);
            let get_vertex_count = lib.get::<crate::api::NiflyGetVertexCountFn>(b"nifly_get_vertex_count\0").ok().map(|s| *s);
            let get_triangle_count = lib.get::<crate::api::NiflyGetTriangleCountFn>(b"nifly_get_triangle_count\0").ok().map(|s| *s);
            let save = lib.get::<crate::api::NiflySaveFn>(b"nifly_save\0").ok().map(|s| *s);

            Box::new(NiflyFunctions {
                load,
                destroy,
                get_block_count,
                get_block_type,
                get_shape_count,
                get_texture_slot,
                create,
                add_shape,
                set_texture,
                get_vertices,
                get_triangles,
                get_uvs,
                get_normals,
                get_vertex_count,
                get_triangle_count,
                save,
            })
        };

        tracing::info!("nifly library loaded successfully from: {}", path_str);

        Ok(Self { _lib: lib, funcs })
    }

    /// Create a new empty NIF file for a given game version.
    ///
    /// Game versions: 0=Oblivion, 1=Fallout3/FNV, 2=SkyrimLE, 3=SkyrimSE, 4=Fallout4, 5=Starfield.
    pub fn create_nif(&self, game_version: u32) -> Result<NifFile, NifError> {
        let create_fn = self
            .funcs
            .create
            .ok_or_else(|| NifError::OperationFailed("nifly_create not available".into()))?;

        let handle = unsafe { create_fn(game_version as std::ffi::c_int) };
        if handle.is_null() {
            return Err(NifError::OperationFailed(format!(
                "nifly_create({}) returned null",
                game_version
            )));
        }

        Ok(unsafe { NifFile::from_raw(handle, &*self.funcs as *const NiflyFunctions) })
    }

    /// Load a NIF file using the nifly library.
    pub fn load_nif(&self, path: &Path) -> Result<NifFile, NifError> {
        let path_cstr = CString::new(
            path.to_str()
                .ok_or_else(|| NifError::InvalidFile("Path contains invalid UTF-8".into()))?,
        )
        .map_err(|_| NifError::InvalidFile("Path contains null bytes".into()))?;

        let handle = unsafe { (self.funcs.load)(path_cstr.as_ptr()) };
        if handle.is_null() {
            return Err(NifError::InvalidFile(format!(
                "nifly failed to load: {}",
                path.display()
            )));
        }

        Ok(unsafe { NifFile::from_raw(handle, &*self.funcs as *const NiflyFunctions) })
    }

    /// Check if nifly is available at the expected path.
    pub fn is_available() -> bool {
        let lib_name = if cfg!(target_os = "windows") {
            "nifly_wrapper.dll"
        } else {
            "libnifly_wrapper.so"
        };

        Path::new(lib_name).exists()
            || std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join(lib_name).exists()))
                .unwrap_or(false)
    }
}
