# Terrain Texture LOD Generation — Status & Issues

## Overview

We are porting xLODGen's terrain LOD texture generation from compiled Delphi (xLODGenx64.exe) to Rust. The code lives in `rust-core/xedit_lod/src/terrain_lod.rs`. The goal is pixel-comparable output to xLODGen's reference for Fallout New Vegas (WastelandNV worldspace).

**Reference output:** `/home/luke/Games/VNV/mods/XLodGen Output/`
**Our test output:** `/home/luke/Games/VNV/mods/LOD GENERATION TEST/`
**Pascal source (no terrain tex code):** `/home/luke/Documents/Xedit Pascal Fork/TES5Edit-dev-4.1.6/Core/wbLOD.pas`
**Ghidra decompilation:** `/home/luke/Downloads/xLODGen/ghidra_out/xLODGenx64_allfunc_decomp.c`
**Settings:** `/home/luke/.local/share/fluorine/Prefix/pfx/drive_c/users/steamuser/AppData/Local/FalloutNV/Plugins.fnvviewsettings`

## How to Test

```bash
cd rust-core

# Regenerate all terrain textures (levels 4/8/16/32) — ~35s
cargo test -p xedit_lod --test integration test_terrain_texture_generation_level32 -- --nocapture

# Generate side-by-side comparison images (requires Python + PIL + ImageMagick)
# See bottom of this file for the comparison script

# Debug: find cells with dark VCLR
cargo test -p xedit_lod --test integration test_debug_vclr_wide -- --nocapture

# Debug: trace DLC texture mapping issues
cargo test -p xedit_lod --test integration test_debug_texture_mapping -- --nocapture

# Debug: trace per-plugin BTXT overrides for specific cells
cargo test -p xedit_lod --test integration test_debug_dlc_overrides -- --nocapture
```

All tests are in `rust-core/xedit_lod/tests/integration.rs` and require the local FNV game data + MO2 setup.

## Current Pipeline (our Rust code)

For each LOD block (e.g. 4x4 cells at level 4):

