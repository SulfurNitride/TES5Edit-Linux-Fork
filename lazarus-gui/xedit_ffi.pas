{ xedit_ffi.pas - Pascal declarations for the xedit_core Rust FFI library.

  All functions use cdecl calling convention.
  String output uses caller-allocated buffers (PChar + buffer length).
  Return values: >= 0 on success (often bytes written), < 0 on error.

  The library is loaded dynamically at runtime via dynlibs so the GUI
  can compile and link even when libxedit_core is not yet built.
  Call xedit_ffi_load to load the library, xedit_ffi_loaded to check.

  Error codes:
    XEDIT_OK             =  0
    XEDIT_ERR_NULL_HANDLE = -1
    XEDIT_ERR_INVALID_PATH = -2
    XEDIT_ERR_LOAD_FAILED = -3
    XEDIT_ERR_SAVE_FAILED = -4
    XEDIT_ERR_NIFLY_MISSING = -5
    XEDIT_ERR_INVALID_GAME = -6
    XEDIT_ERR_PANIC       = -99
}
unit xedit_ffi;

{$mode objfpc}{$H+}

interface

uses
  SysUtils, dynlibs;

const
  XEDIT_OK              =  0;
  XEDIT_ERR_NULL_HANDLE = -1;
  XEDIT_ERR_INVALID_PATH = -2;
  XEDIT_ERR_LOAD_FAILED = -3;
  XEDIT_ERR_SAVE_FAILED = -4;
  XEDIT_ERR_NIFLY_MISSING = -5;
  XEDIT_ERR_INVALID_GAME = -6;
  XEDIT_ERR_PANIC       = -99;

  { Library file name (platform-dependent) }
  {$IFDEF WINDOWS}
  XEDIT_CORE_LIB = 'xedit_core.dll';
  {$ELSE}
  XEDIT_CORE_LIB = 'libxedit_core.so';
  {$ENDIF}

{ Progress callback type: procedure(message: PChar; progress: Double) }
type
  TXEditProgressCallback = procedure(message: PChar; progress: Double); cdecl;

