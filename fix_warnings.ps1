# Fix config.rs
$content = Get-Content -Path "C:\GitHub\RustSync\server\src\config.rs" -Raw
$content = $content.Replace('pub const MOD_DIR: &str = "/data/adb/modules/rustsync_magisk";', '#[allow(dead_code)]' + "`r`n" + 'pub const MOD_DIR: &str = "/data/adb/modules/rustsync_magisk";')
Set-Content -Path "C:\GitHub\RustSync\server\src\config.rs" -Value $content -NoNewline
Write-Output "Fixed config.rs"

# Fix models.rs - StorageMount
$content = Get-Content -Path "C:\GitHub\RustSync\server\src\data\models.rs" -Raw
$content = $content.Replace(
    '#[derive(Debug, Serialize, Deserialize, Clone)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct StorageMount {',
    '#[allow(dead_code)]' + "`r`n" + '#[derive(Debug, Serialize, Deserialize, Clone)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct StorageMount {'
)
$content = $content.Replace(
    '#[derive(Debug, Deserialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct StorageMountRequest {',
    '#[allow(dead_code)]' + "`r`n" + '#[derive(Debug, Deserialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct StorageMountRequest {'
)
$content = $content.Replace(
    'fn default_enabled() -> bool {' + "`r`n" + '    true' + "`r`n" + '}',
    '#[allow(dead_code)]' + "`r`n" + 'fn default_enabled() -> bool {' + "`r`n" + '    true' + "`r`n" + '}'
)
$content = $content.Replace(
    '#[derive(Debug, Deserialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct JobRequest {',
    '#[allow(dead_code)]' + "`r`n" + '#[derive(Debug, Deserialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct JobRequest {'
)
$content = $content.Replace(
    '#[derive(Debug, Deserialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct NotifyRequest {',
    '#[allow(dead_code)]' + "`r`n" + '#[derive(Debug, Deserialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct NotifyRequest {'
)
$content = $content.Replace(
    '#[derive(Debug, Deserialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct PageRequest {',
    '#[allow(dead_code)]' + "`r`n" + '#[derive(Debug, Deserialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct PageRequest {'
)
$content = $content.Replace(
    'fn default_page_num() -> i32 {' + "`r`n" + '    1' + "`r`n" + '}' + "`r`n" + 'fn default_page_size() -> i32 {',
    '#[allow(dead_code)]' + "`r`n" + 'fn default_page_num() -> i32 {' + "`r`n" + '    1' + "`r`n" + '}' + "`r`n" + '#[allow(dead_code)]' + "`r`n" + 'fn default_page_size() -> i32 {'
)
$content = $content.Replace(
    '#[derive(Debug, Serialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct PageResult<T: Serialize> {',
    '#[allow(dead_code)]' + "`r`n" + '#[derive(Debug, Serialize)]' + "`r`n" + '#[serde(rename_all = "camelCase")]' + "`r`n" + 'pub struct PageResult<T: Serialize> {'
)
Set-Content -Path "C:\GitHub\RustSync\server\src\data\models.rs" -Value $content -NoNewline
Write-Output "Fixed models.rs"

# Fix base.rs
$content = Get-Content -Path "C:\GitHub\RustSync\server\src\driver\base.rs" -Raw
$content = $content.Replace(
    '/// ' + [char]0x6587 + [char]0x4EF6 + [char]0x6761 + [char]0x76EE + [char]0x4FE1 + [char]0x606F' + "`r`n" + '#[derive(Debug, Clone)]' + "`r`n" + 'pub struct FileEntry {',
    '/// ' + [char]0x6587 + [char]0x4EF6 + [char]0x6761 + [char]0x76EE + [char]0x4FE1 + [char]0x606F' + "`r`n" + '#[allow(dead_code)]' + "`r`n" + '#[derive(Debug, Clone)]' + "`r`n" + 'pub struct FileEntry {'
)
$content = $content.Replace(
    '/// ' + [char]0x5B58 + [char]0x50A8 + [char]0x9A71 + [char]0x52A8 + [char]0x63A5 + [char]0x53E3' + "`r`n" + '#[async_trait]' + "`r`n" + 'pub trait StorageDriver: Send + Sync {',
    '/// ' + [char]0x5B58 + [char]0x50A8 + [char]0x9A71 + [char]0x52A8 + [char]0x63A5 + [char]0x53E3' + "`r`n" + '#[allow(dead_code)]' + "`r`n" + '#[async_trait]' + "`r`n" + 'pub trait StorageDriver: Send + Sync {'
)
$content = $content.Replace(
    '/// ' + [char]0x9A71 + [char]0x52A8 + [char]0x5DE5 + [char]0x5382 + ' - ' + [char]0x6839 + [char]0x636E + [char]0x914D + [char]0x7F6E + [char]0x521B + [char]0x5EFA + [char]0x9A71 + [char]0x52A8 + [char]0x5B9E + [char]0x4F8B' + "`r`n" + 'pub async fn create_driver',
    '/// ' + [char]0x9A71 + [char]0x52A8 + [char]0x5DE5 + [char]0x5382 + ' - ' + [char]0x6839 + [char]0x636E + [char]0x914D + [char]0x7F6E + [char]0x521B + [char]0x5EFA + [char]0x9A71 + [char]0x52A8 + [char]0x5B9E + [char]0x4F8B' + "`r`n" + '#[allow(dead_code)]' + "`r`n" + 'pub async fn create_driver'
)
Set-Content -Path "C:\GitHub\RustSync\server\src\driver\base.rs" -Value $content -NoNewline
Write-Output "Fixed base.rs"

