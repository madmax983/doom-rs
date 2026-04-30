//! BSP-driven seg ordering for wall rendering.
//!
//! The renderer consumes segs in front-to-back order (relative to the player)
//! so solid-wall clipping can reject farther geometry early.

use doom_map::bsp::{BspChild, BspTree};
use doom_map::{Level, SIDEDEF_NONE};

fn seg_sort_key(level: &Level, seg_idx: usize, player_x: i32, player_y: i32) -> i64 {
    let Some(seg) = level.segs.get(seg_idx) else {
        return i64::MAX;
    };
    let Some(v1) = level.vertexes.get(seg.from_vertex as usize) else {
        return i64::MAX;
    };
    let Some(v2) = level.vertexes.get(seg.to_vertex as usize) else {
        return i64::MAX;
    };

    let v1_dx = i64::from(v1.x as i32 - player_x);
    let v1_dy = i64::from(v1.y as i32 - player_y);
    let v2_dx = i64::from(v2.x as i32 - player_x);
    let v2_dy = i64::from(v2.y as i32 - player_y);
    let mid_dx = i64::from(v1.x as i32 + v2.x as i32 - 2 * player_x);
    let mid_dy = i64::from(v1.y as i32 + v2.y as i32 - 2 * player_y);

    let d1 = v1_dx * v1_dx + v1_dy * v1_dy;
    let d2 = v2_dx * v2_dx + v2_dy * v2_dy;
    let d_mid = mid_dx * mid_dx + mid_dy * mid_dy;

    d1.min(d2).min(d_mid)
}

/// Collect seg indices in front-to-back traversal order from the BSP tree.
///
/// Falls back to linear seg order when BSP validation fails.
#[must_use]
pub fn collect_front_to_back_seg_indices(
    level: &Level,
    player_x: i32,
    player_y: i32,
) -> Vec<usize> {
    let Ok(bsp) = BspTree::validate(&level.nodes, &level.ssectors, level.segs.len()) else {
        return (0..level.segs.len()).collect();
    };

    // Trivial BSP: one subsector.
    if bsp.nodes().is_empty() {
        if let Some(ss) = bsp.ssectors().first() {
            let mut out = Vec::with_capacity(level.segs.len());
            let mut seen = vec![false; level.segs.len()];
            push_ordered_subsector_segs(
                level,
                ss.first_seg as usize,
                ss.seg_count as usize,
                player_x,
                player_y,
                &mut out,
                &mut seen,
            );
            return out;
        }
        return (0..level.segs.len()).collect();
    }

    let mut out = Vec::with_capacity(level.segs.len());
    let mut seen = vec![false; level.segs.len()];
    let mut stack = Vec::new();
    let root = (bsp.nodes().len() - 1) as u16;
    stack.push(BspChild::Node(root));

    while let Some(child) = stack.pop() {
        match child {
            BspChild::Subsector(ss_idx) => {
                if let Some(ss) = bsp.ssectors().get(ss_idx as usize) {
                    push_ordered_subsector_segs(
                        level,
                        ss.first_seg as usize,
                        ss.seg_count as usize,
                        player_x,
                        player_y,
                        &mut out,
                        &mut seen,
                    );
                }
            }
            BspChild::Node(node_idx) => {
                let Some(node) = bsp.nodes().get(node_idx as usize) else {
                    continue;
                };

                // Same side test convention as doom_map::bsp::point_in_subsector.
                let dx = node.dx as i32;
                let dy = node.dy as i32;
                let nx = node.x as i32;
                let ny = node.y as i32;
                let cross = dx * (player_y - ny) - dy * (player_x - nx);
                let (near_raw, far_raw) = if cross > 0 {
                    (node.left_child, node.right_child)
                } else {
                    (node.right_child, node.left_child)
                };

                // DFS stack: push far first so near pops first.
                stack.push(BspChild::decode(far_raw));
                stack.push(BspChild::decode(near_raw));
            }
        }
    }

    if out.is_empty() {
        (0..level.segs.len()).collect()
    } else {
        out
    }
}

fn push_ordered_subsector_segs(
    level: &Level,
    first_seg: usize,
    seg_count: usize,
    player_x: i32,
    player_y: i32,
    out: &mut Vec<usize>,
    seen: &mut [bool],
) {
    let end = first_seg.saturating_add(seg_count).min(level.segs.len());
    let start_idx = out.len();

    #[allow(clippy::needless_range_loop)]
    for seg_idx in first_seg..end {
        if !seen[seg_idx] {
            seen[seg_idx] = true;
            out.push(seg_idx);
        }
    }

    if subsector_needs_hardening_sort(level, first_seg, end - first_seg) {
        out[start_idx..].sort_by_key(|&seg_idx| seg_sort_key(level, seg_idx, player_x, player_y));
    }
}

