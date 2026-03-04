#ifndef XEDIT_FFI_H
#define XEDIT_FFI_H

#include <cstdint>
#include <QString>

#include "XEditTypes.h"

// ---------------------------------------------------------------------------
// Function pointer typedefs for all 50 Rust FFI functions
// ---------------------------------------------------------------------------

// Session lifecycle
typedef int32_t (*fn_xedit_init)(const char* game_name, const char* data_path, void* progress_cb);
typedef int32_t (*fn_xedit_shutdown)();

// Plugin I/O
typedef int32_t (*fn_xedit_load_plugin)(const char* file_path);
typedef int32_t (*fn_xedit_save_plugin)(int32_t plugin_index, const char* file_path);

// Plugin queries
typedef int32_t (*fn_xedit_plugin_count)();
typedef int32_t (*fn_xedit_plugin_filename)(int32_t plugin_index, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_plugin_record_count)(int32_t plugin_index);
typedef int32_t (*fn_xedit_plugin_master_count)(int32_t plugin_index);
typedef int32_t (*fn_xedit_plugin_master_name)(int32_t plugin_index, int32_t master_index, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_plugin_group_count)(int32_t plugin_index);
typedef int32_t (*fn_xedit_plugin_load_order_id)(int32_t plugin_index);

// Group queries
typedef int32_t (*fn_xedit_group_signature)(int32_t plugin_idx, int32_t group_idx, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_group_name)(int32_t plugin_idx, int32_t group_idx, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_group_record_count)(int32_t plugin_idx, int32_t group_idx);
typedef int32_t (*fn_xedit_group_form_ids)(int32_t plugin_idx, int32_t group_idx, uint32_t* buf, int32_t buf_len);

// Record queries
typedef int32_t (*fn_xedit_record_editor_id)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, char* buf, int32_t buf_len);
typedef uint32_t (*fn_xedit_record_form_id)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx);
typedef int32_t (*fn_xedit_record_signature)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_record_subrecord_count)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx);

// Version
typedef int32_t (*fn_xedit_version)(char* buf, int32_t buf_len);

// Subrecord queries
typedef int32_t (*fn_xedit_subrecord_signature)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, int32_t sub_idx, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_subrecord_size)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, int32_t sub_idx);
typedef int32_t (*fn_xedit_subrecord_data)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, int32_t sub_idx, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_subrecord_display_value)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, int32_t sub_idx, uint8_t* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_record_subrecords_batch)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, uint8_t* buf, int32_t buf_len);

// Search
typedef int32_t (*fn_xedit_search_editor_id)(int32_t plugin_idx, const char* query, int32_t* results_buf, int32_t max_results);
typedef int32_t (*fn_xedit_search_form_id)(int32_t plugin_idx, uint32_t form_id, int32_t* out_group_idx, int32_t* out_record_idx);

// Conflict detection
typedef int32_t (*fn_xedit_detect_conflicts)(void* handle);
typedef uint32_t (*fn_xedit_conflict_form_id)(void* handle, int32_t conflict_index);
typedef int32_t (*fn_xedit_conflict_severity)(void* handle, int32_t conflict_index);
typedef int32_t (*fn_xedit_conflict_plugin_count)(void* handle, int32_t conflict_index);

// ITM detection and cleaning
typedef int32_t (*fn_xedit_detect_itm)(void* handle, int32_t plugin_index, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_clean_itm)(void* handle, int32_t plugin_index);
typedef int32_t (*fn_xedit_clean_deleted)(void* handle, int32_t plugin_index);

// Load order
typedef int32_t (*fn_xedit_sort_load_order)(void* handle);

// Form ID resolution and overrides
typedef uint32_t (*fn_xedit_resolve_form_id)(void* handle, int32_t plugin_index, uint32_t raw_form_id);
typedef int32_t (*fn_xedit_find_overrides)(void* handle, uint32_t form_id, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_find_overrides_full)(uint32_t form_id, uint8_t* buf, int32_t buf_len);

// Conflict status
typedef int32_t (*fn_xedit_record_conflict_status)(uint32_t form_id, uint8_t* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_subrecord_conflict_status)(uint32_t form_id, uint8_t* buf, int32_t buf_len);

// NIF (mesh) queries
typedef int32_t (*fn_xedit_nif_block_count)(void* handle, const char* path);
typedef int32_t (*fn_xedit_nif_texture_count)(void* handle, const char* path);
typedef int32_t (*fn_xedit_nif_texture_path)(void* handle, const char* path, int32_t index, char* buf, int32_t buf_len);