1. **Fill with default texture** — `Landscape\DirtWasteland01.dds` tiled across the block
2. **Paint BTXT base textures** — per-quadrant (4 quadrants per cell), hard fill
3. **Overlay ATXT alpha layers** — per-quadrant, using 33x33 alpha grids with square/diamond kernel convolution (matching xLODGen's Ghidra-decompiled algorithm)
4. **Apply VCLR vertex colors** — block-wide bilinear interpolation from 33x33 per-cell grids, formula: `pixel = pixel * (vclr / 255.0)` (confirmed via Ghidra: `TerrainVertexColorMultiplier=1.0` means pure multiplication)
5. **Horizontal flip** — FNV engine UV convention (confirmed empirically)
6. **DDS compress** — DXT1/DXT5 via `dds_util`

## Fixes Completed This Session

### 1. VCLR Dark Patches (LonesomeRoad.esm override)

**Problem:** Cells (-7,-1), (-6,-1), (-8,-1) were near-black (VCLR avg ~44/255).

**Root cause:** LonesomeRoad.esm overrides WastelandNV LAND records with very dark VCLR (avg 44) but **no BTXT**. Our merge logic preserved BTXT from the previous plugin (correct) but used LR's dark VCLR (wrong). The dark VCLR multiplied against the preserved textures → near-black output.

**Fix:** When merging LAND records, if a later plugin provides VCLR but has NO BTXT, and the previous plugin had both VCLR and BTXT, preserve the previous VCLR. Applied in both `scan_wrld_for_land` and `scan_world_children_for_land` (lines ~197-226 and ~286-310).

**Result:** Cell (-7,-1) VCLR: 44→97, Cell (-6,-1): 44→91, Cell (-8,-1): 44→197.

### 2. 33x33 Alpha Grid with Kernel Convolution

**Problem:** Hard cell/quadrant boundary artifacts in ATXT alpha blending.

**Root cause:** We used raw 17x17 VTXT grids with bilinear interpolation. xLODGen uses 3x3 convolution kernels stamped into a 33x33 grid.

**Fix:** New `build_alpha_grid_33()` function that maps each VTXT point at (col, row) in the 17x17 grid to position (col*2, row*2) in the 33x33 grid, then stamps a 3x3 kernel:
- **Even positions** → "square" kernel (all 9 neighbors)
- **Odd positions** → "diamond" kernel (center + 4 cardinal neighbors)

New `alpha_blend_texture_33()` uses the 33x33 grid for bilinear alpha sampling.

**Result:** Smoother ATXT layer transitions. Old `smooth_boundaries_crossfade` and `alpha_blend_texture_to_region` (17x17) removed.

### 3. DLC Texture Filtering (partial — see Current Issue #1)

**Problem:** Red/orange DLC textures (`NVDLC02_RockyGround.dds` from Honest Hearts) appearing in WastelandNV cells.

**Root cause:** DLC plugins (HonestHearts, OldWorldBlues) and mods that master them (YUP, Landscape Texture Improvements) override WastelandNV LAND records with BTXT FormIDs pointing to DLC LTEX records. Our code loads ALL BSAs (including `HonestHearts - Main.bsa`), so the DLC texture loads successfully. xLODGen only loads BSAs from the game INI `SArchiveList`, which excludes DLC BSAs — so DLC textures fail to load and fall back to the default.

**Fix attempted:** `is_dlc_texture_path()` filter in `preload_landscape_textures()` that skips paths containing `nvdlc` or `dlc`. This prevents the DLC texture from entering the texture cache, so the default fill should remain.

**Status:** Filter is in place but **red patches are still appearing in the output**. Needs debugging — the texture may be getting loaded through another path, or the BTXT lookup is falling through to a different texture.

## Current Issues (Unsolved)

### Issue 1: DLC Textures Still Appearing (RED/ORANGE PATCHES)

The rightmost column of the comparison grid (blocks x=0) still shows red/orange patches from `NVDLC02_RockyGround.dds`. The `is_dlc_texture_path` filter was added to `preload_landscape_textures` but the textures still appear in the output.

**Next steps to debug:**
- Add logging to `blend_terrain_block_cached` to check if `tex_cache.get(&btxt_path.to_lowercase())` returns Some or None for DLC paths
- Check if the DLC texture path is being lowercased consistently between preload and lookup
- The filter may need to also be applied at the BTXT painting step (not just preload) — skip painting when `is_dlc_texture_path(btxt_path)` is true
- Alternative approach: filter BSAs at the ResourceLoader level to only include base game BSAs (matching the game INI `SArchiveList`)

**Affected cells (examples):**
- (0,-7), (1,-4), (2,-2), (2,2), (2,7) — all have BTXT `0200B8A7` → `NVDLC02_RockyGround.dds`
- This FormID comes from plugins that master HonestHearts.esm (YUP, Landscape Texture Improvements)

### Issue 2: Cell/Quadrant Boundary Grid Lines

Hard rectangular edges visible between cells and quadrants where different BTXT base textures meet. The reference output has smooth transitions.

**Analysis:**
- BTXT base textures are painted as hard quadrant fills — this is correct per xLODGen's algorithm
- The smoothness in the reference comes from xLODGen's **premultiplied alpha compositing pipeline**: each cell is rendered to an intermediate bitmap, premultiplied by the 33x33 alpha grid, then AlphaBlended onto the block output
- Our variance is std=28-32 vs reference std=21 — we're ~50% more contrasty
- xLODGen also does texture tiling→resize (128 tex tiled 4x into 512, then resized back to 128) which inherently smooths

**Possible fixes:**
1. **Premultiplied alpha compositing** — Render each cell to a cell-sized intermediate bitmap, premultiply by combined alpha grid, then AlphaBlend onto the block. This is the closest match to xLODGen's actual algorithm.
2. **Post-compositing blur** — Apply a box blur (radius 2-3, 1-2 passes) after painting but before VCLR. Simple but approximate. A `box_blur_rgba` function was written and tested but removed as it didn't sufficiently address the boundaries.
3. **Texture tiling+downsample** — Tile each texture NxN and resize back to match xLODGen's tiling step. This creates inherent blurring.

### Issue 3: Remaining Dark VCLR Cells

24 cells have VCLR avg < 80 with legitimate BTXT (not DLC override artifacts). These are genuinely dark cells in the game data. The reference output also shows dark areas for these cells, but with smoother gradients. Our hard cell boundaries make the dark patches look like obvious rectangles.

**Most affected blocks:** (8,24), (32,24), (36,24), (-24,-16)

**Fix:** This resolves naturally once Issue #2 (boundary smoothing) is fixed — the dark VCLR gradients would blend smoothly across cell boundaries instead of showing hard rectangles.

## xLODGen Algorithm (from Ghidra Decompilation)

Key functions in `xLODGenx64_allfunc_decomp.c`:

| Function | Line | Purpose |
|----------|------|---------|
| `FUN_01b96440` | 1894325 | Texture loader with tiling — loads texture, resizes to TerrainDefaultDiffuseSize, tiles NxN, resizes to output |
| `FUN_01b9cb10` | 1896913 | Per-cell renderer — creates cell bitmap, fills with texture, builds 33x33 alpha grid with square/diamond kernels, premultiplies, AlphaBlends onto output |
| `FUN_01b9c580` | 1896680 | Premultiplied alpha application — multiplies cell bitmap RGB by resized alpha grid |
| `FUN_01b9c810` | 1896790 | AlphaBlend compositing — composites premultiplied cell bitmap onto block output per quadrant |
| `FUN_01b9c780` | 1896766 | Quadrant offset calculator — maps quadrant 0-3 to pixel offsets |
| `FUN_01b9e910` | (referenced) | VCLR bitmap creation from raw 33x33x3 data |
| `FUN_01b9e2a0` | (referenced) | VCLR resize and apply to output |
| `FUN_01b9e190` | (referenced) | VCLR blend formula: `result = (vclr*tex)*(1-alpha) + tex*alpha` where `alpha = round((1-TerrainVertexColorMultiplier)*255)` |

**Per-cell rendering flow (FUN_01b9cb10):**
1. Create cell-sized bitmap (TerrainDefaultDiffuseSize × TerrainDefaultDiffuseSize = 128×128)
2. Fill with default landscape texture
3. If VTXT data exists: build 33×33 alpha grid from VTXT entries with 3×3 kernels
4. Premultiply cell bitmap by alpha grid (makes un-textured areas transparent)
5. AlphaBlend the premultiplied cell onto the block output per quadrant

**Higher-level flow (FUN_01b9d740):**
1. Loop over 4 quadrants (0-3)
2. For each quadrant, loop over layers (BTXT base, then ATXT overlays)
3. Call FUN_01b9cb10 for each layer
4. After quadrant loop: composite intermediate cell bitmap onto block
5. Apply VCLR: build 33×33 VCLR bitmap, resize, multiply onto output

## Key Technical Details

- **TerrainDefaultDiffuseSize:** 128 (from settings file)
- **TerrainVertexColorMultiplier:** 1.00 → alpha=0 → VCLR formula = `pixel * vclr/255`
- **VCLR convention:** /255.0 (255=neutral 1.0x)
- **FNV horizontal flip:** Required for engine UV convention
- **Texture downsample:** `(tex_size / lod_level).max(32).min(128)` — currently 128 for level 4
- **Game INI SArchiveList:** `Fallout - Textures.bsa, Fallout - Textures2.bsa, Fallout - Meshes.bsa, ...` (NO DLC BSAs)

## Comparison Script

```python
from PIL import Image, ImageDraw, ImageFont
import subprocess, os

def dds_to_png(dds_path, png_path):
    subprocess.run(['magick', dds_path, png_path], check=True, capture_output=True)

ref_base = '/home/luke/Games/VNV/mods/XLodGen Output/textures/landscape/lod/wastelandnv/diffuse'
our_base = '/home/luke/Games/VNV/mods/LOD GENERATION TEST/textures/landscape/lod/wastelandnv/diffuse'

blocks_x = [-12, -8, -4, 0]
blocks_y = [-8, -4, 0, 4]
tile_size = 256; gap = 4; label_h = 30
grid_w = len(blocks_x) * (tile_size + gap) - gap
grid_h = len(blocks_y) * (tile_size + gap) - gap

full = Image.new('RGB', (grid_w * 2 + 40, grid_h + label_h + 10), (30, 30, 30))
draw = ImageDraw.Draw(full)
font = ImageFont.truetype('/usr/share/fonts/noto/NotoSans-Regular.ttf', 18)
sfont = ImageFont.truetype('/usr/share/fonts/noto/NotoSans-Regular.ttf', 11)

draw.text((10, 5), 'Reference (xLODGen)', fill='white', font=font)
draw.text((grid_w + 50, 5), 'Ours (Rust)', fill='white', font=font)

for col, bx in enumerate(blocks_x):
    for row, by in enumerate(blocks_y):
        fname = f'wastelandnv.n.level4.x{bx}.y{by}.dds'
        px = col * (tile_size + gap)
        py = row * (tile_size + gap) + label_h
        for side, base in [(0, ref_base), (1, our_base)]:
            dds = os.path.join(base, fname)
            if not os.path.exists(dds): continue
            png = f'/tmp/tile_{side}_{bx}_{by}.png'
            dds_to_png(dds, png)
            img = Image.open(png).resize((tile_size, tile_size), Image.LANCZOS)
            offset_x = side * (grid_w + 40)
            full.paste(img, (offset_x + px, py))
            draw.text((offset_x + px + 2, py + 2), f'{bx},{by}', fill='yellow', font=sfont)

full.save('/tmp/terrain_grid_compare.png')
```
