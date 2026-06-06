import re

def insert_examples(filename, item_type, item_name, example_text):
    with open(filename, 'r') as f:
        content = f.read()

    # Find the pub struct or pub enum
    pattern = r"(pub\s+(?:" + item_type + r")\s+" + item_name + r"\b)"
    match = re.search(pattern, content)
    if not match:
        print(f"Could not find {item_type} {item_name} in {filename}")
        return

    idx = match.start()

    # Check if Examples is already in the doc block above
    # Search backwards for the doc block
    doc_start_idx = idx
    while doc_start_idx > 0:
        if content[doc_start_idx-1] == '\n':
            line = content[doc_start_idx:content.find('\n', doc_start_idx)]
            if not line.strip().startswith('///') and not line.strip().startswith('#['):
                break
        doc_start_idx -= 1

    doc_block = content[doc_start_idx:idx]
    if "Examples" in doc_block:
        print(f"Examples already exists for {item_name} in {filename}")
        return

    # Look for the last /// comment before idx (or before #[derive...])

    # We will just insert the example before any #[derive...] or the struct itself
    insert_idx = idx
    while insert_idx > 0 and content[insert_idx-1] != '\n':
        insert_idx -= 1

    while True:
        # Check if previous line is a #[ attribute
        prev_line_end = insert_idx - 1
        if prev_line_end < 0:
            break
        prev_line_start = prev_line_end
        while prev_line_start > 0 and content[prev_line_start-1] != '\n':
            prev_line_start -= 1

        prev_line = content[prev_line_start:prev_line_end+1]
        if prev_line.strip().startswith('#['):
            insert_idx = prev_line_start
        else:
            break

    # Now insert_idx is at the line before #[derive...] or the struct/enum itself
    new_content = content[:insert_idx] + example_text + "\n" + content[insert_idx:]

    with open(filename, 'w') as f:
        f.write(new_content)
    print(f"Inserted example for {item_name} in {filename}")


insert_examples("crates/doom-map/src/bsp.rs", "enum", "BspError", """///
/// ## Examples
/// ```
/// use doom_map::bsp::BspError;
///
/// let err = BspError::LeafCountMismatch { nodes: 10, ssectors: 5 };
/// assert!(err.to_string().contains("BSP invariant violated"));
/// ```""")

insert_examples("crates/doom-map/src/bsp.rs", "enum", "BspChild", """///
/// ## Examples
/// ```
/// use doom_map::bsp::BspChild;
///
/// let child = BspChild::decode(10);
/// assert_eq!(child, BspChild::Node(10));
/// ```""")

insert_examples("crates/doom-map/src/bsp.rs", "struct", "BspTree", """///
/// ## Examples
/// ```
/// use doom_map::bsp::BspTree;
/// use doom_map::lumps::{Node, Ssector, NodeBBox};
///
/// let nodes = vec![Node {
///     x: 0, y: 0, dx: 1, dy: 0,
///     right_bbox: NodeBBox { ymax: 10, ymin: 0, xmin: 0, xmax: 10 },
///     left_bbox: NodeBBox { ymax: 10, ymin: 0, xmin: 0, xmax: 10 },
///     right_child: 0x8000 | 0, // leaf 0
///     left_child: 0x8000 | 1,  // leaf 1
/// }];
/// let ssectors = vec![
///     Ssector { first_seg: 0, seg_count: 1 },
///     Ssector { first_seg: 1, seg_count: 1 },
/// ];
///
/// let tree = BspTree::validate(&nodes, &ssectors, 2).unwrap();
/// assert_eq!(tree.max_depth(), 1);
/// ```""")

insert_examples("crates/doom-map/src/graph.rs", "struct", "SectorGraph", """///
/// ## Examples
/// ```
/// use std::collections::{HashMap, HashSet};
/// use doom_map::SectorGraph;
///
/// let mut adj = HashMap::new();
/// adj.insert(0, HashSet::from([1]));
/// adj.insert(1, HashSet::from([0]));
/// let graph = SectorGraph { adjacency_list: adj };
/// ```""")

