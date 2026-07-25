with open(r"rustsync\web\src\locales\zh-CN.yaml", "r", encoding="utf-8") as f:
    lines = f.readlines()
print(f"Line 318: {lines[317].rstrip()}")
for i, line in enumerate(lines):
    s = line.strip()
    if s.startswith("#") or s.startswith("-"):
        continue
    if ":" in s:
        key, _, val = s.partition(":")
        val = val.strip()
        if val.startswith('"') and val.endswith('"'):
            inner = val[1:-1]
            if '"' in inner:
                print(f"ISSUE Line {i+1}: {s[:120]}")
print("Check done")