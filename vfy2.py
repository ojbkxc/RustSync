with open("server/Cargo.toml", "r", encoding="utf-8") as f:
    lines = f.readlines()
for i in range(20, min(46, len(lines))):
    print(f"{i+1}: {lines[i].rstrip()}")