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

} /* extern "C" */
