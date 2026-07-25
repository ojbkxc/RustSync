with open("server/Cargo.toml","r") as f: lines=f.readlines()
found=False
new_lines=[]
for l in lines:
 if l.strip()=="bytes = \"1\"":
  if not found: found=True; new_lines.append(l)
  else: print("Removed dup")
 else: new_lines.append(l)
open("server/Cargo.toml","w").writelines(new_lines)
print("Done")
