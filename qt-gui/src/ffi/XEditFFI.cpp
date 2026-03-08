#include "XEditFFI.h"

#ifdef _WIN32
#  define WIN32_LEAN_AND_MEAN
#  include <windows.h>
#else
#  include <dlfcn.h>
#endif

#include <QCoreApplication>
#include <QDir>
#include <QDebug>

// ---------------------------------------------------------------------------
// Platform abstraction for dynamic library loading
// ---------------------------------------------------------------------------

#ifdef _WIN32
static void* plat_dlopen(const char* path) {
    return reinterpret_cast<void*>(LoadLibraryA(path));
}
static void* plat_dlsym(void* handle, const char* name) {
    return reinterpret_cast<void*>(GetProcAddress(reinterpret_cast<HMODULE>(handle), name));
}
static void plat_dlclose(void* handle) {
    FreeLibrary(reinterpret_cast<HMODULE>(handle));
}
static QString plat_dlerror() {
    DWORD err = GetLastError();
    if (!err) return {};
    LPSTR buf = nullptr;
    FormatMessageA(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM,
                   nullptr, err, 0, reinterpret_cast<LPSTR>(&buf), 0, nullptr);
    QString msg = buf ? QString::fromLocal8Bit(buf) : QStringLiteral("Unknown error");
    LocalFree(buf);
    return msg;
}
#else
static void* plat_dlopen(const char* path) { return dlopen(path, RTLD_NOW); }
static void* plat_dlsym(void* handle, const char* name) { return dlsym(handle, name); }
static void plat_dlclose(void* handle) { dlclose(handle); }
static QString plat_dlerror() { const char* e = dlerror(); return e ? QString::fromUtf8(e) : QString(); }
#endif

// ---------------------------------------------------------------------------
// Template helper -- resolve a single symbol from the loaded library
// ---------------------------------------------------------------------------

template <typename T>
T XEditFFI::resolve(const char* name)
{
    if (!m_libHandle)
        return nullptr;

    void* sym = plat_dlsym(m_libHandle, name);
    if (!sym)
        qWarning() << "XEditFFI: failed to resolve" << name << "-" << plat_dlerror();
    return reinterpret_cast<T>(sym);
}

// ---------------------------------------------------------------------------
// Meyer's singleton
// ---------------------------------------------------------------------------

XEditFFI& XEditFFI::instance()
{
    static XEditFFI s;
    return s;
}

// ---------------------------------------------------------------------------
// Destructor
// ---------------------------------------------------------------------------

XEditFFI::~XEditFFI()
{
    unload();
}

// ---------------------------------------------------------------------------
// load() -- try to open libxedit_core.so from several locations
// ---------------------------------------------------------------------------

