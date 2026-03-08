/*
 * xedit_nifly_wrapper.cpp - Minimal C wrapper around nifly for xEdit/Rust FFI
 *
 * Copyright (c) 2026 - Minimal wrapper for NIF file reading via nifly.
 */

#include "xedit_nifly_wrapper.h"

#include <cstring>
#include <algorithm>
#include <filesystem>
#include <vector>
#include <string>

#include "NifFile.hpp"

using namespace nifly;

/* ---- Helpers ---- */

static int copy_string_to_buf(const std::string& src, char* buf, int buflen) {
    if (!buf || buflen <= 0)
        return static_cast<int>(src.length());

    int copylen = std::min(static_cast<int>(src.length()), buflen - 1);
    std::memcpy(buf, src.data(), copylen);
    buf[copylen] = '\0';
    return static_cast<int>(src.length());
}

/* ---- API Implementation ---- */

extern "C" {

NIFLY_EXPORT void* nifly_load(const char* path) {
    if (!path)
        return nullptr;

    NifFile* nif = new (std::nothrow) NifFile();
    if (!nif)
        return nullptr;

    try {
        NifLoadOptions options;
        int err = nif->Load(std::filesystem::path(path), options);
        if (err != 0) {
            delete nif;
            return nullptr;
        }
        return static_cast<void*>(nif);
    }
    catch (...) {
        delete nif;
        return nullptr;
    }
}

NIFLY_EXPORT void nifly_destroy(void* handle) {
    if (handle) {
        NifFile* nif = static_cast<NifFile*>(handle);
        delete nif;
    }
}

NIFLY_EXPORT int nifly_get_block_count(void* handle) {
    if (!handle)
        return -1;

    NifFile* nif = static_cast<NifFile*>(handle);
    NiHeader& hdr = nif->GetHeader();
    return static_cast<int>(hdr.GetNumBlocks());
}

NIFLY_EXPORT int nifly_get_block_type(void* handle, int index, char* buf, int buflen) {
    if (!handle || index < 0)
        return -1;

    NifFile* nif = static_cast<NifFile*>(handle);
    NiHeader& hdr = nif->GetHeader();

    if (static_cast<uint32_t>(index) >= hdr.GetNumBlocks())
        return -1;

    NiObject* obj = hdr.GetBlock<NiObject>(static_cast<uint32_t>(index));
    if (!obj)
        return -1;

    std::string name = obj->GetBlockName();
    return copy_string_to_buf(name, buf, buflen);
}

NIFLY_EXPORT int nifly_get_shape_count(void* handle) {
    if (!handle)
        return -1;

    NifFile* nif = static_cast<NifFile*>(handle);
    std::vector<NiShape*> shapes = nif->GetShapes();
    return static_cast<int>(shapes.size());
}

NIFLY_EXPORT int nifly_get_texture_slot(void* handle, int shape_index, int slot,
                                        char* buf, int buflen) {
    if (!handle || shape_index < 0)
        return -1;

    NifFile* nif = static_cast<NifFile*>(handle);
    std::vector<NiShape*> shapes = nif->GetShapes();

    if (static_cast<size_t>(shape_index) >= shapes.size())
        return -1;

    NiShape* shape = shapes[shape_index];
    if (!shape)
        return -1;

    /* Check if there's a shader at all */
    NiShader* shader = nif->GetShader(shape);
    if (!shader) {
        if (buf && buflen > 0)
            buf[0] = '\0';
        return 0;
    }

    std::string texture;
    uint32_t result = nif->GetTextureSlot(shape, texture, static_cast<uint32_t>(slot));

    if (result == 0) {
        if (buf && buflen > 0)
            buf[0] = '\0';
        return 0;
    }

    return copy_string_to_buf(texture, buf, buflen);
}

NIFLY_EXPORT void* nifly_create(int game_version) {
    NifFile* nif = new (std::nothrow) NifFile();
    if (!nif) return nullptr;

    try {
        NiVersion version;
        switch (game_version) {
            case 0: version = NiVersion::getOB(); break;
            case 1: version = NiVersion::getFO3(); break;
            case 2: version = NiVersion::getSK(); break;
            case 3: version = NiVersion::getSSE(); break;
            case 4: version = NiVersion::getFO4(); break;
            case 5: version = NiVersion::getSF(); break;
            default:
                delete nif;
                return nullptr;
        }
        // Create() sets version AND adds root NiNode ("Scene Root").
        // Without the root node, CreateShapeFromData() returns nullptr.
        nif->Create(version);
        return static_cast<void*>(nif);
    } catch (...) {
        delete nif;
        return nullptr;
    }
}

NIFLY_EXPORT int nifly_add_shape(void* handle, const char* name,
                                  const float* verts, int vert_count,
                                  const uint16_t* tris, int tri_count,
                                  const float* uvs, const float* normals) {
    if (!handle || !verts || vert_count <= 0 || !tris || tri_count <= 0)
        return -1;

    NifFile* nif = static_cast<NifFile*>(handle);

    try {
        std::vector<Vector3> vertVec(vert_count);
        for (int i = 0; i < vert_count; i++) {
            vertVec[i].x = verts[i * 3];
            vertVec[i].y = verts[i * 3 + 1];
            vertVec[i].z = verts[i * 3 + 2];
        }

        std::vector<Triangle> triVec(tri_count);
        for (int i = 0; i < tri_count; i++) {
            triVec[i].p1 = tris[i * 3];
            triVec[i].p2 = tris[i * 3 + 1];
            triVec[i].p3 = tris[i * 3 + 2];
        }

        std::vector<Vector2> uvVec;
        if (uvs) {
            uvVec.resize(vert_count);
            for (int i = 0; i < vert_count; i++) {
                uvVec[i].u = uvs[i * 2];
                uvVec[i].v = uvs[i * 2 + 1];
            }
        }

        std::vector<Vector3> normVec;
        if (normals) {
            normVec.resize(vert_count);
            for (int i = 0; i < vert_count; i++) {
                normVec[i].x = normals[i * 3];
                normVec[i].y = normals[i * 3 + 1];
                normVec[i].z = normals[i * 3 + 2];
            }
        }

        std::string shapeName = name ? name : "Shape";
        NiShape* shape = nif->CreateShapeFromData(shapeName, &vertVec, &triVec,
                                                   uvs ? &uvVec : nullptr,
                                                   normals ? &normVec : nullptr);
        if (!shape) return -1;

        /* Find the shape's index */
        auto shapes = nif->GetShapes();
        for (int i = 0; i < static_cast<int>(shapes.size()); i++) {
            if (shapes[i] == shape) return i;
        }
        return static_cast<int>(shapes.size()) - 1;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_set_texture(void* handle, int shape_idx, int slot, const char* path) {
    if (!handle || shape_idx < 0 || !path)
        return -1;

    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size())
        return -1;

    NiShape* shape = shapes[shape_idx];
    if (!shape)
        return -1;

    try {
        std::string texPath(path);
        nif->SetTextureSlot(shape, texPath, static_cast<uint32_t>(slot));
        return 0;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_get_vertices(void* handle, int shape_idx, float* out_buf, int max_count) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    const auto* verts = nif->GetVertsForShape(shape);
    if (!verts) return 0;

    int count = static_cast<int>(verts->size());
    if (out_buf && max_count > 0) {
        int copyCount = std::min(count, max_count);
        for (int i = 0; i < copyCount; i++) {
            out_buf[i * 3]     = (*verts)[i].x;
            out_buf[i * 3 + 1] = (*verts)[i].y;
            out_buf[i * 3 + 2] = (*verts)[i].z;
        }
    }
    return count;
}

NIFLY_EXPORT int nifly_get_triangles(void* handle, int shape_idx, uint16_t* out_buf, int max_count) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    std::vector<Triangle> tris;
    shape->GetTriangles(tris);

    int count = static_cast<int>(tris.size());
    if (out_buf && max_count > 0) {
        int copyCount = std::min(count, max_count);
        for (int i = 0; i < copyCount; i++) {
            out_buf[i * 3]     = tris[i].p1;
            out_buf[i * 3 + 1] = tris[i].p2;
            out_buf[i * 3 + 2] = tris[i].p3;
        }
    }
    return count;
}

NIFLY_EXPORT int nifly_get_uvs(void* handle, int shape_idx, float* out_buf, int max_count) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    const auto* uvs = nif->GetUvsForShape(shape);
    if (!uvs) return 0;

    int count = static_cast<int>(uvs->size());
    if (out_buf && max_count > 0) {
        int copyCount = std::min(count, max_count);
        for (int i = 0; i < copyCount; i++) {
            out_buf[i * 2]     = (*uvs)[i].u;
            out_buf[i * 2 + 1] = (*uvs)[i].v;
        }
    }
    return count;
}

NIFLY_EXPORT int nifly_get_normals(void* handle, int shape_idx, float* out_buf, int max_count) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    const auto* norms = nif->GetNormalsForShape(shape);
    if (!norms) return 0;

    int count = static_cast<int>(norms->size());
    if (out_buf && max_count > 0) {
        int copyCount = std::min(count, max_count);
        for (int i = 0; i < copyCount; i++) {
            out_buf[i * 3]     = (*norms)[i].x;
            out_buf[i * 3 + 1] = (*norms)[i].y;
            out_buf[i * 3 + 2] = (*norms)[i].z;
        }
    }
    return count;
}

