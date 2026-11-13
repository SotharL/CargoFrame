//! Grid arithmetic for the builder: overlap tests (blocks in tabs may share cells, see `model.rs`). (Where a dropped block goes is worked out by
//! the builder page itself, in `ui/src/settings/layout/model.ts`.)

use crate::model::{Layout, Rect};

impl Rect {
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> u32 {
        self.x.saturating_add(self.w)
    }

    pub fn bottom(&self) -> u32 {
        self.y.saturating_add(self.h)
    }

    /// Touching edges do not overlap.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.w > 0
            && self.h > 0
            && other.w > 0
            && other.h > 0
            && self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Whether it is inside a grid of `columns` columns.
    pub fn fits_grid(&self, columns: u32) -> bool {
        self.w > 0 && self.h > 0 && self.right() <= columns
    }
}

impl Layout {
    /// Can `rect` be placed without leaving the grid or overlapping another block?
    /// `ignore` is the block being moved or resized; it may cover the blocks it shares cells with
    /// (its Tabs block, the blocks of the other tabs: [`Layout::may_share`]).
    pub fn can_place(&self, rect: &Rect, ignore: Option<&str>) -> bool {
        rect.fits_grid(self.grid_columns())
            && self
                .blocks
                .iter()
                .filter(|b| Some(b.id.as_str()) != ignore)
                .all(|b| {
                    !b.rect.overlaps(rect) || ignore.is_some_and(|id| self.may_share(id, &b.id))
                })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Block, BlockKind};
    use crate::GRID_COLUMNS;

    fn block(id: &str, x: u32, y: u32, w: u32, h: u32) -> Block {
        Block {
            id: id.into(),
            rect: Rect::new(x, y, w, h),
            kind: BlockKind::Screenshot,
        }
    }

    #[test]
    fn overlap_rules() {
        let a = Rect::new(0, 0, 4, 2);
        assert!(a.overlaps(&Rect::new(3, 1, 4, 2)));
        assert!(!a.overlaps(&Rect::new(4, 0, 4, 2))); // touching on the right
        assert!(!a.overlaps(&Rect::new(0, 2, 4, 2))); // touching below
        assert!(!a.overlaps(&Rect::new(0, 0, 0, 2))); // empty
    }

    #[test]
    fn grid_bounds() {
        assert!(Rect::new(8, 0, 4, 1).fits_grid(GRID_COLUMNS));
        assert!(!Rect::new(9, 0, 4, 1).fits_grid(GRID_COLUMNS));
        // a wider grid has room for it
        assert!(Rect::new(9, 0, 4, 1).fits_grid(16));
        assert!(!Rect::new(0, 0, 0, 1).fits_grid(GRID_COLUMNS));
        assert!(!Rect::new(u32::MAX, 0, 2, 1).fits_grid(GRID_COLUMNS));
    }

    #[test]
    fn can_place_ignores_the_moved_block() {
        let mut l = Layout::empty("l", "L");
        l.blocks.push(block("a", 0, 0, 6, 2));
        assert!(!l.can_place(&Rect::new(2, 0, 4, 2), None));
        assert!(l.can_place(&Rect::new(2, 0, 4, 2), Some("a")));
    }
}
