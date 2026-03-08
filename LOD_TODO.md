# LOD Port — Working TODO

**Reference:** `/home/luke/Games/VNV/mods/XLodGen Output/`
**Output:** `/home/luke/Games/VNV/mods/LOD GENERATION TEST/`
**Game:** FNV, WastelandNV, MO2 at `/home/luke/Games/VNV/`
**Settings:** `/home/luke/.local/share/fluorine/Prefix/pfx/drive_c/users/steamuser/AppData/Local/FalloutNV/Plugins.fnvviewsettings`

Format codes: 200=DXT1, 202=DXT5, 205=BC5

---

## Phase 0: Crate Setup + Prerequisites — DONE

- [x] xedit_lod crate with all dependencies
- [x] ResourceLoader (BSA + loose, 189K entries, tested)
- [x] LOD settings parser (.dlodsettings, tested)
- [x] DDS utilities (image_dds + directxtex, compress/decompress/composite)
- [x] Bin packer (BSP with auto-grow, 5 unit tests)
- [x] Settings file parser (Plugins.fnvviewsettings, tested)
- [x] Progress reporter (thread-safe, C FFI callback)

## Phase 1: Tree LOD — DONE (pipeline working, needs full load order)

- [x] Reference scanner: find STAT/ACTI/TREE with HasTreeLOD (0x0040)
- [x] Billboard path generation matching Delphi `BillboardFileName`
- [x] Billboard .txt config parsing (Width, Height, ShiftX/Y/Z, Scale)
- [x] Atlas builder: load billboards, bin-pack, composite, compress DXT5
- [x] Normal map atlas support
- [x] CRC32 dedup of duplicate billboard textures
- [x] Worldspace REFR scanning from loaded plugins
- [x] REFR filtering (disabled, deleted, enable parent, fallen trees)
- [x] DTL file generation (per-cell, grouped by tree type)
- [x] LST file generation (tree type list with atlas UV coords)
- [x] Tree atlas DDS output (diffuse + normal)
- [x] End-to-end integration test (loads FalloutNV.esm, generates output)

## Phase 2: Object LOD — DONE (580/580 NIF meshes validated)

- [x] Object base scanning (STAT with HasDistantLOD 0x8000 + MNAM)
- [x] LOD texture collection from meshes
- [x] Object texture atlas builder (multi-page, 4096x4096)
- [x] LODGen.txt export writer (native paths)
- [x] Atlas map file writer (tab-separated)
- [x] LODGen.exe ported to .NET 10 (native Linux, no Wine needed!)
- [x] NIF combiner module (calls native LODGen via dotnet)
- [x] LODGen.exe Linux path fix (MO2 mod dir search, case-insensitive, no fatal exit)
- [x] Validate: 580 NIF files matching reference (all filenames match, ~11s generation)
- [ ] Validate: 12 atlas DDS files matching reference

**Note:** LODGen.exe was decompiled from .NET Framework 4.8 and ported to .NET 10.
Builds and runs natively on Linux with `dotnet run`. Located at `lodgen-dotnet/`.

---

## Phase 3: Terrain LOD (~3400 DDS + ~1420 NIF) — DONE

### 3.1 — LAND record parsing — DONE
- [x] Parse VHGT heightmap (33×33 per cell, 16386 cells from FalloutNV.esm)
- [x] Parse VNML vertex normals (all 16386 cells)
- [x] Parse VCLR vertex colors (14158/16386 cells)
- [x] Parse ATXT/BTXT landscape texture layers
- [x] Parse LTEX/TXST landscape texture names
- [x] Write binary terrain data file (matches LODGen format, 1112 bytes/cell)
- [x] Write LODGen terrain config file (GameMode=TERRAINFNV)
- [x] Integration test: scan 16386 cells, write binary, compare with reference (5 cell diff)

### 3.2 — Terrain mesh generation (1420 NIF: 1056+272+72+20) — DONE
- [x] Terrain mesh generation delegated to LODGen.exe (TerrainLOD mode)
- [x] Run LODGen in terrain mode with reference binary data (~10s generation)
- [x] Validate: 1420 NIF files byte-for-byte identical to reference

### 3.3 — Terrain texture generation (3400 DDS: 1700 diffuse + 1700 normal) — DONE
- [x] Basic terrain texture blending (vertex colors + normals)
- [x] Full landscape texture blending (LTEX/ATXT layers with alpha)
- [x] Write DXT1 512×512: diffuse + normal per LOD block
- [x] Validate: 1700 diffuse + 1700 normal DDS files (all filenames match, ~83s generation)

---

## Phase 4: FFI + GUI

### 4.1 — FFI functions
- [ ] xedit_lod_list_worldspaces
- [ ] xedit_lod_generate / export / status / error / cancel
- [ ] Progress callback

### 4.2 — GUI
- [ ] MainWindow menu item
- [ ] LODGenDialog → FFI wiring
- [ ] xLODGen standalone mode

---

## Module Summary (20 unit tests, 8 integration tests)

| Module | Status | Tests |
|--------|--------|-------|
| lib.rs (LodOptions, DdsFormat) | Done | 0 |
| resource_loader.rs | Done | 2 integration |
| lod_settings.rs | Done | 3 unit |
| bin_packer.rs | Done | 4 unit |
| dds_util.rs | Done | 0 (used by atlas_builder) |
| trees_lod.rs | Done | 5 unit |
| atlas_builder.rs | Done | 0 (tested via integration) |
| reference_scanner.rs | Done | 3 unit |
| export_writer.rs | Done | 3 unit |
| nif_combiner.rs | Done | 0 |
| objects_lod.rs | Done | 0 (needs integration test) |
| terrain_lod.rs | Done | 2 unit + 2 integration |
| progress.rs | Done | 0 |

## Reference File Counts

| Category | Reference | Status |
|----------|-----------|--------|
| DTL files | 58 | Phase 1 ✓ (needs full load order) |
| LST files | 1 (68 bytes) | Phase 1 ✓ |
| Tree atlas DDS | 2 | Phase 1 ✓ |
| Building atlas DDS | 12 | Phase 2 (pipeline ready) |
| Object NIF meshes | 580 | Phase 2 ✓ (580/580 filenames match) |
| Terrain meshes (NIF) | 1420 | Phase 3 ✓ (1420/1420 byte-identical) |
| Terrain diffuse DDS | 1700 | Phase 3 ✓ (1700/1700 filenames match) |
| Terrain normal DDS | 1700 | Phase 3 ✓ (1700/1700 filenames match) |
| **Total** | **5473** | |
