#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# RustSync Magisk 模块构建脚本
# 用法:
#   ./scripts/build-magisk.sh              # 构建当前平台
#   ./scripts/build-magisk.sh arm64-v8a    # 指定架构
#   ./scripts/build-magisk.sh all          # 全架构 (arm64-v8a)
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER_DIR="$PROJECT_DIR/server"
WEB_DIR="$PROJECT_DIR/rustsync/web"
DIST_DIR="$PROJECT_DIR/dist"
MAGISK_TMP="$PROJECT_DIR/dist/magisk-tmp"

VERSION=$(cat "$PROJECT_DIR/rustsync/version.txt" 2>/dev/null || echo "1.0.0")
MAGISK_ID="rustsync_magisk"
MAGISK_NAME="RustSync"

# 颜色输出
RED='\033[0;31m'; GREEN='\033[0;32m'; BLUE='\033[0;34m'; NC='\033[0m'
info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; exit 1; }

# ---- 构建前端 ----
build_frontend() {
    info "构建前端..."
    cd "$WEB_DIR"
    npm ci --silent
    npm run build
    ok "前端构建完成"
}

# ---- 构建 Rust 二进制 ----
build_rust() {
    local target=$1
    local rust_target
    local ndk_platform=21

    case "$target" in
        arm64-v8a)
            rust_target="aarch64-linux-android"
            ;;

        *)
            err "不支持的 Android 架构: $target (仅支持: arm64-v8a)"
            ;;
    esac

    info "构建 Rust 后端 ($target -> $rust_target)..."

    # 复制前端到 static
    mkdir -p "$SERVER_DIR/static"
    cp -r "$WEB_DIR/dist/"* "$SERVER_DIR/static/"
    test -f "$SERVER_DIR/static/index.html" || err "前端构建产物缺失"

    cd "$SERVER_DIR"

    if command -v cargo-ndk &>/dev/null; then
        cargo ndk --target "$rust_target" --platform "$ndk_platform" -- build --release
    else
        # fallback: 直接 cargo build
        rustup target add "$rust_target" 2>/dev/null || true
        cargo build --release --target "$rust_target"
    fi

    local bin_src="$SERVER_DIR/target/$rust_target/release/rustsync-server"
    if [ ! -f "$bin_src" ]; then
        # cargo-ndk 可能输出到不同路径
        bin_src=$(find "$SERVER_DIR/target" -name "rustsync-server" -type f | head -1)
    fi
    test -f "$bin_src" || err "未找到编译产物: rustsync-server"

    local bin_dir="$DIST_DIR/bin/$target"
    mkdir -p "$bin_dir"
    cp "$bin_src" "$bin_dir/rustsync_server"
    strip "$bin_dir/rustsync_server" 2>/dev/null || true
    ok "Rust 二进制构建完成 ($target)"
}

# ---- 打包 Magisk 模块 ----
package_magisk() {
    local target=$1
    local zip_name="${MAGISK_NAME}-${VERSION}.zip"
    local zip_path="$DIST_DIR/$zip_name"

    info "打包 Magisk 模块 ($target)..."

    rm -rf "$MAGISK_TMP"
    mkdir -p "$MAGISK_TMP"

    # 复制 Magisk 模块文件
    cp "$PROJECT_DIR/module.prop" "$MAGISK_TMP/"
    cp "$PROJECT_DIR/customize.sh" "$MAGISK_TMP/"
    cp "$PROJECT_DIR/service.sh" "$MAGISK_TMP/"
    cp -r "$PROJECT_DIR/META-INF" "$MAGISK_TMP/"

    # 复制 Rust 二进制
    mkdir -p "$MAGISK_TMP/system/bin"
    cp "$DIST_DIR/bin/$target/rustsync_server" "$MAGISK_TMP/system/bin/rustsync_server"

    # 复制前端静态文件
    mkdir -p "$MAGISK_TMP/static"
    cp -r "$WEB_DIR/dist/"* "$MAGISK_TMP/static/"

    # 更新 module.prop 版本信息
    local version_code=$(echo "$VERSION" | tr -d '.')
    sed -i "s/^version=.*/version=$VERSION/" "$MAGISK_TMP/module.prop"
    sed -i "s/^versionCode=.*/versionCode=$version_code/" "$MAGISK_TMP/module.prop"

    # 打包
    cd "$MAGISK_TMP"
    zip -qr "$zip_path" .
    cd "$PROJECT_DIR"

    rm -rf "$MAGISK_TMP"
    ok "Magisk 模块打包完成: $zip_path"
    ls -lh "$zip_path"
}

# ---- 主流程 ----
main() {
    local arch="${1:-arm64-v8a}"

    info "============================================"
    info " RustSync Magisk 模块构建"
    info " 版本: $VERSION"
    info "============================================"

    mkdir -p "$DIST_DIR"

    # 构建前端（只需一次）
    build_frontend

    case "$arch" in
        all)
            for target in arm64-v8a; do
                build_rust "$target"
                package_magisk "$target"
            done
            ;;
        arm64-v8a)
            build_rust "$arch"
            package_magisk "$arch"
            ;;
        *)
            err "用法: $0 [arm64-v8a|all]"
            ;;
    esac

    info "============================================"
    ok "全部构建完成! 产物在 $DIST_DIR/"
    info "============================================"
}

main "$@"