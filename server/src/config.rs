use std::path::Path;
use std::sync::RwLock;

pub const DEFAULT_DATA_DIR: &str = "data";
pub const CONF_DIR: &str = "/data/adb/rustsync";
pub const MOD_DIR: &str = "/data/adb/modules/rustsync_magisk";

pub const DEFAULT_PASSWORD: &str = "RANDOM";
pub const DEFAULT_PORT: u16 = 8023;
pub const DEFAULT_EXPIRES: u32 = 2;
pub const DEFAULT_LOG_LEVEL: u32 = 1;
pub const DEFAULT_CONSOLE_LEVEL: u32 = 2;
pub const DEFAULT_LOG_SAVE: u32 = 7;
pub const DEFAULT_TASK_SAVE: u32 = 0;
pub const DEFAULT_TASK_TIMEOUT: u32 = 72;

pub fn get_listen_port() -> u16 {
    std::env::var("RUSTSYNC_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT)
}

pub fn get_data_dir() -> String {
    if Path::new(CONF_DIR).exists() {
        format!("{}/data", CONF_DIR)
    } else {
        DEFAULT_DATA_DIR.to_string()
    }
}

fn get_or_create_jwt_secret(data_dir: &str) -> String {
    let key_path = Path::new(data_dir).join("secret.key");
    if key_path.exists() {
        std::fs::read_to_string(&key_path).unwrap_or_default().trim().to_string()
    } else {
        let key = generate_random_string(256);
        let _ = std::fs::create_dir_all(data_dir);
        let _ = std::fs::write(&key_path, &key);
        key
    }
}

fn generate_random_string(length: usize) -> String {
    use rand::Rng;
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        .chars()
        .collect();
    let mut rng = rand::thread_rng();
    (0..length).map(|_| chars[rng.gen_range(0..chars.len())]).collect()
}

#[derive(Debug, Clone)]
pub struct Config {
    pub password: String,
    pub port: u16,
    pub expires: u32,
    pub log_level: u32,
    pub console_level: u32,
    pub log_save: u32,
    pub task_save: u32,
    pub task_timeout: u32,
    pub data_dir: String,
    pub log_dir: String,
    pub db_path: String,
    pub jwt_secret: String,
    pub timezone: String,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = get_data_dir();
        let jwt_secret = get_or_create_jwt_secret(&data_dir);
        Self {
            password: DEFAULT_PASSWORD.to_string(),
            port: DEFAULT_PORT,
            expires: DEFAULT_EXPIRES,
            log_level: DEFAULT_LOG_LEVEL,
            console_level: DEFAULT_CONSOLE_LEVEL,
            log_save: DEFAULT_LOG_SAVE,
            task_save: DEFAULT_TASK_SAVE,
            task_timeout: DEFAULT_TASK_TIMEOUT,
            log_dir: format!("{}/log", data_dir),
            db_path: format!("{}/rustsync.db", data_dir),
            data_dir,
            jwt_secret,
            timezone: "Asia/Shanghai".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let default = Self::default();
        Self {
            password: env_password(),
            port: env_or("RUSTSYNC_PORT", &default.port.to_string()).parse().unwrap_or(default.port),
            expires: env_or("RUSTSYNC_EXPIRES", &default.expires.to_string()).parse().unwrap_or(default.expires),
            log_level: env_or("RUSTSYNC_LOG_LEVEL", &default.log_level.to_string()).parse().unwrap_or(default.log_level),
            console_level: env_or("RUSTSYNC_CONSOLE_LEVEL", &default.console_level.to_string()).parse().unwrap_or(default.console_level),
            log_save: env_or("RUSTSYNC_LOG_SAVE", &default.log_save.to_string()).parse().unwrap_or(default.log_save),
            task_save: env_or("RUSTSYNC_TASK_SAVE", &default.task_save.to_string()).parse().unwrap_or(default.task_save),
            task_timeout: env_or("RUSTSYNC_TASK_TIMEOUT", &default.task_timeout.to_string()).parse().unwrap_or(default.task_timeout),
            timezone: env_or("TZ", &default.timezone),
            jwt_secret: default.jwt_secret,
            data_dir: default.data_dir,
            log_dir: default.log_dir,
            db_path: default.db_path,
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_password() -> String {
    std::env::var("RUSTSYNC_PASSWORD")
        .or_else(|_| std::env::var("RUSTSYNC_PASSWD"))
        .unwrap_or_else(|_| DEFAULT_PASSWORD.to_string())
}

static GLOBAL_CONFIG: RwLock<Option<Config>> = RwLock::new(None);

pub fn get_config() -> Config {
    if let Ok(guard) = GLOBAL_CONFIG.read() {
        if let Some(ref config) = *guard {
            return config.clone();
        }
    }
    if let Ok(mut guard) = GLOBAL_CONFIG.write() {
        if guard.is_none() {
            *guard = Some(Config::load());
        }
        return guard.as_ref().expect("config should be initialized").clone();
    }
    Config::default()
}