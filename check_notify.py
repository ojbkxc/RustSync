with open("server/src/api/notify.rs", "r", encoding="utf-8") as f:
    lines = f.readlines()
# Line 332 (0-indexed: 331)
print(f"Line 332: {lines[331].rstrip()}")
# Check a few lines around it
for i in range(329, min(336, len(lines))):
    print(f"{i+1}: {lines[i].rstrip()}")