// Asset scanning
typedef int32_t (*fn_xedit_scan_assets)(void* handle, int32_t plugin_index, char* buf, int32_t buf_len);

// Progress callback
typedef int32_t (*fn_xedit_set_progress_callback)(void* handle, void* callback);

// Referenced-by index
typedef int32_t (*fn_xedit_build_refby_index)();
typedef int32_t (*fn_xedit_build_refby_index_async)();
typedef int32_t (*fn_xedit_refby_build_status)();
typedef int32_t (*fn_xedit_record_refby_count)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx);
typedef int32_t (*fn_xedit_record_refby_entry)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, int32_t ref_index, int32_t* out_plugin, int32_t* out_group, int32_t* out_record);
typedef int32_t (*fn_xedit_record_refby_batch)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, uint8_t* buf, int32_t buf_len);

// Subrecord offloading
typedef int32_t (*fn_xedit_offload_subrecords)();

// MO2 integration
typedef int32_t (*fn_xedit_load_mo2)(void* handle, const char* mo2_path);
typedef int32_t (*fn_xedit_mo2_profile_count)(void* handle);
typedef int32_t (*fn_xedit_mo2_profile_name)(void* handle, int32_t index, char* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_mo2_select_profile)(void* handle, const char* profile_name);
typedef int32_t (*fn_xedit_mo2_load_order)(void* handle);

// Record/Subrecord mutation
typedef int32_t (*fn_xedit_set_subrecord_data)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx, int32_t sub_idx, const uint8_t* data, int32_t data_len);
typedef int32_t (*fn_xedit_delete_record)(int32_t plugin_idx, int32_t group_idx, int32_t record_idx);
typedef int32_t (*fn_xedit_copy_record)(int32_t src_plugin_idx, int32_t src_group_idx, int32_t src_record_idx, int32_t dst_plugin_idx);
typedef int32_t (*fn_xedit_add_record)(int32_t plugin_idx, int32_t group_idx, uint32_t form_id, const uint8_t* signature);

// LOD generation
typedef int32_t (*fn_xedit_lod_list_worldspaces)(uint8_t* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_lod_generate)(const char* options_json, void* progress_cb);
typedef int32_t (*fn_xedit_lod_status)();
typedef int32_t (*fn_xedit_lod_error)(uint8_t* buf, int32_t buf_len);
typedef int32_t (*fn_xedit_lod_cancel)();

// ---------------------------------------------------------------------------
// XEditFFI -- singleton that loads the Rust core library at runtime.
// Linux:   libxedit_core.so  via dlopen / dlsym
// Windows: xedit_core.dll    via LoadLibrary / GetProcAddress
// ---------------------------------------------------------------------------

class XEditFFI
{
public:
    static XEditFFI& instance();

    bool load(const QString& libPath = {});
    bool isLoaded() const;
    void unload();

    // -- Public function pointers (all 50) ----------------------------------

    // Session lifecycle
    fn_xedit_init                  xedit_init                  = nullptr;
    fn_xedit_shutdown              xedit_shutdown              = nullptr;

    // Plugin I/O
    fn_xedit_load_plugin           xedit_load_plugin           = nullptr;
    fn_xedit_save_plugin           xedit_save_plugin           = nullptr;

    // Plugin queries
    fn_xedit_plugin_count          xedit_plugin_count          = nullptr;
    fn_xedit_plugin_filename       xedit_plugin_filename       = nullptr;
    fn_xedit_plugin_record_count   xedit_plugin_record_count   = nullptr;
    fn_xedit_plugin_master_count   xedit_plugin_master_count   = nullptr;
    fn_xedit_plugin_master_name    xedit_plugin_master_name    = nullptr;
    fn_xedit_plugin_group_count    xedit_plugin_group_count    = nullptr;
    fn_xedit_plugin_load_order_id  xedit_plugin_load_order_id  = nullptr;

    // Group queries
    fn_xedit_group_signature       xedit_group_signature       = nullptr;
    fn_xedit_group_name            xedit_group_name            = nullptr;
    fn_xedit_group_record_count    xedit_group_record_count    = nullptr;
    fn_xedit_group_form_ids        xedit_group_form_ids        = nullptr;

    // Record queries
    fn_xedit_record_editor_id      xedit_record_editor_id      = nullptr;
    fn_xedit_record_form_id        xedit_record_form_id        = nullptr;
    fn_xedit_record_signature      xedit_record_signature      = nullptr;
    fn_xedit_record_subrecord_count xedit_record_subrecord_count = nullptr;

    // Version
    fn_xedit_version               xedit_version               = nullptr;

