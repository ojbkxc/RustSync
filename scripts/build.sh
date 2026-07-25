#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# RustSync 全平台一键构建脚本
# 用法:
#   ./scripts/build.sh                    # 构建当前平台
#   ./scripts/build.sh linux/amd64        # 指定平台/架构
#   ./scripts/build.sh all                # 全平台
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER_DIR="$PROJECT_DIR/server"
WEB_DIR="$PROJECT_DIR/rustsync/web"
DIST_DIR="$PROJECT_DIR/dist"

VERSION=$(cat "$PROJECT_DIR/rustsync/version.txt" 2>/dev/null | head -1 | cut -d, -f1 | sed "s/^v//" || echo "1.0.0")
APP_NAME="RustSync"

# 颜色
RED='\033[0;31m'; GREEN='\033[0;32m'; BLUE='\033[0;34m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; exit 1; }
step()  { echo -e "${CYAN}[STEP]${NC} $*"; }

# ============================================================
# 平台定义
# ============================================================
declare -A PLATFORM_TARGET
PLATFORM_TARGET["linux/amd64"]="x86_64-unknown-linux-musl"
PLATFORM_TARGET["linux/arm64"]="aarch64-unknown-linux-musl"
PLATFORM_TARGET["macos/amd64"]="x86_64-apple-darwin"
PLATFORM_TARGET["macos/arm64"]="aarch64-apple-darwin"
PLATFORM_TARGET["windows/amd64"]="x86_64-pc-windows-msvc"
PLATFORM_TARGET["android/arm64"]="aarch64-linux-android"

declare -A PLATFORM_EXT
PLATFORM_EXT["linux/amd64"]="tar.gz"
PLATFORM_EXT["linux/arm64"]="tar.gz"
PLATFORM_EXT["macos/amd64"]="tar.gz"
PLATFORM_EXT["macos/arm64"]="tar.gz"
PLATFORM_EXT["windows/amd64"]="zip"
PLATFORM_EXT["android/arm64"]="bin"

# ============================================================
# 构建前端
# ============================================================
build_frontend() {
    step "构建前端..."
    cd "$WEB_DIR"
    npm ci --silent
    npm run build

    mkdir -p "$SERVER_DIR/static"
    rm -rf "$SERVER_DIR/static/"*
    cp -r "$WEB_DIR/dist/"* "$SERVER_DIR/static/"

    test -f "$SERVER_DIR/static/index.html" || err "前端构建产物缺失"
    ok "前端构建完成"
}

# ============================================================
# 构建 Rust 二进制
# ============================================================
build_rust() {
    local platform=$1
    local rust_target="${PLATFORM_TARGET[$platform]}"
    local ext="${PLATFORM_EXT[$platform]}"

    step "构建 Rust ($platform -> $rust_target)..."

    cd "$SERVER_DIR"

    # 安装目标
    rustup target add "$rust_target" 2>/dev/null || true

    # Android 特殊处理
    if [[ "$platform" == android/* ]]; then
        if command -v cargo-ndk &>/dev/null; then
            local ndk_arch="arm64-v8a"
            cargo ndk --target "$rust_target" --platform 21 -- build --release
        else
            cargo build --release --target "$rust_target"
        fi
    else
        cargo build --release --target "$rust_target"
    fi

    local bin_src=$(find "$SERVER_DIR/target/$rust_target/release" -maxdepth 1 -name "rustsync-server*" -type f | head -1)
    if [ -z "$bin_src" ]; then
        bin_src=$(find "$SERVER_DIR/target" -path "*/release/rustsync-server*" -type f | head -1)
    fi
    test -n "$bin_src" || err "未找到编译产物"

    # 输出目录
    local out_dir="$DIST_DIR/$platform"
    mkdir -p "$out_dir"

    local bin_name="rustsync-server"
    if [[ "$platform" == "windows/amd64" ]]; then
        bin_name="rustsync-server.exe"
    fi

    cp "$bin_src" "$out_dir/$bin_name"
    strip "$out_dir/$bin_name" 2>/dev/null || true

    # 打包
    local archive="${APP_NAME}-v${VERSION}-${platform//\//-}.${ext}"
    if [[ "$ext" == "zip" ]]; then
        cd "$out_dir"
        zip -qr "$DIST_DIR/$archive" .
    elif [[ "$ext" == "tar.gz" ]]; then
        cd "$out_dir"
        tar czf "$DIST_DIR/$archive" .
    else
        cp "$out_dir/$bin_name" "$DIST_DIR/${APP_NAME}-v${VERSION}-${platform//\//-}"
    fi

    ok "构建完成: $DIST_DIR/$archive"
}

# ============================================================
# 检测当前平台
# ============================================================
detect_platform() {
    local os arch
    case "$(uname -s)" in
        Linux)  os="linux" ;;
        Darwin) os="macos" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *)      err "不支持的操作系统: $(uname -s)" ;;
    esac
    case "$(uname -m)" in
        x86_64|amd64) arch="amd64" ;;
        aarch64|arm64) arch="arm64" ;;
        *) err "不支持的架构: $(uname -m)" ;;
    esac
    echo "$os/$arch"
}

# ============================================================
# 主流程
# ============================================================
main() {
    local target="${1:-}"

    if [ -z "$target" ]; then
        target=$(detect_platform)
    fi

    info "============================================"
    info " RustSync 全平台构建"
    info " 版本: $VERSION"
    info " 目标: $target"
    info "============================================"

    mkdir -p "$DIST_DIR"

    # 构建前端
    build_frontend

    case "$target" in
        all|--all)
            for platform in "${!PLATFORM_TARGET[@]}"; do
                # 跳过 Android（需要 NDK）
                if [[ "$platform" == android/* ]]; then
                    info "跳过 Android 构建 (需要 NDK，请使用 build-magisk.sh)"
                    continue
                fi
                build_rust "$platform"
            done
            ;;
        linux/*|macos/*|windows/*)
            build_rust "$target"
            ;;
        android/*)
            err "Android 构建请使用: ./scripts/build-magisk.sh"
            ;;
        *)
            err "未知目标: $target (支持: linux/amd64, linux/arm64, macos/amd64, macos/arm64, windows/amd64, all)"
            ;;
    esac

    info "============================================"
    ok "全部构建完成! 产物在 $DIST_DIR/"
    info "============================================"
}

main "$@"