# Fix local.rs
$content = Get-Content -Path "C:\GitHub\RustSync\server\src\driver\local.rs" -Raw
$content = $content.Replace('pub struct LocalDriver {', '#[allow(dead_code)]' + "`r`n" + 'pub struct LocalDriver {')
$content = $content.Replace('    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {', '    #[allow(dead_code)]' + "`r`n" + '    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {')
$content = $content.Replace('    fn full_path(&self, path: &str) -> PathBuf {', '    #[allow(dead_code)]' + "`r`n" + '    fn full_path(&self, path: &str) -> PathBuf {')
$content = $content.Replace('    fn list_all_recursive(&self, path: &str, result: &mut Vec<FileEntry>) -> anyhow::Result<()> {', '    #[allow(dead_code)]' + "`r`n" + '    fn list_all_recursive(&self, path: &str, result: &mut Vec<FileEntry>) -> anyhow::Result<()> {')
Set-Content -Path "C:\GitHub\RustSync\server\src\driver\local.rs" -Value $content -NoNewline
Write-Output "Fixed local.rs"

# Fix smb.rs
$content = Get-Content -Path "C:\GitHub\RustSync\server\src\driver\smb.rs" -Raw
$content = $content.Replace('pub struct SmbDriver {', '#[allow(dead_code)]' + "`r`n" + 'pub struct SmbDriver {')
$content = $content.Replace('    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {', '    #[allow(dead_code)]' + "`r`n" + '    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {')
Set-Content -Path "C:\GitHub\RustSync\server\src\driver\smb.rs" -Value $content -NoNewline
Write-Output "Fixed smb.rs"

# Fix ftp.rs
$content = Get-Content -Path "C:\GitHub\RustSync\server\src\driver\ftp.rs" -Raw
$content = $content.Replace('pub struct FtpDriver {', '#[allow(dead_code)]' + "`r`n" + 'pub struct FtpDriver {')
$content = $content.Replace('    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {', '    #[allow(dead_code)]' + "`r`n" + '    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {')
Set-Content -Path "C:\GitHub\RustSync\server\src\driver\ftp.rs" -Value $content -NoNewline
Write-Output "Fixed ftp.rs"

# Fix sftp.rs
$content = Get-Content -Path "C:\GitHub\RustSync\server\src\driver\sftp.rs" -Raw
$content = $content.Replace('pub struct SftpDriver {', '#[allow(dead_code)]' + "`r`n" + 'pub struct SftpDriver {')
$content = $content.Replace('    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {', '    #[allow(dead_code)]' + "`r`n" + '    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {')
Set-Content -Path "C:\GitHub\RustSync\server\src\driver\sftp.rs" -Value $content -NoNewline
Write-Output "Fixed sftp.rs"

# Fix aliyun.rs
$content = Get-Content -Path "C:\GitHub\RustSync\server\src\driver\aliyun.rs" -Raw
$content = $content.Replace('pub struct AliyunDriver {', '#[allow(dead_code)]' + "`r`n" + 'pub struct AliyunDriver {')
$content = $content.Replace('    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {', '    #[allow(dead_code)]' + "`r`n" + '    pub fn new(config: &serde_json::Value) -> anyhow::Result<Self> {')
Set-Content -Path "C:\GitHub\RustSync\server\src\driver\aliyun.rs" -Value $content -NoNewline
Write-Output "Fixed aliyun.rs"

Write-Output "All done!"