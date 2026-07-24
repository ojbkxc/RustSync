use async_trait::async_trait;
use super::base::{FileEntry, StorageDriver};

// FTP 驱动骨架 - 完整实现需要 suppaftp 异步客户端

pub struct FtpDriver {
    host: String,
    port: u16,
    username: String,
    password: String,
    root: String,
}

impl FtpDriver {
    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {
        let host = config.get("host").and_then(|v| v.as_str()).unwrap_or("localhost").to_string();
        let port = config.get("port").and_then(|v| v.as_u64()).unwrap_or(21) as u16;
        let username = config.get("username").and_then(|v| v.as_str()).unwrap_or("anonymous").to_string();
        let password = config.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let root = config.get("root").and_then(|v| v.as_str()).unwrap_or("/").to_string();

        Ok(Self { host, port, username, password, root })
    }
}

#[async_trait]
impl StorageDriver for FtpDriver {
    fn name(&self) -> &str { "ftp" }

    async fn list_dir(&self, _path: &str) -> anyhow::Result<Vec<FileEntry>> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn list_all(&self, _path: &str) -> anyhow::Result<Vec<FileEntry>> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn exists(&self, _path: &str) -> anyhow::Result<bool> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn metadata(&self, _path: &str) -> anyhow::Result<FileEntry> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn create_dir(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn read_file(&self, _path: &str) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn write_file(&self, _path: &str, _data: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn delete_file(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn delete_dir(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }

    async fn copy_file(&self, _src: &str, _dst: &str) -> anyhow::Result<()> {
        anyhow::bail!("FTP 驱动完整实现需要 suppaftp 异步客户端，当前为骨架")
    }
}