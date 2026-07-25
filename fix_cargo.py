with open("server/Cargo.toml", "r", encoding="utf-8") as f:
    content = f.read()

# Add bytes dependency
old = "# 存储驱动"
new = "# 字节缓冲\nbytes = \"1\"\n\n# 存储驱动"
if old in content:
    content = content.replace(old, new)
    print("bytes dep added")
else:
    print("FAILED: pattern not found")

with open("server/Cargo.toml", "w", encoding="utf-8") as f:
    f.write(content)