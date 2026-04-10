import os
import re
from collections import defaultdict

def find_cycles(crate_path):
    src_dir = os.path.join(crate_path, "src")
    if not os.path.exists(src_dir):
        return

    imports = defaultdict(set)
    for root, dirs, files in os.walk(src_dir):
        for file in files:
            if file.endswith(".rs"):
                mod_name = file[:-3]
                if mod_name == "lib" or mod_name == "mod":
                    mod_name = os.path.basename(root)
                filepath = os.path.join(root, file)
                with open(filepath, 'r') as f:
                    content = f.read()

                # simplistic import parsing
                for line in content.split('\n'):
                    if line.startswith('use crate::'):
                        parts = line[11:].split('::')
                        if len(parts) > 0:
                            imported_mod = parts[0].strip().split('{')[0].strip(';')
                            if imported_mod and imported_mod != mod_name:
                                imports[mod_name].add(imported_mod)

    # find cycles
    cycles = []
    path = []
    visited = set()

    def dfs(node):
        if node in path:
            cycle_start = path.index(node)
            cycles.append(path[cycle_start:] + [node])
            return
        if node in visited:
            return
        visited.add(node)
        path.append(node)
        for neighbor in imports.get(node, []):
            dfs(neighbor)
        path.pop()

    for node in list(imports.keys()):
        dfs(node)

    if cycles:
        print(f"Cycles in {crate_path}:")
        for cycle in cycles:
            print(" -> ".join(cycle))

for d in os.listdir("crates"):
    find_cycles(os.path.join("crates", d))
