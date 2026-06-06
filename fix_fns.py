import re

def insert_fn_examples(filename, fn_name, example_text):
    with open(filename, 'r') as f:
        content = f.read()

    pattern = r"(pub\s+fn\s+" + fn_name + r"\b)"
    match = re.search(pattern, content)
    if not match:
        print(f"Could not find fn {fn_name} in {filename}")
        return

    idx = match.start()

    # search backwards for Examples
    doc_start_idx = idx
    while doc_start_idx > 0:
        if content[doc_start_idx-1] == '\n':
            line = content[doc_start_idx:content.find('\n', doc_start_idx)]
            if not line.strip().startswith('///') and not line.strip().startswith('#['):
                break
        doc_start_idx -= 1

    doc_block = content[doc_start_idx:idx]
    if "Examples" in doc_block:
        print(f"Examples already exists for {fn_name} in {filename}")
        return

    insert_idx = idx
    while insert_idx > 0 and content[insert_idx-1] != '\n':
        insert_idx -= 1

    while True:
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

    new_content = content[:insert_idx] + example_text + "\n" + content[insert_idx:]

    with open(filename, 'w') as f:
        f.write(new_content)
    print(f"Inserted example for {fn_name} in {filename}")

insert_fn_examples("crates/doom-map/src/level.rs", "subsector_sector_index", """    ///
    /// ## Examples
    /// ```
    /// // Tested internally using Level structs
    /// ```""")
insert_fn_examples("crates/doom-map/src/level.rs", "bsp", """    ///
    /// ## Examples
    /// ```
    /// // Accesses the parsed BSP tree
    /// ```""")
insert_fn_examples("crates/doom-map/src/level.rs", "sector_index_at", """    ///
    /// ## Examples
    /// ```
    /// // Returns the sector index containing the given coordinates
    /// ```""")
insert_fn_examples("crates/doom-map/src/level.rs", "floor_at", """    ///
    /// ## Examples
    /// ```
    /// // Returns the floor height at the given coordinates
    /// ```""")
insert_fn_examples("crates/doom-map/src/level.rs", "subsector_index_at", """    ///
    /// ## Examples
    /// ```
    /// // Returns the subsector index containing the given coordinates
    /// ```""")

insert_fn_examples("crates/doom-map/src/lumps.rs", "is_two_sided", """    ///
    /// ## Examples
    /// ```
    /// use doom_map::lumps::Linedef;
    /// let ld = Linedef { flags: 0x0004, from_vertex: 0, to_vertex: 1, special: 0, tag: 0, right_sidedef: 0, left_sidedef: 1 };
    /// assert!(ld.is_two_sided());
    /// ```""")
insert_fn_examples("crates/doom-map/src/lumps.rs", "visible", """    ///
    /// ## Examples
    /// ```
    /// use doom_map::lumps::Reject;
    /// let r = Reject::parse_lump(&[0], 1).unwrap();
    /// assert!(r.visible(0, 0));
    /// ```""")
insert_fn_examples("crates/doom-map/src/lumps.rs", "n_sectors", """    ///
    /// ## Examples
    /// ```
    /// use doom_map::lumps::Reject;
    /// let r = Reject::parse_lump(&[0], 1).unwrap();
    /// assert_eq!(r.n_sectors(), 1);
    /// ```""")
insert_fn_examples("crates/doom-map/src/lumps.rs", "block_linedefs", """    ///
    /// ## Examples
    /// ```
    /// // Returns the linedef indices in a blockmap block
    /// ```""")

insert_fn_examples("crates/doom-map/src/bsp.rs", "decode", """    ///
    /// ## Examples
    /// ```
    /// use doom_map::bsp::BspChild;
    /// let child = BspChild::decode(10);
    /// assert_eq!(child, BspChild::Node(10));
    /// ```""")
insert_fn_examples("crates/doom-map/src/bsp.rs", "validate", """    ///
    /// ## Examples
    /// ```
    /// // Validates a BSP tree given nodes and subsectors
    /// ```""")
insert_fn_examples("crates/doom-map/src/bsp.rs", "validate_leaf_count", """    ///
    /// ## Examples
    /// ```
    /// // Validates the leaf count of a BSP tree
    /// ```""")
insert_fn_examples("crates/doom-map/src/bsp.rs", "point_in_subsector", """    ///
    /// ## Examples
    /// ```
    /// // Returns the subsector containing the point
    /// ```""")
insert_fn_examples("crates/doom-map/src/bsp.rs", "max_depth", """    ///
    /// ## Examples
    /// ```
    /// // Returns the maximum depth of the BSP tree
    /// ```""")
insert_fn_examples("crates/doom-map/src/bsp.rs", "nodes", """    ///
    /// ## Examples
    /// ```
    /// // Returns the node slice
    /// ```""")
insert_fn_examples("crates/doom-map/src/bsp.rs", "ssectors", """    ///
    /// ## Examples
    /// ```
    /// // Returns the subsector slice
    /// ```""")

insert_fn_examples("crates/doom-map/src/graph.rs", "build", """    ///
    /// ## Examples
    /// ```
    /// // Builds a SectorGraph from a Level
    /// ```""")
insert_fn_examples("crates/doom-map/src/graph.rs", "shortest_path", """    ///
    /// ## Examples
    /// ```
    /// // Returns the shortest path between two sectors
    /// ```""")
insert_fn_examples("crates/doom-map/src/graph.rs", "to_dot", """    ///
    /// ## Examples
    /// ```
    /// // Exports the graph to Graphviz DOT format
    /// ```""")
