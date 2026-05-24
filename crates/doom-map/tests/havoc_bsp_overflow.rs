use doom_map::bsp::BspTree;
use doom_map::lumps::{NODE_SUBSECTOR_BIT, Node, NodeBBox, Ssector};

#[test]
fn test_bsp_max_depth_overflow() {
    let n_nodes = 32767; // Maximum u16 nodes
    let mut nodes = Vec::new();
    let mut ssectors = Vec::new();

    // The tree must not cycle, and must be completely right-leaning.
    for i in 0..n_nodes {
        let left_child = NODE_SUBSECTOR_BIT | 1;
        let right_child = if i == 0 {
            NODE_SUBSECTOR_BIT
        } else {
            (i - 1) as u16
        };
        nodes.push(Node {
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
            right_child,
            left_child,
        });
    }

    // We need n_nodes + 1 subsectors
    for _ in 0..=n_nodes {
        ssectors.push(Ssector {
            first_seg: 0,
            seg_count: 1,
        });
    }

    let tree = BspTree::validate(&nodes, &ssectors, 1).expect("should validate");
    let depth = tree.max_depth();

    // The recursive max_depth will panic due to stack overflow before it can return depth!
    // But since the current implementation is recursive, let's just observe the overflow!
    assert_eq!(depth, n_nodes as u32);
}
