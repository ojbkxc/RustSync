use async_trait::async_trait;
use std::path::PathBuf;
use super::base::{FileEntry, StorageDriver};

#[allow(dead_code)]
pub struct LocalDriver {
    root: PathBuf,
}

impl LocalDriver {
    #[allow(dead_code)]
    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {
        let path = config.get("root_path")
            .and_then(|v| v.as_str())
            .unwrap_or("/");
        let root = PathBuf::from(path);
        if !root.exists() {
            std::fs::create_dir_all(&root)?;
        }
        Ok(Self { root })
    }

    #[allow(dead_code)]
    fn full_path(&self, path: &str) -> PathBuf {
        let clean = path.trim_start_matches('/');
        self.root.join(clean)
    }
}

#[async_trait]
impl StorageDriver for LocalDriver {
    fn name(&self) -> &str {
        "local"
    }

    async fn list_dir(&self, path: &str) -> anyhow::Result<Vec<FileEntry>> {
        let full = self.full_path(path);
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let mut entries = vec![];
            if full.is_dir() {
                for entry in std::fs::read_dir(&full)? {
                    let entry = entry?;
                    let meta = entry.metadata()?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    entries.push(FileEntry {
                        path: format!("{}/{}", path.trim_end_matches('/'), name),
                        name,
                        is_dir: meta.is_dir(),
                        size: meta.len() as i64,
                        modified: meta.modified().ok()
                            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64),
                        fingerprint: None,
                    });
                }
            }
            Ok(entries)
        }).await?
    }

    async fn list_all(&self, path: &str) -> anyhow::Result<Vec<FileEntry>> {
        let root = self.root.clone();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let mut all = vec![];
            let driver = LocalDriver { root };
            driver.list_all_recursive(&path, &mut all)?;
            Ok(all)
        }).await?
    }

    async fn exists(&self, path: &str) -> anyhow::Result<bool> {
        let full = self.full_path(path);
        Ok(full.exists())
    }

    async fn metadata(&self, path: &str) -> anyhow::Result<FileEntry> {
        let full = self.full_path(path);
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let meta = std::fs::metadata(&full)?;
            let name = full.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(FileEntry {
                path,
                name,
                is_dir: meta.is_dir(),
                size: meta.len() as i64,
                modified: meta.modified().ok()
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64),
                fingerprint: None,
            })
        }).await?
    }

    async fn create_dir(&self, path: &str) -> anyhow::Result<()> {
        let full = self.full_path(path);
        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&full)
        }).await??;
        Ok(())
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let full = self.full_path(path);
        let data = tokio::task::spawn_blocking(move || {
            std::fs::read(&full)
        }).await??;
        Ok(data)
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> anyhow::Result<()> {
        let full = self.full_path(path);
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full, &data)?;
            Ok(())
        }).await?
    }

    async fn delete_file(&self, path: &str) -> anyhow::Result<()> {
        let full = self.full_path(path);
        tokio::task::spawn_blocking(move || {
            if full.is_file() {
                std::fs::remove_file(&full)?;
            }
            Ok(())
        }).await?
    }

    async fn delete_dir(&self, path: &str) -> anyhow::Result<()> {
        let full = self.full_path(path);
        tokio::task::spawn_blocking(move || {
            if full.is_dir() {
                std::fs::remove_dir_all(&full)?;
            }
            Ok(())
        }).await?
    }

    async fn copy_file(&self, src: &str, dst: &str) -> anyhow::Result<()> {
        let src_full = self.full_path(src);
        let dst_full = self.full_path(dst);
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = dst_full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src_full, &dst_full)?;
            Ok(())
        }).await?
    }
}

impl LocalDriver {
    #[allow(dead_code)]
    fn list_all_recursive(&self, path: &str, result: &mut Vec<FileEntry>) -> anyhow::Result<()> {
        let full = self.full_path(path);
        if !full.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&full)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_path = format!("{}/{}", path.trim_end_matches('/'), name);
            let file_entry = FileEntry {
                path: rel_path.clone(),
                name,
                is_dir: meta.is_dir(),
                size: meta.len() as i64,
                modified: meta.modified().ok()
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64),
                fingerprint: None,
            };
            if file_entry.is_dir {
                self.list_all_recursive(&rel_path, result)?;
            }
            result.push(file_entry);
        }
        Ok(())
    }
}