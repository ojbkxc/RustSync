use async_trait::async_trait;

/// 文件条目信息
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: i64,
    pub modified: Option<i64>,
    pub fingerprint: Option<String>,
}

/// 同步操作类型
#[derive(Debug, Clone, PartialEq)]
pub enum SyncOperation {
    Copy {
        src: String,
        dst: String,
        size: i64,
    },
    Delete {
        path: String,
        is_dir: bool,
    },
    Move {
        src: String,
        dst: String,
        size: i64,
    },
}

/// 存储驱动接口
#[allow(dead_code)]
#[async_trait]
pub trait StorageDriver: Send + Sync {
    /// 获取驱动名称
    fn name(&self) -> &str;

    /// 列出目录下的所有文件和子目录
    async fn list_dir(&self, path: &str) -> anyhow::Result<Vec<FileEntry>>;

    /// 递归列出所有文件（用于扫描）
    async fn list_all(&self, path: &str) -> anyhow::Result<Vec<FileEntry>>;

    /// 检查文件/目录是否存在
    async fn exists(&self, path: &str) -> anyhow::Result<bool>;

    /// 获取文件元数据
    async fn metadata(&self, path: &str) -> anyhow::Result<FileEntry>;

    /// 创建目录
    async fn create_dir(&self, path: &str) -> anyhow::Result<()>;

    /// 读取文件内容
    async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>>;

    /// 写入文件内容
    async fn write_file(&self, path: &str, data: &[u8]) -> anyhow::Result<()>;

    /// 删除文件
    async fn delete_file(&self, path: &str) -> anyhow::Result<()>;

    /// 删除目录（递归）
    async fn delete_dir(&self, path: &str) -> anyhow::Result<()>;

    /// 复制文件（从当前驱动到当前驱动）
    async fn copy_file(&self, src: &str, dst: &str) -> anyhow::Result<()>;

    /// 获取文件指纹（用于变更检测）
    fn fingerprint(&self, entry: &FileEntry) -> String {
        format!("{}:{}:{}", entry.size, entry.is_dir, entry.modified.unwrap_or(0))
    }
}

/// 驱动工厂 - 根据配置创建驱动实例
#[allow(dead_code)]
pub async fn create_driver(driver_type: &str, config: &serde_json::Value) -> anyhow::Result<Box<dyn StorageDriver>> {
    match driver_type {
        "local" => Ok(Box::new(crate::driver::local::LocalDriver::new(config)?)),
        "ftp" => Ok(Box::new(crate::driver::ftp::FtpDriver::new(config)?)),
        "sftp" => Ok(Box::new(crate::driver::sftp::SftpDriver::new(config)?)),
        "smb" => Ok(Box::new(crate::driver::smb::SmbDriver::new(config)?)),
        "aliyun" => Ok(Box::new(crate::driver::aliyun::AliyunDriver::new(config)?)),
        _ => anyhow::bail!("不支持的驱动类型: {}", driver_type),
    }
}