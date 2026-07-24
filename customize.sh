SKIPMOUNT=true
PROPFILE=false
POSTFSDATA=false
LATESTARTSERVICE=true

# 复制二进制到模块根目录
# 优先使用 Rust 编译的 rustsync_server，回退到 Python 编译的 taosync
if [ -f "$MODPATH/system/bin/rustsync_server" ]; then
  cp "$MODPATH/system/bin/rustsync_server" "$MODPATH/rustsync_bin"
  ui_print "- 使用 Rust 版本"
elif [ -f "$MODPATH/system/bin/taosync" ]; then
  cp "$MODPATH/system/bin/taosync" "$MODPATH/rustsync_bin"
  ui_print "- 使用 Python 版本"
fi

# 设置执行权限
if [ -f "$MODPATH/rustsync_bin" ]; then
  set_perm $MODPATH/rustsync_bin 0 0 0755
  ui_print "- RustSync 二进制文件就绪"
else
  ui_print "! 警告：未找到 RustSync 二进制文件"
fi

# 复制前端静态文件到数据目录（Rust 版本需要）
if [ -d "$MODPATH/static" ]; then
  mkdir -p /data/adb/rustsync/static
  cp -r "$MODPATH/static/"* /data/adb/rustsync/static/ 2>/dev/null
  ui_print "- 前端静态文件已就绪"
fi

ui_print "----------------------------------"
ui_print "  RustSync 同步服务 安装成功"
ui_print "----------------------------------"
ui_print "作者：ojbkxc"
ui_print "管理面板：http://手机IP:8023"
ui_print "默认账户：admin / admin"
ui_print "配置目录：/data/adb/rustsync/data/"
ui_print "日志文件：/data/adb/rustsync/rustsync.log"
ui_print "----------------------------------"