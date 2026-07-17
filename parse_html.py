import re

with open("target/llvm-cov/html/coverage/app/crates/doom-map/src/udmf.rs.html", "r") as f:
    html = f.read()

lines = html.split('</tr>')
for line in lines:
    if 'uncovered-line' in line:
        m = re.search(r"<a name='L(\d+)'", line)
        if m:
            line_num = int(m.group(1))
            if line_num >= 430 and line_num <= 460:
                code_m = re.search(r"<td class='code'><pre>(.*?)</pre></td>", line)
                code = code_m.group(1) if code_m else ""
                print(f"{line_num}: {code}")