bool XEditFFI::load(const QString& libPath)
{
    if (m_libHandle)
        return true; // already loaded

#ifdef _WIN32
    const char* libName = "xedit_core.dll";
#else
    const char* libName = "libxedit_core.so";
#endif

    // Build a list of candidate paths to try in order
    QStringList candidates;

    // 1. Explicit path supplied by caller
    if (!libPath.isEmpty())
        candidates << libPath;

    // 2. <appdir>/../lib/  (typical AppImage / install layout)
    QString appDir = QCoreApplication::applicationDirPath();
    candidates << QDir(appDir).filePath(QStringLiteral("../lib/") + QLatin1String(libName));

    // 3. Same directory as the executable
    candidates << QDir(appDir).filePath(QLatin1String(libName));

    // 4. Bare name -- let the dynamic linker search LD_LIBRARY_PATH / PATH
    candidates << QLatin1String(libName);

    for (const QString& path : candidates) {
        m_libHandle = plat_dlopen(path.toUtf8().constData());
        if (m_libHandle) {
            qDebug() << "XEditFFI: loaded library from" << path;
            break;
        }
    }

    if (!m_libHandle) {
        qWarning() << "XEditFFI: could not load" << libName << "-" << plat_dlerror();
        return false;
    }

    // -----------------------------------------------------------------------
    // Resolve all 50 function pointers
    // -----------------------------------------------------------------------

    // Session lifecycle
    xedit_init                  = resolve<fn_xedit_init>("xedit_init");
    xedit_shutdown              = resolve<fn_xedit_shutdown>("xedit_shutdown");

    // Plugin I/O
    xedit_load_plugin           = resolve<fn_xedit_load_plugin>("xedit_load_plugin");
    xedit_save_plugin           = resolve<fn_xedit_save_plugin>("xedit_save_plugin");

    // Plugin queries
    xedit_plugin_count          = resolve<fn_xedit_plugin_count>("xedit_plugin_count");
    xedit_plugin_filename       = resolve<fn_xedit_plugin_filename>("xedit_plugin_filename");
    xedit_plugin_record_count   = resolve<fn_xedit_plugin_record_count>("xedit_plugin_record_count");
    xedit_plugin_master_count   = resolve<fn_xedit_plugin_master_count>("xedit_plugin_master_count");
    xedit_plugin_master_name    = resolve<fn_xedit_plugin_master_name>("xedit_plugin_master_name");
    xedit_plugin_group_count    = resolve<fn_xedit_plugin_group_count>("xedit_plugin_group_count");
    xedit_plugin_load_order_id  = resolve<fn_xedit_plugin_load_order_id>("xedit_plugin_load_order_id");

    // Group queries
    xedit_group_signature       = resolve<fn_xedit_group_signature>("xedit_group_signature");
    xedit_group_name            = resolve<fn_xedit_group_name>("xedit_group_name");
    xedit_group_record_count    = resolve<fn_xedit_group_record_count>("xedit_group_record_count");
    xedit_group_form_ids        = resolve<fn_xedit_group_form_ids>("xedit_group_form_ids");

    // Record queries
    xedit_record_editor_id      = resolve<fn_xedit_record_editor_id>("xedit_record_editor_id");
    xedit_record_form_id        = resolve<fn_xedit_record_form_id>("xedit_record_form_id");
    xedit_record_signature      = resolve<fn_xedit_record_signature>("xedit_record_signature");
    xedit_record_subrecord_count = resolve<fn_xedit_record_subrecord_count>("xedit_record_subrecord_count");

    // Version
    xedit_version               = resolve<fn_xedit_version>("xedit_version");

    // Subrecord queries
    xedit_subrecord_signature   = resolve<fn_xedit_subrecord_signature>("xedit_subrecord_signature");
    xedit_subrecord_size        = resolve<fn_xedit_subrecord_size>("xedit_subrecord_size");
    xedit_subrecord_data        = resolve<fn_xedit_subrecord_data>("xedit_subrecord_data");
    xedit_subrecord_display_value = resolve<fn_xedit_subrecord_display_value>("xedit_subrecord_display_value");
    xedit_record_subrecords_batch = resolve<fn_xedit_record_subrecords_batch>("xedit_record_subrecords_batch");

    // Search
    xedit_search_editor_id      = resolve<fn_xedit_search_editor_id>("xedit_search_editor_id");
    xedit_search_form_id        = resolve<fn_xedit_search_form_id>("xedit_search_form_id");

    // Conflict detection
    xedit_detect_conflicts      = resolve<fn_xedit_detect_conflicts>("xedit_detect_conflicts");
    xedit_conflict_form_id      = resolve<fn_xedit_conflict_form_id>("xedit_conflict_form_id");
    xedit_conflict_severity     = resolve<fn_xedit_conflict_severity>("xedit_conflict_severity");
    xedit_conflict_plugin_count = resolve<fn_xedit_conflict_plugin_count>("xedit_conflict_plugin_count");

    // ITM detection and cleaning
    xedit_detect_itm            = resolve<fn_xedit_detect_itm>("xedit_detect_itm");
    xedit_clean_itm             = resolve<fn_xedit_clean_itm>("xedit_clean_itm");
    xedit_clean_deleted         = resolve<fn_xedit_clean_deleted>("xedit_clean_deleted");

    // Load order
    xedit_sort_load_order       = resolve<fn_xedit_sort_load_order>("xedit_sort_load_order");

    // Form ID resolution and overrides
    xedit_resolve_form_id       = resolve<fn_xedit_resolve_form_id>("xedit_resolve_form_id");
    xedit_find_overrides        = resolve<fn_xedit_find_overrides>("xedit_find_overrides");
    xedit_find_overrides_full   = resolve<fn_xedit_find_overrides_full>("xedit_find_overrides_full");

    // Conflict status
    xedit_record_conflict_status    = resolve<fn_xedit_record_conflict_status>("xedit_record_conflict_status");
    xedit_subrecord_conflict_status = resolve<fn_xedit_subrecord_conflict_status>("xedit_subrecord_conflict_status");

    // NIF (mesh) queries
    xedit_nif_block_count       = resolve<fn_xedit_nif_block_count>("xedit_nif_block_count");
    xedit_nif_texture_count     = resolve<fn_xedit_nif_texture_count>("xedit_nif_texture_count");
    xedit_nif_texture_path      = resolve<fn_xedit_nif_texture_path>("xedit_nif_texture_path");

    // Asset scanning
    xedit_scan_assets           = resolve<fn_xedit_scan_assets>("xedit_scan_assets");

    // Progress callback
    xedit_set_progress_callback = resolve<fn_xedit_set_progress_callback>("xedit_set_progress_callback");

    // Referenced-by index
    xedit_build_refby_index       = resolve<fn_xedit_build_refby_index>("xedit_build_refby_index");
    xedit_build_refby_index_async = resolve<fn_xedit_build_refby_index_async>("xedit_build_refby_index_async");
    xedit_refby_build_status      = resolve<fn_xedit_refby_build_status>("xedit_refby_build_status");
    xedit_record_refby_count      = resolve<fn_xedit_record_refby_count>("xedit_record_refby_count");
    xedit_record_refby_entry      = resolve<fn_xedit_record_refby_entry>("xedit_record_refby_entry");
    xedit_record_refby_batch      = resolve<fn_xedit_record_refby_batch>("xedit_record_refby_batch");

    // Subrecord offloading
    xedit_offload_subrecords      = resolve<fn_xedit_offload_subrecords>("xedit_offload_subrecords");

    // MO2 integration
    xedit_load_mo2             = resolve<fn_xedit_load_mo2>("xedit_load_mo2");
    xedit_mo2_profile_count    = resolve<fn_xedit_mo2_profile_count>("xedit_mo2_profile_count");
    xedit_mo2_profile_name     = resolve<fn_xedit_mo2_profile_name>("xedit_mo2_profile_name");
    xedit_mo2_select_profile   = resolve<fn_xedit_mo2_select_profile>("xedit_mo2_select_profile");
    xedit_mo2_load_order       = resolve<fn_xedit_mo2_load_order>("xedit_mo2_load_order");

    // Record/Subrecord mutation
    xedit_set_subrecord_data    = resolve<fn_xedit_set_subrecord_data>("xedit_set_subrecord_data");
    xedit_delete_record         = resolve<fn_xedit_delete_record>("xedit_delete_record");
    xedit_copy_record           = resolve<fn_xedit_copy_record>("xedit_copy_record");
    xedit_add_record            = resolve<fn_xedit_add_record>("xedit_add_record");

    // LOD generation
    xedit_generate_lod         = resolve<fn_xedit_generate_lod>("xedit_generate_lod");
    xedit_lod_cancel           = resolve<fn_xedit_lod_cancel>("xedit_lod_cancel");

    return true;
}

