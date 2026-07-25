with open("server/Cargo.toml", "r", encoding="utf-8") as f:
    lines = f.readlines()
for i, line in enumerate(lines):
    if "bytes" in line.lower() and "bytes" in line:
        print(f"{i+1}: {line.rstrip()}")