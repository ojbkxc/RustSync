# RustSync

Rust 重写版文件同步管理服务，Magisk 模块形式在 Android 设备上开机自启动。

## 功能特性

- **开机自启** — 刷入后重启自动运行，无需任何操作
- **进程守护** — 每 30 秒检测进程状态，崩溃自动重启
- **Magisk 开关控制** — 关闭模块 30 秒内自动停服，无需重启；重新开启后重启恢复
- **默认密码** — 首次启动密码为 `admin`
- **兼容 Magisk v20.4+**

## 安装

1. 从 [Releases](https://github.com/ojbkxc/RustSync/releases) 下载最新版本的 zip 文件
2. Magisk App → 模块 → 从本地安装
3. 重启设备

## 使用

### 访问管理面板

同一局域网浏览器访问 `http://手机IP:8023`，默认密码 `admin`。

### Magisk 开关控制

- **关闭**：Magisk 中关闭模块 → 守护进程 30 秒内检测到 `disable` 文件 → 自动停止服务，**无需重启**
- **开启**：Magisk 中重新开启 → 重启设备 → 服务自动恢复

### 环境变量

| 变量 | 值 | 说明 |
|------|-----|------|
| `RUSTSYNC_PASSWORD` | `admin` | 管理员密码 |
| `RUSTSYNC_PORT` | `8023` | Web 服务端口 |
| `RUSTSYNC_EXPIRES` | `2` | 登录过期时间（小时） |
| `RUSTSYNC_LOG_LEVEL` | `1` | 日志级别 |
| `RUSTSYNC_CONSOLE_LEVEL` | `2` | 控制台日志级别 |
| `RUSTSYNC_LOG_SAVE` | `7` | 日志保留天数 |
| `RUSTSYNC_TASK_SAVE` | `0` | 任务记录保留天数 |
| `RUSTSYNC_TASK_TIMEOUT` | `72` | 任务超时时间（小时） |

## 目录

| 路径 | 说明 |
|------|------|
| `/data/adb/rustsync/data/` | 数据目录（rustsync.db、secret.key） |
| `/data/adb/rustsync/rustsync.log` | 运行日志 |

## 构建

GitHub Actions 自动构建：

| 组件 | 编译方式 |
|------|----------|
| 服务端 | cargo-ndk (aarch64-linux-android) |
| 前端 | npm (Vue) |

- 推送 `v*` 标签自动触发发版
- 或手动触发 "Build & Release" 工作流

## 常见问题

**Q: 如何查看 RustSync 是否正常运行？**

```bash
ps -ef | grep rustsync
```

**Q: 如何查看日志？**

```bash
cat /data/adb/rustsync/rustsync.log
```

**Q: 如何手动重启？**

```bash
pkill -f rustsync_bin
# 守护进程会在 30 秒内自动重启
```

**Q: 忘记管理员密码？**

```bash
# 删除数据目录重新初始化（丢失所有配置）
rm -rf /data/adb/rustsync/data
# 重启后密码恢复为 admin
```

## License

MIT