// ---------------------------------------------------------------------------
// isLoaded()
// ---------------------------------------------------------------------------

bool XEditFFI::isLoaded() const
{
    return m_libHandle != nullptr;
}

// ---------------------------------------------------------------------------
// unload() -- close the library and null every pointer
// ---------------------------------------------------------------------------

void XEditFFI::unload()
{
    if (!m_libHandle)
        return;

    plat_dlclose(m_libHandle);
    m_libHandle = nullptr;

    // Session lifecycle
    xedit_init                  = nullptr;
    xedit_shutdown              = nullptr;

    // Plugin I/O
    xedit_load_plugin           = nullptr;
    xedit_save_plugin           = nullptr;

    // Plugin queries
    xedit_plugin_count          = nullptr;
    xedit_plugin_filename       = nullptr;
    xedit_plugin_record_count   = nullptr;
    xedit_plugin_master_count   = nullptr;
    xedit_plugin_master_name    = nullptr;
    xedit_plugin_group_count    = nullptr;
    xedit_plugin_load_order_id  = nullptr;

    // Group queries
    xedit_group_signature       = nullptr;
    xedit_group_name            = nullptr;
    xedit_group_record_count    = nullptr;
    xedit_group_form_ids        = nullptr;

    // Record queries
    xedit_record_editor_id      = nullptr;
    xedit_record_form_id        = nullptr;
    xedit_record_signature      = nullptr;
    xedit_record_subrecord_count = nullptr;

    // Version
    xedit_version               = nullptr;

    // Subrecord queries
    xedit_subrecord_signature   = nullptr;
    xedit_subrecord_size        = nullptr;
    xedit_subrecord_data        = nullptr;
    xedit_subrecord_display_value = nullptr;
    xedit_record_subrecords_batch = nullptr;

    // Search
    xedit_search_editor_id      = nullptr;
    xedit_search_form_id        = nullptr;

    // Conflict detection
    xedit_detect_conflicts      = nullptr;
    xedit_conflict_form_id      = nullptr;
    xedit_conflict_severity     = nullptr;
    xedit_conflict_plugin_count = nullptr;

    // ITM detection and cleaning
    xedit_detect_itm            = nullptr;
    xedit_clean_itm             = nullptr;
    xedit_clean_deleted         = nullptr;

    // Load order
    xedit_sort_load_order       = nullptr;

    // Form ID resolution and overrides
    xedit_resolve_form_id       = nullptr;
    xedit_find_overrides        = nullptr;
    xedit_find_overrides_full   = nullptr;

    // Conflict status
    xedit_record_conflict_status    = nullptr;
    xedit_subrecord_conflict_status = nullptr;

    // NIF (mesh) queries
    xedit_nif_block_count       = nullptr;
    xedit_nif_texture_count     = nullptr;
    xedit_nif_texture_path      = nullptr;

    // Asset scanning
    xedit_scan_assets           = nullptr;

    // Progress callback
    xedit_set_progress_callback = nullptr;

    // Referenced-by index
    xedit_build_refby_index       = nullptr;
    xedit_build_refby_index_async = nullptr;
    xedit_refby_build_status      = nullptr;
    xedit_record_refby_count      = nullptr;
    xedit_record_refby_entry      = nullptr;
    xedit_record_refby_batch      = nullptr;

    // Subrecord offloading
    xedit_offload_subrecords      = nullptr;

    // MO2 integration
    xedit_load_mo2             = nullptr;
    xedit_mo2_profile_count    = nullptr;
    xedit_mo2_profile_name     = nullptr;
    xedit_mo2_select_profile   = nullptr;
    xedit_mo2_load_order       = nullptr;

    // Record/Subrecord mutation
    xedit_set_subrecord_data    = nullptr;
    xedit_delete_record         = nullptr;
    xedit_copy_record           = nullptr;
    xedit_add_record            = nullptr;

    // LOD generation
    xedit_generate_lod         = nullptr;
    xedit_lod_cancel           = nullptr;

}
