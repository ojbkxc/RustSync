with open(".github/workflows/build.yml", "r", encoding="utf-8") as f:
    content = f.read()

changes = 0

# Fix 1: Add CC env var for aarch64 musl
old1 = "      - name: 构建 Rust\n        working-directory: server\n        run: cargo build --release --target ${{ matrix.target }}"
new1 = "      - name: 构建 Rust\n        working-directory: server\n        env:\n          CC_aarch64_unknown_linux_musl: aarch64-linux-gnu-gcc\n        run: cargo build --release --target ${{ matrix.target }}"
if old1 in content:
    content = content.replace(old1, new1)
    changes += 1
    print("Fix 1 (CC env var): OK")
else:
    print("Fix 1: FAILED")

# Fix 2: Magisk artifact path
old2 = "path: dist/${APP_NAME}-*.zip"
new2 = "path: dist/${{ env.APP_NAME }}-*.zip"
if old2 in content:
    content = content.replace(old2, new2)
    changes += 1
    print("Fix 2 (Magisk path): OK")
else:
    print("Fix 2: FAILED")

# Fix 3: create-release files
old3 = "          files: |\n            dist/${APP_NAME}-app-*.apk\n            dist/${APP_NAME}-${VERSION}.zip\n            dist/${APP_NAME}-*-linux-*.tar.gz\n            dist/${APP_NAME}-*-darwin-*.tar.gz\n            dist/${APP_NAME}-*-windows-*.zip"
new3 = "          files: |\n            dist/${{ env.APP_NAME }}-app-*.apk\n            dist/${{ env.APP_NAME }}-${{ env.VERSION }}.zip\n            dist/${{ env.APP_NAME }}-*-linux-*.tar.gz\n            dist/${{ env.APP_NAME }}-*-darwin-*.tar.gz\n            dist/${{ env.APP_NAME }}-*-windows-*.zip"
if old3 in content:
    content = content.replace(old3, new3)
    changes += 1
    print("Fix 3 (Release files): OK")
else:
    print("Fix 3: FAILED")

with open(".github/workflows/build.yml", "w", encoding="utf-8") as f:
    f.write(content)
print(f"Total changes: {changes}")