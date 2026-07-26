# RustSync

自动化存储同步服务，支持多引擎、多存储驱动、定时调度与通知。

## 功能特性

- **多引擎支持** — 兼容 AList 引擎，内置 RustSync 本地引擎
- **多存储驱动** — 本地文件系统、SMB/CIFS、FTP、SFTP、阿里云盘
- **灵活调度** — 间隔执行 / Cron 表达式 / 手动触发，支持文件大小过滤与排除规则
- **多通知渠道** — 自定义 Webhook、Server 酱、钉钉、企业微信、飞书、邮件
- **文件管理** — 内置文件浏览器，支持上传、下载、编辑、复制、删除、重命名
- **Web 管理面板** — Vue 3 + Element Plus 响应式界面，支持中英文切换
- **日志查看** — 在线查看运行日志，支持清空

## 部署方式

### Docker

#### docker run

```bash
docker run -d \
  --name rustsync \
  --network host \
  -v ./data:/app/data \
  -e RUSTSYNC_PASSWORD=admin \
  ojbkxc/rustsync:latest
```

#### docker-compose

在项目根目录创建 `docker-compose.yml`：

```yaml
services:
  rustsync:
    image: ojbkxc/rustsync:latest
    container_name: rustsync
    restart: always
    network_mode: host
    # ports:
    #   - "8023:8023"  # 左侧的8023可以修改为你本机未被占用的端口
    volumes:
      - ./data:/app/data
    environment:
      - RUSTSYNC_PORT=8023
      - RUSTSYNC_PASSWORD=admin
      - RUSTSYNC_EXPIRES=2
      - RUSTSYNC_LOG_LEVEL=1
      - RUSTSYNC_CONSOLE_LEVEL=2
      - RUSTSYNC_LOG_SAVE=7
      - RUSTSYNC_TASK_SAVE=0
      - RUSTSYNC_TASK_TIMEOUT=72
      - TZ=Asia/Shanghai
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:8023/"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10s
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

```bash
# 下载 docker-compose.yml
wget https://raw.githubusercontent.com/ojbkxc/RustSync/main/docker-compose.yml

# 启动
docker compose up -d
```

> 默认使用 `network_mode: host`，如需改用端口映射，注释掉 `network_mode: host` 并取消 `ports` 的注释。

### Magisk 模块 (Android)

从 [Releases](https://github.com/ojbkxc/RustSync/releases) 下载 `RustSync-*.zip`，Magisk App → 模块 → 从本地安装 → 重启设备。

- **开机自启** — 刷入后自动运行，无需额外操作
- **进程守护** — 每 30 秒检测进程状态，崩溃自动重启
- **开关控制** — 关闭模块 30 秒内自动停服，无需重启；重新开启后重启恢复
- 兼容 Magisk v20.4+

### 独立二进制

从 [Releases](https://github.com/ojbkxc/RustSync/releases) 下载对应平台二进制，直接运行：

```bash
./rustsync-server
```

浏览器访问 `http://127.0.0.1:8023`，默认密码 `admin`。

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `RUSTSYNC_PASSWORD` | `admin` | 管理员密码 |
| `RUSTSYNC_PORT` | `8023` | Web 服务端口 |
| `RUSTSYNC_EXPIRES` | `2` | 登录过期时间（天） |
| `RUSTSYNC_LOG_LEVEL` | `1` | 日志级别 |
| `RUSTSYNC_CONSOLE_LEVEL` | `2` | 控制台日志级别 |
| `RUSTSYNC_LOG_SAVE` | `7` | 日志保留天数 |
| `RUSTSYNC_TASK_SAVE` | `0` | 任务记录保留天数（0=不限制） |
| `RUSTSYNC_TASK_TIMEOUT` | `72` | 单任务超时时间（小时） |

## 技术栈

| 组件 | 技术 |
|------|------|
| 后端 | Rust (axum + rusqlite + tokio) |
| 前端 | Vue 3 + Element Plus + Vite |
| 数据库 | SQLite |
| 认证 | JWT (bcrypt) |
| 调度 | cron |
| 通知 | reqwest + lettre (SMTP) |

## 目录结构

### Android (Magisk)

| 路径 | 说明 |
|------|------|
| `/data/adb/rustsync/data/` | 数据目录（数据库、密钥） |
| `/data/adb/rustsync/rustsync.log` | 运行日志 |

### Docker / 二进制

| 路径 | 说明 |
|------|------|
| `./data/` | 数据目录（数据库、密钥） |
| `./log/` | 日志目录 |

## 构建

### 本地构建

```bash
# 前端
cd rustsync/web && npm ci && npm run build

# 后端
cd server && cargo build --release
```

### GitHub Actions

| 工作流 | 触发方式 | 产物 |
|--------|----------|------|
| **Build & Release** | 推送 `v*` 标签 / 手动触发 | 多平台二进制 + Release |
| **Magisk** | 推送 `v*` 标签 / 手动触发 | Magisk 模块 zip |
| **Pre-release v1.1.0** | 推送到 main 分支（自动） | 全部产物 + Docker 镜像 + Pre-release |
| **Release to Docker Hub** | 手动触发 | Docker 镜像 |
| **CI** | PR / 推送到 main | 编译检查 |

> **预发布机制**: 每次推送代码到 main 分支，自动构建全部平台产物并更新 `v1.1.0` 预发布版本，同时推送 Docker 镜像 `ojbkxc/rustsync:latest` 和 `ojbkxc/rustsync:1.1.0`。可在 [Releases](https://github.com/ojbkxc/RustSync/releases) 和 [Tags](https://github.com/ojbkxc/RustSync/tags) 页面查看最新预发布内容。

### 支持平台

| 平台 | 架构 |
|------|------|
| Linux | amd64, arm64 |
| Windows | amd64 |
| macOS | x86_64, arm64 |
| Android | arm64-v8a |

## 常见问题

**Q: 如何查看服务是否正常运行？**

```bash
# Android
ps -ef | grep rustsync

# Docker
docker ps | grep rustsync
```

**Q: 如何查看日志？**

```bash
# Android
cat /data/adb/rustsync/rustsync.log

# Docker
docker logs rustsync
```

**Q: 忘记管理员密码？**

删除数据目录后重启，密码恢复为 `admin`。

```bash
# Android
rm -rf /data/adb/rustsync/data

# Docker
docker compose down && rm -rf ./data && docker compose up -d
```

## License

MIT