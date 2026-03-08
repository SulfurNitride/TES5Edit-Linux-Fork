# Terrain LOD Texture Blending — Current Issues & Missing Pieces

## The Problem

Terrain LOD textures still show visible rectangular block boundaries in-game, especially at LOD level 4 (closest terrain). The textures within each quadrant/cell look correct individually, but the **transitions between them are hard edges** instead of smooth gradients. The reference xLODGen output has smooth, blended transitions.

## Current Pipeline (Rust — `terrain_lod.rs`)

```
For each LOD block (step×step cells):
  1. Fill block canvas with default texture
  2. For each cell in the block:
     a. Create 128×128 compositing canvas (comp_cell_size)
     b. Fill with default texture
     c. BTXT pass: tile each quadrant's base texture into its 64×64 region
     d. ATXT pass: for each alpha layer:
        - Build 33×33 mask from VTXT stamps
        - Embed into 65×65 cell-wide grid at quadrant position
        - Lanczos3 upscale 65×65 → 128×128
        - Composite texture with cell-wide mask (bilinear-tiled)
     e. Area-downsample 128→pixels_per_cell if needed
     f. Copy into block canvas
  3. Apply VCLR block-wide (bilinear interpolation, /255 multiply)
  4. Horizontal flip for FNV engine
  5. DXT1 compress → DDS
```

## What xLODGen Does Differently (from decompilation)

### Source Files
- Decompiled reconstruction: `/home/luke/Downloads/xLODGen/decomp_xlodgen_x64/recon/src/`
- Key file: `uTerrainCellCompositor.pas` — cell-level compositing
- Key file: `uTerrainCompositor.pas` — block-level atlas assembly

### Critical Difference #1: BaseDiffuse is Pre-Built Externally

In xLODGen, `ComposeCellEx()` receives `Data.BaseDiffuse` — the cell's base texture with **all 4 quadrants already composited into a single image**. This image is the **output resolution** (e.g., 512×512 for a single cell in a level-4 block, or 128×128 if that's what the settings dictate).

**We don't know how BaseDiffuse is constructed.** The reconstruction code (`uTerrainCellCompositor.pas`) doesn't contain the BaseDiffuse builder — it's done upstream in code we haven't decompiled. The key unknowns:
- What resolution is BaseDiffuse? (128? 256? 512? output cell size?)
- How are the 4 BTXT textures tiled/placed into BaseDiffuse?
- Is there any blending/feathering between adjacent BTXT quadrants in BaseDiffuse itself?

### Critical Difference #2: Layer Texture Size ≠ Cell Size

In xLODGen's `ComposeCellEx()`:
```pascal
// Mask is resized to the LAYER TEXTURE size, not the cell size
MaskTex := ResampleMaskToTexture(Mask33, L.Diffuse.Width, L.Diffuse.Height);
// Quadrant offset is based on the OUTPUT cell size
ComputeQuadrantOffset(L.Quadrant, Result.Diffuse.Width, Result.Diffuse.Height, OffX, OffY);
// Composite the layer-sized texture at the cell-sized offset
CompositePremultiplied(Result.Diffuse, LayerDiffuse, OffX, OffY);
```

The mask+texture is `L.Diffuse.Width × L.Diffuse.Height` — the **layer's texture** size. The offset is `Result.Diffuse.Width / 2` — **half the output cell**. If these differ (e.g., layer texture is 128×128 but output cell is 256×256), the texture only covers **part of the quadrant**, and the mask feathers within the layer size, not the cell size.

**We assume** layer texture = texture cache = 128×128, and output cell = 128×128, making offset = 64 and the texture overshooting its quadrant. But **we don't actually know** what size `L.Diffuse` is in the real pipeline. If `L.Diffuse` is pre-tiled to the output quadrant size (e.g., 256×256), the compositing math changes completely.

### Critical Difference #3: VCLR Uses Lanczos3 Upscale

xLODGen's `ApplyVCLR()`:
```pascal
// Build 33×33 VCLR image from vertex colors
VCLRImg := NewImage(33, 33);
// Lanczos3 upscale to (W * 33/32, H * 33/32)
UpW := Round(Diffuse.Width * 33.0 / 32.0);
UpH := Round(Diffuse.Height * 33.0 / 32.0);
Upscaled := ResizeLanczos3(VCLRImg, UpW, UpH);
// Center-crop and multiply
OffX := (UpW - Diffuse.Width) div 2;
OffY := (UpH - Diffuse.Height) div 2;
// Per-pixel: result = tex * lerp(vclr, 1.0, VAlpha)
// Where VAlpha = 1.0 - Multiplier
```

**Our code** uses bilinear interpolation in `apply_block_vclr()`. The Lanczos3 vs bilinear difference affects smoothness at cell boundaries. Also, xLODGen applies VCLR **per-cell**, while we apply it **block-wide**. The block-wide approach averages VCLR at shared borders (good), but uses bilinear instead of Lanczos3 (less smooth).

### Critical Difference #4: Premultiply-Then-Composite vs Our Inline Blend

