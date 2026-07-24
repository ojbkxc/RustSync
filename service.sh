#!/system/bin/sh
export PATH=/sbin:/system/sbin:/system/bin:/system/xbin

# 等待系统完全启动
until [ "$(getprop sys.boot_completed)" = "1" ]; do
  sleep 10
done

MODDIR=${0%/*}
CONF_DIR=/data/adb/rustsync
LOG_FILE="$CONF_DIR/rustsync.log"
DATA_DIR="$CONF_DIR/data"
TMP_DIR="$CONF_DIR/tmp"

log() {
  echo "$(date '+%Y-%m-%d %H:%M:%S') $1" >> "$LOG_FILE"
}

mkdir -p "$CONF_DIR"
mkdir -p "$DATA_DIR"
mkdir -p "$TMP_DIR"
chmod 1777 "$TMP_DIR"
export TMPDIR="$TMP_DIR"

# 创建 /tmp 符号链接指向数据分区
if [ -L /tmp ]; then
  link_target=$(readlink /tmp 2>/dev/null)
  if [ "$link_target" != "$TMP_DIR" ]; then
    rm -f /tmp
    ln -s "$TMP_DIR" /tmp
    log "更新 /tmp 符号链接 -> $TMP_DIR"
  fi
elif [ -d /tmp ]; then
  rm -rf /tmp
  ln -s "$TMP_DIR" /tmp
  log "替换 /tmp 目录为符号链接 -> $TMP_DIR"
else
  ln -s "$TMP_DIR" /tmp
  log "创建 /tmp 符号链接 -> $TMP_DIR"
fi

log "=== service.sh 启动 ==="

# 查找 rustsync 二进制
RUSTSYNC_BIN=""
if [ -f "$MODDIR/rustsync_bin" ]; then
  RUSTSYNC_BIN="$MODDIR/rustsync_bin"
else
  log "错误: 未找到 RustSync 二进制文件"
  exit 1
fi

chmod 755 "$RUSTSYNC_BIN" 2>/dev/null
log "二进制: $RUSTSYNC_BIN"
log "数据目录: $DATA_DIR"

# 启动函数
start_rustsync() {
  mkdir -p "$DATA_DIR"
  mkdir -p "$TMP_DIR"
  chmod 1777 "$TMP_DIR"

  # 设置环境变量
  export TZ=Asia/Shanghai
  export RUSTSYNC_PASSWORD=admin
  export RUSTSYNC_PORT=8023
  export RUSTSYNC_EXPIRES=2
  export RUSTSYNC_LOG_LEVEL=1
  export RUSTSYNC_CONSOLE_LEVEL=2
  export RUSTSYNC_LOG_SAVE=7
  export RUSTSYNC_TASK_SAVE=0
  export RUSTSYNC_TASK_TIMEOUT=72

  # CWD = CONF_DIR
  cd "$CONF_DIR"
  "$RUSTSYNC_BIN" >> "$LOG_FILE" 2>&1 &
  log "RustSync 已启动, PID=$!"
}

# 首次启动
start_rustsync

# 守护循环：每30秒检测一次，崩溃自动重启，检测到 disable 文件则退出
while true; do
  sleep 30

  # 检测 Magisk 模块是否被禁用
  if [ -f "$MODDIR/disable" ]; then
    pkill -f "$RUSTSYNC_BIN" 2>/dev/null
    log "检测到模块已禁用，RustSync 已停止"
    exit 0
  fi

  if ! pgrep -f "$RUSTSYNC_BIN" > /dev/null 2>&1; then
    log "RustSync 进程已退出，正在重启..."
    start_rustsync
  fi
done