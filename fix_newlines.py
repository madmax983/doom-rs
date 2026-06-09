import re

def fix(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Replace write!(..., "...\\n", ...) with writeln!(..., "...", ...)
    pattern = re.compile(r'write!\(([^,]+),\s*"(.*?)\\n"(.*?)\)', re.DOTALL)

    def repl(m):
        var_name = m.group(1)
        fmt_str = m.group(2)
        args = m.group(3)
        return f'writeln!({var_name}, "{fmt_str}"{args})'

    new_content = pattern.sub(repl, content)

    # for the ones that look like r#"...\n"#
    pattern2 = re.compile(r'write!\(([^,]+),\s*r#"(.*?)\n"#\s*\)', re.DOTALL)
    def repl2(m):
        var_name = m.group(1)
        fmt_str = m.group(2)
        return f'writeln!({var_name}, r#"{fmt_str}"#)'

    new_content = pattern2.sub(repl2, new_content)

    pattern3 = re.compile(r'write!\(([^,]+),\s*r#"(.*?)\n"#,(.*?)\)', re.DOTALL)
    def repl3(m):
        var_name = m.group(1)
        fmt_str = m.group(2)
        args = m.group(3)
        return f'writeln!({var_name}, r#"{fmt_str}"#,{args})'

    new_content = pattern3.sub(repl3, new_content)

    with open(filepath, 'w') as f:
        f.write(new_content)

for f in ["crates/doom-app/src/main.rs", "crates/doom-map/src/obj.rs", "crates/doom-map/src/svg.rs", "crates/doom-map/src/graph.rs"]:
    fix(f)
