use async_trait::async_trait;
use super::base::{FileEntry, StorageDriver};

// SMB 驱动骨架 - 完整实现需要 pavao 或 smb crate

#[allow(dead_code)]
pub struct SmbDriver {
    host: String,
    port: u16,
    share: String,
    username: String,
    password: String,
    root: String,
}

impl SmbDriver {
    #[allow(dead_code)]
    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {
        let host = config.get("host").and_then(|v| v.as_str()).unwrap_or("localhost").to_string();
        let port = config.get("port").and_then(|v| v.as_u64()).unwrap_or(445) as u16;
        let share = config.get("share").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let username = config.get("username").and_then(|v| v.as_str()).unwrap_or("guest").to_string();
        let password = config.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let root = config.get("root_path").and_then(|v| v.as_str()).unwrap_or("/").to_string();

        Ok(Self { host, port, share, username, password, root })
    }
}

#[async_trait]
impl StorageDriver for SmbDriver {
    fn name(&self) -> &str { "smb" }

    async fn list_dir(&self, _path: &str) -> anyhow::Result<Vec<FileEntry>> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn list_all(&self, _path: &str) -> anyhow::Result<Vec<FileEntry>> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn exists(&self, _path: &str) -> anyhow::Result<bool> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn metadata(&self, _path: &str) -> anyhow::Result<FileEntry> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn create_dir(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn read_file(&self, _path: &str) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn write_file(&self, _path: &str, _data: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn delete_file(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn delete_dir(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }

    async fn copy_file(&self, _src: &str, _dst: &str) -> anyhow::Result<()> {
        anyhow::bail!("SMB 驱动完整实现需要 pavao 或 smb crate，当前为骨架")
    }
}