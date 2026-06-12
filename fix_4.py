import sys

def replace(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    target = """struct AnalyzerContext {
    visited: HashSet<usize>,
    discovery_time: HashMap<usize, usize>,
    low_time: HashMap<usize, usize>,
    parent: HashMap<usize, usize>,
    articulation_points: HashSet<usize>,
    children_map: HashMap<usize, usize>,
    time: usize,
}

impl Default for AnalyzerContext {
    fn default() -> Self {
        Self {
            visited: HashSet::new(),
            discovery_time: HashMap::new(),
            low_time: HashMap::new(),
            parent: HashMap::new(),
            articulation_points: HashSet::new(),
            children_map: HashMap::new(),
            time: 0,
        }
    }
}"""

    replacement = """#[derive(Default)]
struct AnalyzerContext {
    visited: HashSet<usize>,
    discovery_time: HashMap<usize, usize>,
    low_time: HashMap<usize, usize>,
    parent: HashMap<usize, usize>,
    articulation_points: HashSet<usize>,
    children_map: HashMap<usize, usize>,
    time: usize,
}"""

    new_content = content.replace(target, replacement)

    with open(filepath, 'w') as f:
        f.write(new_content)

replace('crates/doom-map/src/analyzer.rs')
