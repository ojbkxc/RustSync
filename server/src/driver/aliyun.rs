use async_trait::async_trait;
use super::base::{FileEntry, StorageDriver};

// 阿里云盘驱动骨架 - 完整实现需要 OAuth 流程和 OpenFile API

pub struct AliyunDriver {
    client_id: String,
    client_secret: String,
    refresh_token: String,
    root: String,
}

impl AliyunDriver {
    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {
        let client_id = config.get("client_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let client_secret = config.get("client_secret").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let refresh_token = config.get("refresh_token").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let root = config.get("root").and_then(|v| v.as_str()).unwrap_or("root").to_string();

        Ok(Self { client_id, client_secret, refresh_token, root })
    }
}

#[async_trait]
impl StorageDriver for AliyunDriver {
    fn name(&self) -> &str { "aliyun" }

    async fn list_dir(&self, _path: &str) -> anyhow::Result<Vec<FileEntry>> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn list_all(&self, _path: &str) -> anyhow::Result<Vec<FileEntry>> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn exists(&self, _path: &str) -> anyhow::Result<bool> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn metadata(&self, _path: &str) -> anyhow::Result<FileEntry> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn create_dir(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn read_file(&self, _path: &str) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn write_file(&self, _path: &str, _data: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn delete_file(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn delete_dir(&self, _path: &str) -> anyhow::Result<()> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }

    async fn copy_file(&self, _src: &str, _dst: &str) -> anyhow::Result<()> {
        anyhow::bail!("阿里云盘驱动完整实现需要 OAuth 和 OpenFile API，当前为骨架")
    }
}