NIFLY_EXPORT int nifly_get_vertex_count(void* handle, int shape_idx) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    return static_cast<int>(shape->GetNumVertices());
}

NIFLY_EXPORT int nifly_get_triangle_count(void* handle, int shape_idx) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    return static_cast<int>(shape->GetNumTriangles());
}

NIFLY_EXPORT int nifly_save(void* handle, const char* path) {
    if (!handle || !path) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    try {
        int result = nif->Save(std::filesystem::path(path));
        return (result == 0) ? 0 : -1;
    } catch (...) {
        return -1;
    }
}

/* ---- Transform helpers ---- */

static void mat_transform_to_floats(const MatTransform& t,
                                     float* out_translation,
                                     float* out_rotation,
                                     float* out_scale) {
    if (out_translation) {
        out_translation[0] = t.translation.x;
        out_translation[1] = t.translation.y;
        out_translation[2] = t.translation.z;
    }
    if (out_rotation) {
        // Row-major 3x3 matrix
        for (int r = 0; r < 3; r++)
            for (int c = 0; c < 3; c++)
                out_rotation[r * 3 + c] = t.rotation[r][c];
    }
    if (out_scale) {
        *out_scale = t.scale;
    }
}

NIFLY_EXPORT int nifly_get_root_translation(void* handle, float* out_xyz) {
    if (!handle || !out_xyz) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    try {
        Vector3 v;
        nif->GetRootTranslation(v);
        out_xyz[0] = v.x;
        out_xyz[1] = v.y;
        out_xyz[2] = v.z;
        return 0;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_get_shape_parent_node(void* handle, int shape_idx,
                                              char* buf, int buflen) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    NiNode* parent = nif->GetParentNode(shape);
    if (!parent) {
        if (buf && buflen > 0) buf[0] = '\0';
        return 0;
    }

    std::string name = parent->name.get();
    return copy_string_to_buf(name, buf, buflen);
}

NIFLY_EXPORT int nifly_get_node_transform(void* handle, const char* node_name,
                                           float* out_translation,
                                           float* out_rotation,
                                           float* out_scale) {
    if (!handle || !node_name) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    try {
        MatTransform t;
        bool found = nif->GetNodeTransformToParent(node_name, t);
        if (!found) return 0;
        mat_transform_to_floats(t, out_translation, out_rotation, out_scale);
        return 1;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_get_node_transform_global(void* handle, const char* node_name,
                                                  float* out_translation,
                                                  float* out_rotation,
                                                  float* out_scale) {
    if (!handle || !node_name) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    try {
        MatTransform t;
        bool found = nif->GetNodeTransformToGlobal(node_name, t);
        if (!found) return 0;
        mat_transform_to_floats(t, out_translation, out_rotation, out_scale);
        return 1;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_get_shape_transform(void* handle, int shape_idx,
                                            float* out_translation,
                                            float* out_rotation,
                                            float* out_scale) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    if (!shape) return -1;

    try {
        // Get shape's own transform-to-parent
        auto& t = shape->GetTransformToParent();
        mat_transform_to_floats(t, out_translation, out_rotation, out_scale);
        return 1;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_get_shape_global_transform(void* handle, int shape_idx,
                                                   float* out_translation,
                                                   float* out_rotation,
                                                   float* out_scale) {
    if (!handle || shape_idx < 0) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);
    auto shapes = nif->GetShapes();
    if (static_cast<size_t>(shape_idx) >= shapes.size()) return -1;

    NiShape* shape = shapes[shape_idx];
    if (!shape) return -1;

    try {
        // Get shape's own transform
        MatTransform shapeTrans = shape->GetTransformToParent();

        // Get parent node's global transform and compose
        NiNode* parent = nif->GetParentNode(shape);
        if (parent) {
            std::string parentName = parent->name.get();
            MatTransform parentGlobal;
            if (nif->GetNodeTransformToGlobal(parentName, parentGlobal)) {
                // Compose: parentGlobal * shapeTrans
                shapeTrans = parentGlobal.ComposeTransforms(shapeTrans);
            }
        }

        mat_transform_to_floats(shapeTrans, out_translation, out_rotation, out_scale);
        return 1;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT void* nifly_create_lod(int game_version) {
    NifFile* nif = new (std::nothrow) NifFile();
    if (!nif) return nullptr;

    try {
        NiVersion version;
        switch (game_version) {
            case 0: version = NiVersion::getOB(); break;
            case 1: version = NiVersion::getFO3(); break;
            case 2: version = NiVersion::getSK(); break;
            case 3: version = NiVersion::getSSE(); break;
            case 4: version = NiVersion::getFO4(); break;
            case 5: version = NiVersion::getSF(); break;
            default:
                delete nif;
                return nullptr;
        }

        // Create normally (adds NiNode root at block 0)
        nif->Create(version);

        NiHeader& hdr = nif->GetHeader();

        // Replace root NiNode with BSMultiBoundNode
        // (no children yet — CreateShapeFromData adds them later)
        auto mbRoot = std::make_unique<BSMultiBoundNode>();
        mbRoot->name.get() = "Scene Root";
        hdr.ReplaceBlock(0, std::move(mbRoot));

        return static_cast<void*>(nif);
    } catch (...) {
        delete nif;
        return nullptr;
    }
}

NIFLY_EXPORT int nifly_add_multibound(void* handle,
                                       float center_x, float center_y, float center_z,
                                       float extent_x, float extent_y, float extent_z) {
    if (!handle) return -1;
    NifFile* nif = static_cast<NifFile*>(handle);

    try {
        NiHeader& hdr = nif->GetHeader();

        // Create BSMultiBoundAABB
        auto aabb = std::make_unique<BSMultiBoundAABB>();
        aabb->center = {center_x, center_y, center_z};
        aabb->halfExtent = {extent_x, extent_y, extent_z};
        auto aabbIdx = hdr.AddBlock(std::move(aabb));

        // Create BSMultiBound pointing to AABB
        auto mb = std::make_unique<BSMultiBound>();
        mb->dataRef.index = aabbIdx;
        auto mbIdx = hdr.AddBlock(std::move(mb));

        // Link root BSMultiBoundNode to BSMultiBound
        auto* root = hdr.GetBlock<BSMultiBoundNode>(static_cast<uint32_t>(0));
        if (!root) return -1;
        root->multiBoundRef.index = mbIdx;

        return 0;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_calc_tangents(void* handle, int shape_idx)
{
    if (!handle) return -1;
    try {
        auto* nif = static_cast<NifFile*>(handle);
        auto shapes = nif->GetShapes();
        if (shape_idx < 0 || shape_idx >= static_cast<int>(shapes.size()))
            return -1;
        nif->CalcTangentsForShape(shapes[shape_idx]);
        return 0;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_set_root_translation(void* handle, float x, float y, float z)
{
    if (!handle) return -1;
    try {
        auto* nif = static_cast<NifFile*>(handle);
        NiHeader& hdr = nif->GetHeader();
        auto* root = hdr.GetBlock<NiNode>(static_cast<uint32_t>(0));
        if (!root) return -1;
        root->transform.translation = {x, y, z};
        return 0;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_set_root_flags(void* handle, uint16_t flags, uint16_t flags2)
{
    if (!handle) return -1;
    try {
        auto* nif = static_cast<NifFile*>(handle);
        NiHeader& hdr = nif->GetHeader();
        auto* root = hdr.GetBlock<NiNode>(static_cast<uint32_t>(0));
        if (!root) return -1;
        root->flags = static_cast<uint32_t>(flags) | (static_cast<uint32_t>(flags2) << 16);
        return 0;
    } catch (...) {
        return -1;
    }
}

NIFLY_EXPORT int nifly_set_texture_clamp_mode(void* handle, int shape_idx, uint32_t mode)
{
    if (!handle) return -1;
    try {
        auto* nif = static_cast<NifFile*>(handle);
        auto shapes = nif->GetShapes();
        if (shape_idx < 0 || shape_idx >= static_cast<int>(shapes.size()))
            return -1;

        auto* shader = nif->GetShader(shapes[shape_idx]);
        if (!shader) return -1;

        // BSLightingShaderProperty and BSEffectShaderProperty both have textureClampMode
        if (auto* lsp = dynamic_cast<BSLightingShaderProperty*>(shader)) {
            lsp->textureClampMode = mode;
            return 0;
        }
        if (auto* esp = dynamic_cast<BSEffectShaderProperty*>(shader)) {
            esp->textureClampMode = mode;
            return 0;
        }
        return -1;
    } catch (...) {
        return -1;
    }
}

} /* extern "C" */
