use async_trait::async_trait;
use super::base::{FileEntry, StorageDriver};

// SFTP 驱动 - 完整实现需要 russh 或 ssh2 crate
// 这里提供骨架，实际使用时需要引入 SSH 库

pub struct SftpDriver {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    root: String,
}

impl SftpDriver {
    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {
        let host = config.get("host").and_then(|v| v.as_str()).unwrap_or("localhost").to_string();
        let port = config.get("port").and_then(|v| v.as_u64()).unwrap_or(22) as u16;
        let username = config.get("username").and_then(|v| v.as_str()).unwrap_or("root").to_string();
        let password = config.get("password").and_then(|v| v.as_str()).map(|s| s.to_string());
        let root = config.get("root").and_then(|v| v.as_str()).unwrap_or("/").to_string();

        Ok(Self { host, port, username, password, root })
    }
}

#[async_trait]
impl StorageDriver for SftpDriver {
    fn name(&self) -> &str { "sftp" }

    async fn list_dir(&self, _path: &str) -> anyhow::Result<Vec<FileEntry>> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn list_all(&self, _path: &str) -> anyhow::Result<Vec<FileEntry>> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn exists(&self, _path: &str) -> anyhow::Result<bool> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn metadata(&self, _path: &str) -> anyhow::Result<FileEntry> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn create_dir(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn read_file(&self, _path: &str) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn write_file(&self, _path: &str, _data: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn delete_file(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn delete_dir(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }

    async fn copy_file(&self, _src: &str, _dst: &str) -> anyhow::Result<()> {
        anyhow::bail!("SFTP 驱动需要 russh 或 ssh2 crate，当前为骨架实现")
    }
}