{ ========================================================================== }
{ Function pointer types                                                       }
{ ========================================================================== }
type
  Txedit_init = function(game_name: PChar; data_path: PChar; progress: Pointer): Integer; cdecl;
  Txedit_shutdown = function(): Integer; cdecl;
  Txedit_load_plugin = function(file_path: PChar): Integer; cdecl;
  Txedit_save_plugin = function(plugin_index: Integer; file_path: PChar): Integer; cdecl;
  Txedit_plugin_count = function(): Integer; cdecl;
  Txedit_plugin_filename = function(plugin_index: Integer;
    buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_plugin_record_count = function(plugin_index: Integer): Integer; cdecl;
  Txedit_plugin_master_count = function(plugin_index: Integer): Integer; cdecl;
  Txedit_plugin_master_name = function(plugin_index: Integer; master_index: Integer;
    buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_version = function(buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_plugin_group_count = function(plugin_idx: Integer): Integer; cdecl;
  Txedit_group_signature = function(plugin_idx: Integer; group_idx: Integer;
    buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_group_name = function(plugin_idx: Integer; group_idx: Integer;
    buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_group_record_count = function(plugin_idx: Integer; group_idx: Integer): Integer; cdecl;
  Txedit_record_editor_id = function(plugin_idx: Integer; group_idx: Integer;
    record_idx: Integer; buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_record_form_id = function(plugin_idx: Integer; group_idx: Integer;
    record_idx: Integer): Cardinal; cdecl;
  Txedit_record_signature = function(plugin_idx: Integer; group_idx: Integer;
    record_idx: Integer; buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_record_subrecord_count = function(plugin_idx: Integer; group_idx: Integer;
    record_idx: Integer): Integer; cdecl;
  Txedit_subrecord_signature = function(plugin_idx: Integer; group_idx: Integer;
    record_idx: Integer; sub_idx: Integer;
    buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_subrecord_size = function(plugin_idx: Integer; group_idx: Integer;
    record_idx: Integer; sub_idx: Integer): Integer; cdecl;
  Txedit_subrecord_data = function(plugin_idx: Integer; group_idx: Integer;
    record_idx: Integer; sub_idx: Integer;
    buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_search_editor_id = function(plugin_idx: Integer; query: PChar;
    results_buf: PInteger; max_results: Integer): Integer; cdecl;
  Txedit_search_form_id = function(plugin_idx: Integer; form_id: Cardinal;
    out_group_idx: PInteger; out_record_idx: PInteger): Integer; cdecl;

  { Conflict detection }
  Txedit_detect_conflicts = function(handle: Pointer): Integer; cdecl;
  Txedit_conflict_form_id = function(handle: Pointer; conflict_index: Integer): Cardinal; cdecl;
  Txedit_conflict_severity = function(handle: Pointer; conflict_index: Integer): Integer; cdecl;
  Txedit_conflict_plugin_count = function(handle: Pointer; conflict_index: Integer): Integer; cdecl;
  Txedit_detect_itm = function(handle: Pointer; plugin_index: Integer;
    buf: PCardinal; buf_len: Integer): Integer; cdecl;

  { Load order }
  Txedit_sort_load_order = function(handle: Pointer): Integer; cdecl;
  Txedit_resolve_form_id = function(handle: Pointer; plugin_index: Integer;
    raw_form_id: Cardinal): Cardinal; cdecl;
  Txedit_find_overrides = function(handle: Pointer; form_id: Cardinal;
    buf: PInteger; buf_len: Integer): Integer; cdecl;

  { Progress callback }
  Txedit_set_progress_callback = function(handle: Pointer;
    callback: TXEditProgressCallback): Integer; cdecl;

  { NIF operations }
  Txedit_nif_block_count = function(handle: Pointer; path: PChar): Integer; cdecl;
  Txedit_nif_texture_count = function(handle: Pointer; path: PChar): Integer; cdecl;
  Txedit_nif_texture_path = function(handle: Pointer; path: PChar;
    index: Integer; buf: PChar; buf_len: Integer): Integer; cdecl;

  { Unified tools }
  Txedit_scan_assets = function(handle: Pointer; plugin_index: Integer;
    buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_clean_itm = function(handle: Pointer;
    plugin_index: Integer): Integer; cdecl;
  Txedit_clean_deleted = function(handle: Pointer;
    plugin_index: Integer): Integer; cdecl;

  { Referenced By }
  Txedit_build_refby_index = function: Int32; cdecl;
  Txedit_build_refby_index_async = function: Int32; cdecl;
  Txedit_refby_build_status = function: Int32; cdecl;
  Txedit_record_refby_count = function(plugin_idx, group_idx, record_idx: Int32): Int32; cdecl;
  Txedit_record_refby_entry = function(plugin_idx, group_idx, record_idx, ref_index: Int32;
    out_plugin_idx, out_group_idx, out_record_idx: PInt32): Int32; cdecl;

  { MO2 integration }
  Txedit_load_mo2 = function(handle: Pointer; mo2_folder_path: PChar): Integer; cdecl;
  Txedit_mo2_profile_count = function(handle: Pointer): Integer; cdecl;
  Txedit_mo2_profile_name = function(handle: Pointer; index: Integer;
    buf: PChar; buf_len: Integer): Integer; cdecl;
  Txedit_mo2_select_profile = function(handle: Pointer; profile_name: PChar): Integer; cdecl;
  Txedit_mo2_load_order = function(handle: Pointer): Integer; cdecl;

{ ========================================================================== }
{ Function pointer variables (set by xedit_ffi_load)                           }
{ ========================================================================== }
var
  xedit_init: Txedit_init = nil;
  xedit_shutdown: Txedit_shutdown = nil;
  xedit_load_plugin: Txedit_load_plugin = nil;
  xedit_save_plugin: Txedit_save_plugin = nil;
  xedit_plugin_count: Txedit_plugin_count = nil;
  xedit_plugin_filename: Txedit_plugin_filename = nil;
  xedit_plugin_record_count: Txedit_plugin_record_count = nil;
  xedit_plugin_master_count: Txedit_plugin_master_count = nil;
  xedit_plugin_master_name: Txedit_plugin_master_name = nil;
  xedit_version: Txedit_version = nil;
  xedit_plugin_group_count: Txedit_plugin_group_count = nil;
  xedit_group_signature: Txedit_group_signature = nil;
  xedit_group_name: Txedit_group_name = nil;
  xedit_group_record_count: Txedit_group_record_count = nil;
  xedit_record_editor_id: Txedit_record_editor_id = nil;
  xedit_record_form_id: Txedit_record_form_id = nil;
  xedit_record_signature: Txedit_record_signature = nil;
  xedit_record_subrecord_count: Txedit_record_subrecord_count = nil;
  xedit_subrecord_signature: Txedit_subrecord_signature = nil;
  xedit_subrecord_size: Txedit_subrecord_size = nil;
  xedit_subrecord_data: Txedit_subrecord_data = nil;
  xedit_search_editor_id: Txedit_search_editor_id = nil;
  xedit_search_form_id: Txedit_search_form_id = nil;

  { Conflict detection }
  xedit_detect_conflicts: Txedit_detect_conflicts = nil;
  xedit_conflict_form_id: Txedit_conflict_form_id = nil;
  xedit_conflict_severity: Txedit_conflict_severity = nil;
  xedit_conflict_plugin_count: Txedit_conflict_plugin_count = nil;
  xedit_detect_itm: Txedit_detect_itm = nil;

  { Load order }
  xedit_sort_load_order: Txedit_sort_load_order = nil;
  xedit_resolve_form_id: Txedit_resolve_form_id = nil;
  xedit_find_overrides: Txedit_find_overrides = nil;

  { Progress callback }
  xedit_set_progress_callback: Txedit_set_progress_callback = nil;

  { NIF operations }
  xedit_nif_block_count: Txedit_nif_block_count = nil;
  xedit_nif_texture_count: Txedit_nif_texture_count = nil;
  xedit_nif_texture_path: Txedit_nif_texture_path = nil;

  { Unified tools }
  xedit_scan_assets: Txedit_scan_assets = nil;
  xedit_clean_itm: Txedit_clean_itm = nil;
  xedit_clean_deleted: Txedit_clean_deleted = nil;

  { Referenced By }
  xedit_build_refby_index: Txedit_build_refby_index = nil;
  xedit_build_refby_index_async: Txedit_build_refby_index_async = nil;
  xedit_refby_build_status: Txedit_refby_build_status = nil;
  xedit_record_refby_count: Txedit_record_refby_count = nil;
  xedit_record_refby_entry: Txedit_record_refby_entry = nil;

  { MO2 integration }
  xedit_load_mo2: Txedit_load_mo2 = nil;
  xedit_mo2_profile_count: Txedit_mo2_profile_count = nil;
  xedit_mo2_profile_name: Txedit_mo2_profile_name = nil;
  xedit_mo2_select_profile: Txedit_mo2_select_profile = nil;
  xedit_mo2_load_order: Txedit_mo2_load_order = nil;

{ ========================================================================== }
{ Library management                                                           }
{ ========================================================================== }

{ Load the xedit_core shared library. Returns True on success.
  ALibPath can override the default library name/path. }
function xedit_ffi_load(const ALibPath: string = ''): Boolean;

{ Returns True if the library has been loaded successfully. }
function xedit_ffi_loaded: Boolean;

{ Unload the library and nil all function pointers. }
procedure xedit_ffi_unload;

implementation

var
  FLibHandle: TLibHandle = NilHandle;

function xedit_ffi_loaded: Boolean;
begin
  Result := FLibHandle <> NilHandle;
end;

function xedit_ffi_load(const ALibPath: string): Boolean;
var
  Lib: string;
begin
  Result := False;

  if FLibHandle <> NilHandle then
  begin
    Result := True;
    Exit;
  end;

  if ALibPath <> '' then
    Lib := ALibPath
  else
    Lib := XEDIT_CORE_LIB;

  FLibHandle := LoadLibrary(Lib);
  if FLibHandle = NilHandle then
    Exit;

  { Resolve all symbols. We use GetProcedureAddress which returns nil
    if not found; callers should check xedit_ffi_loaded before calling. }
  Pointer(xedit_init) := GetProcedureAddress(FLibHandle, 'xedit_init');
  Pointer(xedit_shutdown) := GetProcedureAddress(FLibHandle, 'xedit_shutdown');
  Pointer(xedit_load_plugin) := GetProcedureAddress(FLibHandle, 'xedit_load_plugin');
  Pointer(xedit_save_plugin) := GetProcedureAddress(FLibHandle, 'xedit_save_plugin');
  Pointer(xedit_plugin_count) := GetProcedureAddress(FLibHandle, 'xedit_plugin_count');
  Pointer(xedit_plugin_filename) := GetProcedureAddress(FLibHandle, 'xedit_plugin_filename');
  Pointer(xedit_plugin_record_count) := GetProcedureAddress(FLibHandle, 'xedit_plugin_record_count');
  Pointer(xedit_plugin_master_count) := GetProcedureAddress(FLibHandle, 'xedit_plugin_master_count');
  Pointer(xedit_plugin_master_name) := GetProcedureAddress(FLibHandle, 'xedit_plugin_master_name');
  Pointer(xedit_version) := GetProcedureAddress(FLibHandle, 'xedit_version');
  Pointer(xedit_plugin_group_count) := GetProcedureAddress(FLibHandle, 'xedit_plugin_group_count');
  Pointer(xedit_group_signature) := GetProcedureAddress(FLibHandle, 'xedit_group_signature');
  Pointer(xedit_group_name) := GetProcedureAddress(FLibHandle, 'xedit_group_name');
  Pointer(xedit_group_record_count) := GetProcedureAddress(FLibHandle, 'xedit_group_record_count');
  Pointer(xedit_record_editor_id) := GetProcedureAddress(FLibHandle, 'xedit_record_editor_id');
  Pointer(xedit_record_form_id) := GetProcedureAddress(FLibHandle, 'xedit_record_form_id');
  Pointer(xedit_record_signature) := GetProcedureAddress(FLibHandle, 'xedit_record_signature');
  Pointer(xedit_record_subrecord_count) := GetProcedureAddress(FLibHandle, 'xedit_record_subrecord_count');
  Pointer(xedit_subrecord_signature) := GetProcedureAddress(FLibHandle, 'xedit_subrecord_signature');
  Pointer(xedit_subrecord_size) := GetProcedureAddress(FLibHandle, 'xedit_subrecord_size');
  Pointer(xedit_subrecord_data) := GetProcedureAddress(FLibHandle, 'xedit_subrecord_data');
  Pointer(xedit_search_editor_id) := GetProcedureAddress(FLibHandle, 'xedit_search_editor_id');
  Pointer(xedit_search_form_id) := GetProcedureAddress(FLibHandle, 'xedit_search_form_id');

  { Conflict detection }
  Pointer(xedit_detect_conflicts) := GetProcedureAddress(FLibHandle, 'xedit_detect_conflicts');
  Pointer(xedit_conflict_form_id) := GetProcedureAddress(FLibHandle, 'xedit_conflict_form_id');
  Pointer(xedit_conflict_severity) := GetProcedureAddress(FLibHandle, 'xedit_conflict_severity');
  Pointer(xedit_conflict_plugin_count) := GetProcedureAddress(FLibHandle, 'xedit_conflict_plugin_count');
  Pointer(xedit_detect_itm) := GetProcedureAddress(FLibHandle, 'xedit_detect_itm');

  { Load order }
  Pointer(xedit_sort_load_order) := GetProcedureAddress(FLibHandle, 'xedit_sort_load_order');
  Pointer(xedit_resolve_form_id) := GetProcedureAddress(FLibHandle, 'xedit_resolve_form_id');
  Pointer(xedit_find_overrides) := GetProcedureAddress(FLibHandle, 'xedit_find_overrides');

  { Progress callback }
  Pointer(xedit_set_progress_callback) := GetProcedureAddress(FLibHandle, 'xedit_set_progress_callback');

  { NIF operations }
  Pointer(xedit_nif_block_count) := GetProcedureAddress(FLibHandle, 'xedit_nif_block_count');
  Pointer(xedit_nif_texture_count) := GetProcedureAddress(FLibHandle, 'xedit_nif_texture_count');
  Pointer(xedit_nif_texture_path) := GetProcedureAddress(FLibHandle, 'xedit_nif_texture_path');

  { Unified tools }
  Pointer(xedit_scan_assets) := GetProcedureAddress(FLibHandle, 'xedit_scan_assets');
  Pointer(xedit_clean_itm) := GetProcedureAddress(FLibHandle, 'xedit_clean_itm');
  Pointer(xedit_clean_deleted) := GetProcedureAddress(FLibHandle, 'xedit_clean_deleted');

  { Referenced By }
  Pointer(xedit_build_refby_index) := GetProcedureAddress(FLibHandle, 'xedit_build_refby_index');
  Pointer(xedit_build_refby_index_async) := GetProcedureAddress(FLibHandle, 'xedit_build_refby_index_async');
  Pointer(xedit_refby_build_status) := GetProcedureAddress(FLibHandle, 'xedit_refby_build_status');
  Pointer(xedit_record_refby_count) := GetProcedureAddress(FLibHandle, 'xedit_record_refby_count');
  Pointer(xedit_record_refby_entry) := GetProcedureAddress(FLibHandle, 'xedit_record_refby_entry');

  { MO2 integration }
  Pointer(xedit_load_mo2) := GetProcedureAddress(FLibHandle, 'xedit_load_mo2');
  Pointer(xedit_mo2_profile_count) := GetProcedureAddress(FLibHandle, 'xedit_mo2_profile_count');
  Pointer(xedit_mo2_profile_name) := GetProcedureAddress(FLibHandle, 'xedit_mo2_profile_name');
  Pointer(xedit_mo2_select_profile) := GetProcedureAddress(FLibHandle, 'xedit_mo2_select_profile');
  Pointer(xedit_mo2_load_order) := GetProcedureAddress(FLibHandle, 'xedit_mo2_load_order');

  Result := True;
end;

procedure xedit_ffi_unload;
begin
  if FLibHandle <> NilHandle then
  begin
    UnloadLibrary(FLibHandle);
    FLibHandle := NilHandle;
  end;

  xedit_init := nil;
  xedit_shutdown := nil;
  xedit_load_plugin := nil;
  xedit_save_plugin := nil;
  xedit_plugin_count := nil;
  xedit_plugin_filename := nil;
  xedit_plugin_record_count := nil;
  xedit_plugin_master_count := nil;
  xedit_plugin_master_name := nil;
  xedit_version := nil;
  xedit_plugin_group_count := nil;
  xedit_group_signature := nil;
  xedit_group_name := nil;
  xedit_group_record_count := nil;
  xedit_record_editor_id := nil;
  xedit_record_form_id := nil;
  xedit_record_signature := nil;
  xedit_record_subrecord_count := nil;
  xedit_subrecord_signature := nil;
  xedit_subrecord_size := nil;
  xedit_subrecord_data := nil;
  xedit_search_editor_id := nil;
  xedit_search_form_id := nil;

  { Conflict detection }
  xedit_detect_conflicts := nil;
  xedit_conflict_form_id := nil;
  xedit_conflict_severity := nil;
  xedit_conflict_plugin_count := nil;
  xedit_detect_itm := nil;

  { Load order }
  xedit_sort_load_order := nil;
  xedit_resolve_form_id := nil;
  xedit_find_overrides := nil;

  { Progress callback }
  xedit_set_progress_callback := nil;

  { NIF operations }
  xedit_nif_block_count := nil;
  xedit_nif_texture_count := nil;
  xedit_nif_texture_path := nil;

  { Unified tools }
  xedit_scan_assets := nil;
  xedit_clean_itm := nil;
  xedit_clean_deleted := nil;

  { Referenced By }
  xedit_build_refby_index := nil;
  xedit_build_refby_index_async := nil;
  xedit_refby_build_status := nil;
  xedit_record_refby_count := nil;
  xedit_record_refby_entry := nil;

  { MO2 integration }
  xedit_load_mo2 := nil;
  xedit_mo2_profile_count := nil;
  xedit_mo2_profile_name := nil;
  xedit_mo2_select_profile := nil;
  xedit_mo2_load_order := nil;
end;

finalization
  xedit_ffi_unload;

end.
