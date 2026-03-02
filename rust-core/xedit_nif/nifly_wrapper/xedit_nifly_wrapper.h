/*
 * xedit_nifly_wrapper.h - Minimal C wrapper around nifly for xEdit/Rust FFI
 *
 * This provides a stable C ABI for loading NIF files and querying basic
 * block/shape/texture information via nifly.
 */
#ifndef XEDIT_NIFLY_WRAPPER_H
#define XEDIT_NIFLY_WRAPPER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#ifdef _WIN32
  #define NIFLY_EXPORT __declspec(dllexport)
#else
  #define NIFLY_EXPORT __attribute__((visibility("default")))
#endif

/* Load a NIF file. Returns an opaque handle, or NULL on failure. */
NIFLY_EXPORT void* nifly_load(const char* path);

/* Destroy a previously loaded NIF handle. */
NIFLY_EXPORT void nifly_destroy(void* handle);

/* Return the number of blocks in the NIF header. */
NIFLY_EXPORT int nifly_get_block_count(void* handle);

/*
 * Get the block type name (e.g. "NiNode", "BSFadeNode") for the block at `index`.
 * Writes into `buf` up to `buflen` bytes (including null terminator).
 * Returns the length of the type name (excluding null), or -1 on error.
 */
NIFLY_EXPORT int nifly_get_block_type(void* handle, int index, char* buf, int buflen);

/* Return the number of shapes in the NIF. */
NIFLY_EXPORT int nifly_get_shape_count(void* handle);

/*
 * Get the texture path for a given shape (by index) and texture slot.
 * Slot indices: 0=diffuse, 1=normal, 2=glow, etc. (game-dependent).
 * Writes into `buf` up to `buflen` bytes (including null terminator).
 * Returns the length of the texture path, or 0 if no texture in that slot.
 * Returns -1 on error (bad handle, bad shape_index).
 */
NIFLY_EXPORT int nifly_get_texture_slot(void* handle, int shape_index, int slot,
                                        char* buf, int buflen);

#ifdef __cplusplus
}
#endif

#endif /* XEDIT_NIFLY_WRAPPER_H */
