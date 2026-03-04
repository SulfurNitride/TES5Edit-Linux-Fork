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

/* Create a new empty NIF file for a given game version.
 * game_version: 0=Oblivion, 1=Fallout3/FNV, 2=SkyrimLE, 3=SkyrimSE, 4=Fallout4, 5=Starfield
 * Returns an opaque handle, or NULL on failure. */
NIFLY_EXPORT void* nifly_create(int game_version);

/* Add a shape with geometry data. Returns the shape index (>= 0), or -1 on error. */
NIFLY_EXPORT int nifly_add_shape(void* handle, const char* name,
                                  const float* verts, int vert_count,
                                  const uint16_t* tris, int tri_count,
                                  const float* uvs, const float* normals);

/* Set a texture path for a shape. slot: 0=diffuse, 1=normal, etc. Returns 0 on success. */
NIFLY_EXPORT int nifly_set_texture(void* handle, int shape_idx, int slot, const char* path);

/* Get vertex positions for a shape. Writes up to max_count * 3 floats.
 * Returns actual vertex count, or -1 on error. */
NIFLY_EXPORT int nifly_get_vertices(void* handle, int shape_idx, float* out_buf, int max_count);

/* Get triangle indices for a shape. Writes up to max_count * 3 uint16_t values.
 * Returns actual triangle count, or -1 on error. */
NIFLY_EXPORT int nifly_get_triangles(void* handle, int shape_idx, uint16_t* out_buf, int max_count);

/* Get UV coordinates for a shape. Writes up to max_count * 2 floats.
 * Returns actual UV count (should equal vertex count), or -1 on error. */
NIFLY_EXPORT int nifly_get_uvs(void* handle, int shape_idx, float* out_buf, int max_count);

/* Get vertex normals for a shape. Writes up to max_count * 3 floats.
 * Returns actual count, or -1 on error. */
NIFLY_EXPORT int nifly_get_normals(void* handle, int shape_idx, float* out_buf, int max_count);

/* Get the vertex count for a shape. Returns count or -1 on error. */
NIFLY_EXPORT int nifly_get_vertex_count(void* handle, int shape_idx);

/* Get the triangle count for a shape. Returns count or -1 on error. */
NIFLY_EXPORT int nifly_get_triangle_count(void* handle, int shape_idx);

/* Save NIF to disk. Returns 0 on success, -1 on failure. */
NIFLY_EXPORT int nifly_save(void* handle, const char* path);

#ifdef __cplusplus
}
#endif

#endif /* XEDIT_NIFLY_WRAPPER_H */
