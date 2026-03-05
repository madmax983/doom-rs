//! BSP tree traversal and structural validity proofs.
//!
//! # The Crown-Jewel Invariant
//! For any valid Doom BSP tree: **N_SSECTORS == N_NODES + 1**
//!
//! This follows from the full-binary-tree leaf theorem:
//! every BSP node has exactly 2 children (left and right), each of which
//! is either another node or a leaf (subsector).  By induction on tree depth:
//! - Base case (1 node): left=leaf, right=leaf → 2 leaves = 1 node + 1 ✓
//! - Inductive step: adding a node converts one leaf into a node + 2 leaves,
//!   net change = +1 node, +1 leaf → invariant preserved ✓
//!
//! The runtime check in [`BspTree::validate_leaf_count`] enforces this.
//! A Verus formal proof would encode this by structural induction on
//! `bsp_depth(node_id, nodes)` (a `spec fn` that is finite because
//! `N_NODES < u16::MAX`).

use crate::lumps::{NODE_INDEX_MASK, NODE_SUBSECTOR_BIT, Node, Ssector};
use thiserror::Error;

/// Errors from BSP structural validation.
#[derive(Debug, Error)]
pub enum BspError {
    /// The leaf-count invariant is violated.
    #[error("BSP invariant violated: N_SSECTORS ({ssectors}) != N_NODES ({nodes}) + 1")]
    LeafCountMismatch { nodes: usize, ssectors: usize },

    /// A child pointer has a leaf bit but the index exceeds N_SSECTORS.
    #[error("BSP node {node_idx}: leaf child index {child_idx} >= N_SSECTORS ({n_ssectors})")]
    LeafChildOutOfBounds {
        node_idx: usize,
        child_idx: usize,
        n_ssectors: usize,
    },

    /// A child pointer has no leaf bit but the index exceeds N_NODES.
    #[error("BSP node {node_idx}: node child index {child_idx} >= N_NODES ({n_nodes})")]
    NodeChildOutOfBounds {
        node_idx: usize,
        child_idx: usize,
        n_nodes: usize,
    },

    /// A node bounding box is degenerate (ymax < ymin or xmax < xmin).
    #[error("BSP node {node_idx}: bounding box is degenerate")]
    DegenerateBBox { node_idx: usize },

    /// A subsector has zero segs.
    #[error("BSP subsector {ss_idx}: seg_count is 0")]
    EmptySubsector { ss_idx: usize },

    /// A subsector's seg range exceeds N_SEGS.
    #[error(
        "BSP subsector {ss_idx}: first_seg({first_seg}) + seg_count({seg_count}) > N_SEGS({n_segs})"
    )]
    SubsectorSegsOutOfBounds {
        ss_idx: usize,
        first_seg: usize,
        seg_count: usize,
        n_segs: usize,
    },
}

/// Decoded child pointer from a BSP node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BspChild {
    /// Another BSP node at the given index into `nodes`.
    Node(u16),
    /// A leaf subsector at the given index into `ssectors`.
    Subsector(u16),
}

impl BspChild {
    /// Decode a raw 16-bit child pointer.
    #[inline]
    pub fn decode(raw: u16) -> Self {
        if raw & NODE_SUBSECTOR_BIT != 0 {
            Self::Subsector(raw & NODE_INDEX_MASK)
        } else {
            Self::Node(raw)
        }
    }
}

/// A validated BSP tree — holds references to the node and subsector arrays.
pub struct BspTree<'a> {
    nodes: &'a [Node],
    ssectors: &'a [Ssector],
}

impl<'a> BspTree<'a> {
    /// Construct from already-parsed arrays. Runs full structural validation.
    ///
    /// # Errors
    /// Returns the first invariant violation encountered.
    pub fn validate(
        nodes: &'a [Node],
        ssectors: &'a [Ssector],
        n_segs: usize,
    ) -> Result<Self, BspError> {
        let tree = Self { nodes, ssectors };

        // Invariant 1: N_SSECTORS == N_NODES + 1  (crown jewel)
        tree.validate_leaf_count()?;

        // Invariant 2 & 3: all child pointers in bounds
        tree.validate_child_bounds()?;

        // Invariant 4: all node bboxes non-degenerate
        tree.validate_bboxes()?;

        // Invariant 5 & 6: all ssector seg ranges valid
        tree.validate_ssector_segs(n_segs)?;

        Ok(tree)
    }

    // -- individual checks ---------------------------------------------------