xLODGen's per-layer compositing:
```pascal
// Step 1: Premultiply ALL channels by mask
PremultiplyByMask(LayerDiffuse, MaskTex);  // R = R * mask_B / 255, A = A * mask_B / 255
// Step 2: Premultiplied src_over
CompositePremultiplied(Result, LayerDiffuse, OffX, OffY);
// dst = src + dst * (1 - src.A)
```

The mask value is stored in the **B channel** of the mask image. The premultiplication scales RGBA, then the composite uses the premultiplied alpha. This is a **two-step** process that properly handles alpha accumulation.

**Our code** does it inline in one step:
```rust
canvas[oi] = (tex_r * alpha + canvas[oi] * (1 - alpha)) as u8;
```
This is mathematically equivalent for a single layer, but may accumulate differently across many layers because we don't track the accumulated alpha in the A channel.

## Specific Missing Pieces to Investigate

### 1. BaseDiffuse Construction (HIGHEST PRIORITY)

The code that builds `Data.BaseDiffuse` from BTXT records is **not in the decompiled reconstruction**. This is the most critical missing piece because it determines the base that ALL alpha layers blend onto.

**Where to find it:** This code lives in the main xLODGen binary (`xLODGen.exe`), not in the reconstructed Pascal units. It would be in whatever function creates `TCellLayerData` before calling `ComposeCellEx()`. The Ghidra decompilation at `/home/luke/Downloads/xLODGen/decomp_xlodgen_x64/` may contain this in the full function decompilation (search for `BaseDiffuse` population or `TCellLayerData` construction).

**Questions to answer:**
- At what resolution is BaseDiffuse created? (128? 256? Output cell size?)
- How are the 4 BTXT textures placed? (simple blit? tiled? with overlap/feathering?)
- Is there any cross-quadrant smoothing in BaseDiffuse itself?

### 2. Layer Texture Preparation

How is `L.Diffuse` (each ATXT layer's texture) prepared before `ComposeCellEx()`?
- Is it the 128×128 from the texture cache directly?
- Or is it resized/tiled to match the output quadrant size?
- The answer changes whether the mask placement creates overflow or not.

### 3. Full Pipeline Caller

The code that:
1. Reads LAND records from plugins
2. Resolves BTXT/ATXT FormIDs to textures
3. Loads and tiles textures
4. Builds `TCellLayerData` (including BaseDiffuse)
5. Calls `ComposeCellEx()`
6. Places result into block canvas

This caller is the "glue" we're missing. It's in the main xLODGen binary, likely in the terrain LOD generation function (search Ghidra for `TerrainLOD`, `BTXT`, or `BaseDiffuse`).

## Current Metrics

| Metric | Value |
|--------|-------|
| Level 4 mean similarity | 98.1% |
| Level 4 median similarity | 98.4% |
| Level 4 mean MSE | 28.7 |
| Level 4 worst MSE | 272 (was 975 before cell-wide mask fix) |
| Level 4 tiles matched | 964 / 1280 |

## What's Working

- BTXT base texture tiling per quadrant
- ATXT alpha layer compositing with Lanczos3 mask resampling
- Cell-wide 65×65 mask embedding (fix from this session — eliminated most hard blocks)
- VCLR vertex color application (bilinear, block-wide)
- Horizontal flip for FNV engine
- DXT1 compression with mipmaps
- Parallel processing via rayon (~3 minutes for 3400 DDS files)
- 33×33 alpha grid built from 17×17 VTXT via square/diamond 3×3 stamps

## What's Not Working

- **Visible quadrant/cell boundaries in-game** — transitions between different base textures are too sharp
- **BTXT is a hard fill per quadrant** — no feathering at quadrant boundaries in the base layer
- **VCLR uses bilinear instead of Lanczos3** — less smooth than reference
- **Possible resolution mismatch** — our comp_cell_size=128 may not match xLODGen's actual cell compositing resolution

## Files

| File | Purpose |
|------|---------|
| `rust-core/xedit_lod/src/terrain_lod.rs` | Main terrain texture pipeline (our code) |
| `decomp_xlodgen_x64/recon/src/uTerrainCellCompositor.pas` | xLODGen cell compositing (decompiled) |
| `decomp_xlodgen_x64/recon/src/uTerrainCompositor.pas` | xLODGen block atlas assembly (decompiled) |
| `decomp_xlodgen_x64/recon/src/uReconTypes.pas` | Shared types |
| `decomp_xlodgen_x64/` (Ghidra project) | Full binary decompilation — search for BaseDiffuse builder |
| `rust-core/lod_comparison/` | Comparison images and scripts |

## Next Steps

1. **Decompile the BaseDiffuse builder** from the xLODGen binary — this is the #1 missing piece
2. **Check actual L.Diffuse size** — is it 128×128 or something larger?
3. **Implement Lanczos3 VCLR** to match xLODGen's per-cell ApplyVCLR
4. **Test with larger comp_cell_size** (256 or output cell size) to see if that matches xLODGen's actual resolution
5. **Verify in Ghidra** how the BTXT base textures are composed — any cross-quadrant blending?
