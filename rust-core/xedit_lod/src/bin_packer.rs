//! Binary space partition texture packer for atlas layout.
//!
//! Ported from TwbBinPacker in wbLOD.pas (lines 74-554).
//! Uses MaxSideSort heuristic for optimal packing.

/// A block to be packed into the atlas.
#[derive(Debug, Clone)]
pub struct BinBlock {
    /// User-supplied index for identifying this block after packing.
    pub index: usize,
    /// Width of the block.
    pub w: u32,
    /// Height of the block.
    pub h: u32,
    /// Packed X position (filled by `fit()`).
    pub x: u32,
    /// Packed Y position (filled by `fit()`).
    pub y: u32,
    /// Whether this block was successfully placed.
    pub placed: bool,
}

/// BSP node for the packing tree.
struct BinNode {
    used: bool,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    down: Option<Box<BinNode>>,
    right: Option<Box<BinNode>>,
}

impl BinNode {
    fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self {
            used: false,
            x, y, w, h,
            down: None,
            right: None,
        }
    }
}

/// Binary space partition packer for texture atlas layout.
pub struct BinPacker {
    pub width: u32,
    pub height: u32,
    pub padding_x: u32,
    pub padding_y: u32,
}

impl BinPacker {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width, height,
            padding_x: 0,
            padding_y: 0,
        }
    }

    pub fn with_padding(mut self, px: u32, py: u32) -> Self {
        self.padding_x = px;
        self.padding_y = py;
        self
    }

    /// Pack blocks into the atlas using MaxSideSort heuristic.
    ///
    /// Returns true if all blocks fit. Block positions are written to `x` and `y` fields.
    pub fn fit(&self, blocks: &mut [BinBlock]) -> bool {
        if blocks.is_empty() {
            return true;
        }

        // Sort by max side descending (MaxSideSort heuristic from wbLOD.pas)
        blocks.sort_by(|a, b| {
            let max_a = a.w.max(a.h);
            let max_b = b.w.max(b.h);
            max_b.cmp(&max_a)
        });

        let mut root = BinNode::new(0, 0, self.width, self.height);

        let mut all_fit = true;
        for block in blocks.iter_mut() {
            let bw = block.w + self.padding_x;
            let bh = block.h + self.padding_y;

            if let Some((x, y)) = Self::find_and_split(&mut root, bw, bh) {
                block.x = x;
                block.y = y;
                block.placed = true;
            } else {
                block.placed = false;
                all_fit = false;
            }
        }

        all_fit
    }

    /// Find a free node and split it.
    fn find_and_split(node: &mut BinNode, w: u32, h: u32) -> Option<(u32, u32)> {
        if node.used {
            // Try right child first, then down
            if let Some(ref mut right) = node.right {
                if let Some(pos) = Self::find_and_split(right, w, h) {
                    return Some(pos);
                }
            }
            if let Some(ref mut down) = node.down {
                return Self::find_and_split(down, w, h);
            }
            None
        } else if w <= node.w && h <= node.h {
            // Fits — split this node
            node.used = true;

            // Down: remaining vertical space below the placed block
            node.down = Some(Box::new(BinNode::new(
                node.x,
                node.y + h,
                node.w,
                node.h.saturating_sub(h),
            )));

            // Right: remaining horizontal space to the right
            node.right = Some(Box::new(BinNode::new(
                node.x + w,
                node.y,
                node.w.saturating_sub(w),
                h,
            )));

            Some((node.x, node.y))
        } else {
            None
        }
    }
}

/// Try to pack blocks, growing the atlas if needed.
///
/// Returns the final (width, height) used, or None if blocks can't fit even at max_size.
pub fn fit_with_growth(
    blocks: &mut [BinBlock],
    initial_w: u32,
    initial_h: u32,
    max_w: u32,
    max_h: u32,
    padding_x: u32,
    padding_y: u32,
) -> Option<(u32, u32)> {
    let mut w = initial_w;
    let mut h = initial_h;

    loop {
        let packer = BinPacker::new(w, h).with_padding(padding_x, padding_y);
        if packer.fit(blocks) {
            return Some((w, h));
        }

        // Grow the smaller dimension
        if w <= h {
            w = (w * 2).min(max_w);
        } else {
            h = (h * 2).min(max_h);
        }

        if w > max_w && h > max_h {
            return None;
        }

        // Reset placement
        for b in blocks.iter_mut() {
            b.placed = false;
            b.x = 0;
            b.y = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_block() {
        let mut blocks = vec![BinBlock { index: 0, w: 256, h: 256, x: 0, y: 0, placed: false }];
        let packer = BinPacker::new(1024, 1024);
        assert!(packer.fit(&mut blocks));
        assert_eq!(blocks[0].x, 0);
        assert_eq!(blocks[0].y, 0);
        assert!(blocks[0].placed);
    }

    #[test]
    fn test_multiple_blocks() {
        let mut blocks = vec![
            BinBlock { index: 0, w: 512, h: 512, x: 0, y: 0, placed: false },
            BinBlock { index: 1, w: 512, h: 512, x: 0, y: 0, placed: false },
            BinBlock { index: 2, w: 512, h: 512, x: 0, y: 0, placed: false },
            BinBlock { index: 3, w: 512, h: 512, x: 0, y: 0, placed: false },
        ];
        let packer = BinPacker::new(1024, 1024);
        assert!(packer.fit(&mut blocks));
        for b in &blocks {
            assert!(b.placed);
        }
    }

    #[test]
    fn test_doesnt_fit() {
        let mut blocks = vec![
            BinBlock { index: 0, w: 1024, h: 1024, x: 0, y: 0, placed: false },
            BinBlock { index: 1, w: 1024, h: 1024, x: 0, y: 0, placed: false },
        ];
        let packer = BinPacker::new(1024, 1024);
        assert!(!packer.fit(&mut blocks));
    }

    #[test]
    fn test_fit_with_growth() {
        let mut blocks = vec![
            BinBlock { index: 0, w: 512, h: 512, x: 0, y: 0, placed: false },
            BinBlock { index: 1, w: 512, h: 512, x: 0, y: 0, placed: false },
            BinBlock { index: 2, w: 512, h: 512, x: 0, y: 0, placed: false },
            BinBlock { index: 3, w: 512, h: 512, x: 0, y: 0, placed: false },
            BinBlock { index: 4, w: 512, h: 512, x: 0, y: 0, placed: false },
        ];
        let result = fit_with_growth(&mut blocks, 512, 512, 4096, 4096, 0, 0);
        assert!(result.is_some());
        for b in &blocks {
            assert!(b.placed);
        }
    }
}