    /// Verifies `N_SSECTORS == N_NODES + 1`.
    fn validate_leaf_count(&self) -> Result<(), BspError> {
        let n = self.nodes.len();
        let s = self.ssectors.len();
        // Special case: a map with no nodes has exactly one subsector.
        if n == 0 {
            if s == 1 {
                return Ok(());
            }
        } else if s == n + 1 {
            return Ok(());
        }
        Err(BspError::LeafCountMismatch {
            nodes: n,
            ssectors: s,
        })
    }

    /// Verifies all child pointers are in-bounds.
    fn validate_child_bounds(&self) -> Result<(), BspError> {
        let n_nodes = self.nodes.len();
        let n_ssectors = self.ssectors.len();

        for (i, node) in self.nodes.iter().enumerate() {
            for raw_child in [node.right_child, node.left_child] {
                match BspChild::decode(raw_child) {
                    BspChild::Subsector(idx) => {
                        if idx as usize >= n_ssectors {
                            return Err(BspError::LeafChildOutOfBounds {
                                node_idx: i,
                                child_idx: idx as usize,
                                n_ssectors,
                            });
                        }
                    }
                    BspChild::Node(idx) => {
                        if idx as usize >= n_nodes {
                            return Err(BspError::NodeChildOutOfBounds {
                                node_idx: i,
                                child_idx: idx as usize,
                                n_nodes,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Verifies all node bounding boxes are non-degenerate.
    fn validate_bboxes(&self) -> Result<(), BspError> {
        for (i, node) in self.nodes.iter().enumerate() {
            if !node.right_bbox.is_valid() || !node.left_bbox.is_valid() {
                return Err(BspError::DegenerateBBox { node_idx: i });
            }
        }
        Ok(())
    }

    /// Verifies all subsector seg ranges stay within `[0, n_segs)`.
    fn validate_ssector_segs(&self, n_segs: usize) -> Result<(), BspError> {
        for (i, ss) in self.ssectors.iter().enumerate() {
            if ss.seg_count == 0 {
                return Err(BspError::EmptySubsector { ss_idx: i });
            }
            let end = ss.seg_end();
            if end > n_segs {
                return Err(BspError::SubsectorSegsOutOfBounds {
                    ss_idx: i,
                    first_seg: ss.first_seg as usize,
                    seg_count: ss.seg_count as usize,
                    n_segs,
                });
            }
        }
        Ok(())
    }

    // -- traversal -----------------------------------------------------------

    /// Walk to the subsector containing point `(px, py)`.
    ///
    /// Implements Doom's `R_PointInSubsector`: traverse from the root node,
    /// choosing right or left child based on which side of the partition line
    /// the point falls on.
    ///
    /// Returns `None` only if the tree has no nodes and no subsectors (empty level).
    pub fn point_in_subsector(&self, px: i32, py: i32) -> Option<&Ssector> {
        if self.nodes.is_empty() {
            return self.ssectors.first();
        }

        let mut node_idx = (self.nodes.len() - 1) as u16;

        loop {
            let node = &self.nodes[node_idx as usize];

            // Partition-line side test.
            // dx*(py - y) - dy*(px - x):
            // positive → left side, negative → right side (or on the line).
            let dx = node.dx as i32;
            let dy = node.dy as i32;
            let nx = node.x as i32;
            let ny = node.y as i32;
            let cross = dx * (py - ny) - dy * (px - nx);
            let child_raw = if cross > 0 {
                node.left_child
            } else {
                node.right_child
            };

            match BspChild::decode(child_raw) {
                BspChild::Subsector(ss_idx) => {
                    return self.ssectors.get(ss_idx as usize);
                }
                BspChild::Node(next_node) => {
                    node_idx = next_node;
                }
            }
        }
    }

    /// Compute the maximum depth of the BSP tree (counting from root).
    ///
    /// Used by the Phase 3 gate to print geometry stats.
    pub fn max_depth(&self) -> u32 {
        if self.nodes.is_empty() {
            return 0;
        }
        let root = (self.nodes.len() - 1) as u16;
        self.subtree_depth(BspChild::Node(root))
    }

    fn subtree_depth(&self, child: BspChild) -> u32 {
        match child {
            BspChild::Subsector(_) => 0,
            BspChild::Node(idx) => {
                let node = &self.nodes[idx as usize];
                let left_depth = self.subtree_depth(BspChild::decode(node.left_child));
                let right_depth = self.subtree_depth(BspChild::decode(node.right_child));
                1 + left_depth.max(right_depth)
            }
        }
    }

    /// Access the raw node slice.
    pub fn nodes(&self) -> &[Node] {
        self.nodes
    }

    /// Access the raw subsector slice.
    pub fn ssectors(&self) -> &[Ssector] {
        self.ssectors
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{NodeBBox, Ssector};

    fn make_node(right: u16, left: u16) -> Node {
        Node {
            x: 0,
            y: 0,
            dx: 1,
            dy: 0,
            right_bbox: NodeBBox {
                ymax: 10,
                ymin: 0,
                xmin: 0,
                xmax: 10,
            },
            left_bbox: NodeBBox {
                ymax: 10,
                ymin: 0,
                xmin: 0,
                xmax: 10,
            },
            right_child: right,
            left_child: left,
        }
    }

    fn leaf(idx: u16) -> u16 {
        NODE_SUBSECTOR_BIT | idx
    }

    fn make_ssector(first: u16, count: u16) -> Ssector {
        Ssector {
            first_seg: first,
            seg_count: count,
        }
    }

    #[test]
    fn minimal_valid_bsp_one_node_two_leaves() {
        // 1 node, 2 ssectors (leaf 0, leaf 1)
        let nodes = vec![make_node(leaf(0), leaf(1))];
        let ssectors = vec![make_ssector(0, 1), make_ssector(1, 1)];
        let tree = BspTree::validate(&nodes, &ssectors, 2).expect("should validate");
        assert_eq!(tree.max_depth(), 1);
    }

    #[test]
    fn leaf_count_invariant_enforced() {
        // 1 node but only 1 ssector (should be 2) → error
        let nodes = vec![make_node(leaf(0), leaf(0))];
        let ssectors = vec![make_ssector(0, 1)];
        assert!(matches!(
            BspTree::validate(&nodes, &ssectors, 1),
            Err(BspError::LeafCountMismatch { .. })
        ));
    }

    #[test]
    fn node_child_out_of_bounds_detected() {
        // Right child points to node 99 which doesn't exist.
        let nodes = vec![make_node(99, leaf(0))]; // right=99 (node), left=leaf(0)
        let ssectors = vec![make_ssector(0, 1), make_ssector(1, 1)];
        assert!(matches!(
            BspTree::validate(&nodes, &ssectors, 2),
            Err(BspError::NodeChildOutOfBounds { .. })
        ));
    }

    #[test]
    fn leaf_child_out_of_bounds_detected() {
        // Leaf index 5 but only 2 ssectors.
        let nodes = vec![make_node(leaf(5), leaf(1))];
        let ssectors = vec![make_ssector(0, 1), make_ssector(1, 1)];
        assert!(matches!(
            BspTree::validate(&nodes, &ssectors, 2),
            Err(BspError::LeafChildOutOfBounds { .. })
        ));
    }

    #[test]
    fn empty_ssector_detected() {
        let nodes = vec![make_node(leaf(0), leaf(1))];
        let ssectors = vec![
            Ssector {
                first_seg: 0,
                seg_count: 0,
            }, // empty!
            make_ssector(0, 1),
        ];
        assert!(matches!(
            BspTree::validate(&nodes, &ssectors, 2),
            Err(BspError::EmptySubsector { .. })
        ));
    }

    #[test]
    fn point_in_subsector_right_side() {
        // Partition line: x=0, y=0, dx=0, dy=1 (vertical line at x=0).
        // Point (10, 5): cross = 0*(5-0) - 1*(10-0) = -10 < 0 → right child.
        let mut node = make_node(leaf(0), leaf(1));
        node.x = 0;
        node.y = 0;
        node.dx = 0;
        node.dy = 1;
        let nodes = vec![node];
        let ssectors = vec![make_ssector(0, 1), make_ssector(1, 1)];
        let tree = BspTree::validate(&nodes, &ssectors, 2).unwrap();
        let ss = tree.point_in_subsector(10, 5).unwrap();
        assert_eq!(ss.first_seg, 0); // right subsector
    }

    #[test]
    fn bsp_child_decode_subsector() {
        let raw = NODE_SUBSECTOR_BIT | 42;
        assert_eq!(BspChild::decode(raw), BspChild::Subsector(42));
    }

    #[test]
    fn bsp_child_decode_node() {
        assert_eq!(BspChild::decode(7), BspChild::Node(7));
    }

    #[test]
    fn three_node_tree_depth_2() {
        //       root(2)
        //      /       \
        //   node(1)   leaf(2)
        //   /   \
        // leaf(0) leaf(1)
        let nodes = vec![
            make_node(leaf(0), leaf(1)), // node 0
            make_node(0, leaf(2)),       // node 1: right=node(0), left=leaf(2)
        ];
        // With 2 nodes we need 3 ssectors.
        let ssectors = vec![make_ssector(0, 1), make_ssector(1, 1), make_ssector(2, 1)];
        let tree = BspTree::validate(&nodes, &ssectors, 3).expect("valid");
        assert_eq!(tree.max_depth(), 2);
    }
}