fn subsector_needs_hardening_sort(level: &Level, first_seg: usize, seg_count: usize) -> bool {
    let end = first_seg.saturating_add(seg_count).min(level.segs.len());
    (first_seg..end).any(|seg_idx| {
        let Some(seg) = level.segs.get(seg_idx) else {
            return false;
        };
        let Some(linedef) = level.linedefs.get(seg.linedef as usize) else {
            return false;
        };
        if !linedef.is_two_sided() {
            return false;
        }

        let front_sidedef_idx = if seg.direction == 0 {
            linedef.right_sidedef
        } else {
            linedef.left_sidedef
        };
        let back_sidedef_idx = if seg.direction == 0 {
            linedef.left_sidedef
        } else {
            linedef.right_sidedef
        };
        if front_sidedef_idx == SIDEDEF_NONE || back_sidedef_idx == SIDEDEF_NONE {
            return false;
        }

        let Some(front_sector) = level
            .sidedefs
            .get(front_sidedef_idx as usize)
            .and_then(|sidedef| level.sectors.get(sidedef.sector as usize))
        else {
            return false;
        };

        let Some(back_sector) = level
            .sidedefs
            .get(back_sidedef_idx as usize)
            .and_then(|sidedef| level.sectors.get(sidedef.sector as usize))
        else {
            return false;
        };

        let front_floor = i32::from(front_sector.floor_height);
        let front_ceil = i32::from(front_sector.ceil_height);
        let back_floor = i32::from(back_sector.floor_height);
        let back_ceil = i32::from(back_sector.ceil_height);

        let opening_floor = front_floor.max(back_floor);
        let opening_ceil = front_ceil.min(back_ceil);
        let has_opening = opening_floor < opening_ceil;
        let narrows_front_span = back_floor > front_floor || back_ceil < front_ceil;

        has_opening && narrows_front_span
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_map::lumps::{
        Blockmap, FLAG_TWO_SIDED, Linedef, Node, NodeBBox, Reject, Sector, Seg, Sidedef, Ssector,
        Vertex,
    };

    fn make_two_leaf_bsp_level() -> Level {
        // Two horizontal segs in separate subsectors so order is easy to observe.
        let vertexes = vec![
            Vertex { x: -64, y: 64 },
            Vertex { x: 64, y: 64 },
            Vertex { x: -64, y: 128 },
            Vertex { x: 64, y: 128 },
        ];
        let segs = vec![
            Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            },
            Seg {
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];
        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            },
        ];
        let sidedefs = vec![Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: [0; 8],
            lower_texture: [0; 8],
            middle_texture: *b"WALL1\0\0\0",
            sector: 0,
        }];
        let sectors = vec![Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];

        // One node splitting by line x=0 with direction (0,1):
        // cross = -(px - 0), so px<0 => left child (cross>0), px>=0 => right child.
        let bbox = NodeBBox {
            ymax: 256,
            ymin: -256,
            xmin: -256,
            xmax: 256,
        };
        let nodes = vec![Node {
            x: 0,
            y: 0,
            dx: 0,
            dy: 1,
            right_bbox: bbox,
            left_bbox: bbox,
            right_child: 0x8000,    // subsector 0
            left_child: 0x8000 | 1, // subsector 1
        }];
        let ssectors = vec![
            Ssector {
                first_seg: 0,
                seg_count: 1,
            },
            Ssector {
                first_seg: 1,
                seg_count: 1,
            },
        ];

        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "SEGTEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes,
            sectors,
            reject,
            blockmap,
        }
    }

    fn make_single_subsector_reversed_depth_level(
        portal_bearing: bool,
        back_floor_height: i16,
        back_ceil_height: i16,
    ) -> Level {
        let vertexes = vec![
            Vertex { x: -64, y: 256 },
            Vertex { x: 64, y: 256 },
            Vertex { x: -64, y: 128 },
            Vertex { x: 64, y: 128 },
        ];
        let segs = vec![
            Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            },
            Seg {
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];
        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: if portal_bearing { FLAG_TWO_SIDED } else { 0 },
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: if portal_bearing { 1 } else { 0xFFFF },
            },
        ];
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: *b"WALL1\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
        ];
        let sectors = vec![
            Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: back_floor_height,
                ceil_height: back_ceil_height,
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 160,
                special: 0,
                tag: 0,
            },
        ];
        let ssectors = vec![Ssector {
            first_seg: 0,
            seg_count: 2,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "SEGREV".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    #[test]
    fn front_to_back_order_prefers_player_side_child() {
        let level = make_two_leaf_bsp_level();
        // px > 0 => right child first => seg 0 then seg 1.
        let order = collect_front_to_back_seg_indices(&level, 10, 0);
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn front_to_back_order_swaps_when_player_crosses_partition() {
        let level = make_two_leaf_bsp_level();
        // px < 0 => left child first => seg 1 then seg 0.
        let order = collect_front_to_back_seg_indices(&level, -10, 0);
        assert_eq!(order, vec![1, 0]);
    }

    #[test]
    fn solid_only_subsector_preserves_map_order() {
        let level = make_single_subsector_reversed_depth_level(false, 32, 96);
        let order = collect_front_to_back_seg_indices(&level, 0, 0);
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn closed_two_sided_subsector_preserves_map_order() {
        let level = make_single_subsector_reversed_depth_level(true, 64, 64);
        let order = collect_front_to_back_seg_indices(&level, 0, 0);
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn full_height_two_sided_subsector_preserves_map_order() {
        let level = make_single_subsector_reversed_depth_level(true, 0, 128);
        let order = collect_front_to_back_seg_indices(&level, 0, 0);
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn wider_back_sector_subsector_preserves_map_order() {
        let level = make_single_subsector_reversed_depth_level(true, -32, 160);
        let order = collect_front_to_back_seg_indices(&level, 0, 0);
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn portal_bearing_subsector_still_sorts_nearest_first() {
        let level = make_single_subsector_reversed_depth_level(true, 32, 96);
        let order = collect_front_to_back_seg_indices(&level, 0, 0);
        assert_eq!(order, vec![1, 0]);
    }
}