    // Subrecord queries
    fn_xedit_subrecord_signature   xedit_subrecord_signature   = nullptr;
    fn_xedit_subrecord_size        xedit_subrecord_size        = nullptr;
    fn_xedit_subrecord_data        xedit_subrecord_data        = nullptr;
    fn_xedit_subrecord_display_value xedit_subrecord_display_value = nullptr;
    fn_xedit_record_subrecords_batch xedit_record_subrecords_batch = nullptr;

    // Search
    fn_xedit_search_editor_id      xedit_search_editor_id      = nullptr;
    fn_xedit_search_form_id        xedit_search_form_id        = nullptr;

    // Conflict detection
    fn_xedit_detect_conflicts      xedit_detect_conflicts      = nullptr;
    fn_xedit_conflict_form_id      xedit_conflict_form_id      = nullptr;
    fn_xedit_conflict_severity     xedit_conflict_severity     = nullptr;
    fn_xedit_conflict_plugin_count xedit_conflict_plugin_count = nullptr;

    // ITM detection and cleaning
    fn_xedit_detect_itm            xedit_detect_itm            = nullptr;
    fn_xedit_clean_itm             xedit_clean_itm             = nullptr;
    fn_xedit_clean_deleted         xedit_clean_deleted         = nullptr;

    // Load order
    fn_xedit_sort_load_order       xedit_sort_load_order       = nullptr;

    // Form ID resolution and overrides
    fn_xedit_resolve_form_id       xedit_resolve_form_id       = nullptr;
    fn_xedit_find_overrides        xedit_find_overrides        = nullptr;
    fn_xedit_find_overrides_full   xedit_find_overrides_full   = nullptr;

    // Conflict status
    fn_xedit_record_conflict_status    xedit_record_conflict_status    = nullptr;
    fn_xedit_subrecord_conflict_status xedit_subrecord_conflict_status = nullptr;

    // NIF (mesh) queries
    fn_xedit_nif_block_count       xedit_nif_block_count       = nullptr;
    fn_xedit_nif_texture_count     xedit_nif_texture_count     = nullptr;
    fn_xedit_nif_texture_path      xedit_nif_texture_path      = nullptr;

    // Asset scanning
    fn_xedit_scan_assets           xedit_scan_assets           = nullptr;

    // Progress callback
    fn_xedit_set_progress_callback xedit_set_progress_callback = nullptr;

    // Referenced-by index
    fn_xedit_build_refby_index       xedit_build_refby_index       = nullptr;
    fn_xedit_build_refby_index_async xedit_build_refby_index_async = nullptr;
    fn_xedit_refby_build_status      xedit_refby_build_status      = nullptr;
    fn_xedit_record_refby_count      xedit_record_refby_count      = nullptr;
    fn_xedit_record_refby_entry      xedit_record_refby_entry      = nullptr;
    fn_xedit_record_refby_batch      xedit_record_refby_batch      = nullptr;

    // Subrecord offloading
    fn_xedit_offload_subrecords      xedit_offload_subrecords      = nullptr;

    // MO2 integration
    fn_xedit_load_mo2             xedit_load_mo2             = nullptr;
    fn_xedit_mo2_profile_count    xedit_mo2_profile_count    = nullptr;
    fn_xedit_mo2_profile_name     xedit_mo2_profile_name     = nullptr;
    fn_xedit_mo2_select_profile   xedit_mo2_select_profile   = nullptr;
    fn_xedit_mo2_load_order       xedit_mo2_load_order       = nullptr;

    // Record/Subrecord mutation
    fn_xedit_set_subrecord_data    xedit_set_subrecord_data    = nullptr;
    fn_xedit_delete_record         xedit_delete_record         = nullptr;
    fn_xedit_copy_record           xedit_copy_record           = nullptr;
    fn_xedit_add_record            xedit_add_record            = nullptr;

    // LOD generation
    fn_xedit_lod_list_worldspaces  xedit_lod_list_worldspaces  = nullptr;
    fn_xedit_lod_generate          xedit_lod_generate          = nullptr;
    fn_xedit_lod_status            xedit_lod_status            = nullptr;
    fn_xedit_lod_error             xedit_lod_error             = nullptr;
    fn_xedit_lod_cancel            xedit_lod_cancel            = nullptr;

private:
    XEditFFI() = default;
    ~XEditFFI();

    XEditFFI(const XEditFFI&) = delete;
    XEditFFI& operator=(const XEditFFI&) = delete;

    template <typename T>
    T resolve(const char* name);

    void* m_libHandle = nullptr;
};

#endif // XEDIT_FFI_H
