#ifndef XEDIT_TYPES_H
#define XEDIT_TYPES_H

#include <cstdint>

// Error codes matching the Rust FFI side
enum XEditError : int32_t {
    XEDIT_OK              =   0,
    XEDIT_ERR_NULL_HANDLE = -1,
    XEDIT_ERR_INVALID_PATH = -2,
    XEDIT_ERR_LOAD_FAILED  = -3,
    XEDIT_ERR_SAVE_FAILED  = -4,
    XEDIT_ERR_NIFLY_MISSING = -5,
    XEDIT_ERR_INVALID_GAME  = -6,
    XEDIT_ERR_PANIC         = -99
};

// Progress callback signature used by xedit_init and xedit_set_progress_callback
extern "C" {
typedef void (*ProgressCallback)(const char* message, double progress);
}

// Node types for the tree view
enum class NodeType {
    Plugin,
    Group,
    Record
};

#endif // XEDIT_TYPES_H
