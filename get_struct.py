import sys

def find_struct(filename, struct_name):
    with open(filename) as f:
        content = f.read()

    start_idx = content.find(f"pub struct {struct_name}")
    if start_idx == -1:
        start_idx = content.find(f"struct {struct_name}")
        if start_idx == -1:
            print("Struct not found")
            return

    # Find matching brace
    brace_count = 0
    in_struct = False

    for i in range(start_idx, len(content)):
        if content[i] == '{':
            brace_count += 1
            in_struct = True
        elif content[i] == '}':
            brace_count -= 1
            if in_struct and brace_count == 0:
                print(content[start_idx:i+1])
                return

if __name__ == "__main__":
    find_struct(sys.argv[1], sys.argv[2])
