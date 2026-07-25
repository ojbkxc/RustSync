use std::sync::{Arc, OnceLock};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use crate::config::Config;

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct AppState {
    pub config: Config,
    pub db: DbPool,
}

pub type SharedState = Arc<AppState>;

static GLOBAL_STATE: OnceLock<SharedState> = OnceLock::new();

impl AppState {
    pub fn new(config: Config) -> anyhow::Result<SharedState> {
        let manager = SqliteConnectionManager::file(&config.db_path);
        let pool = Pool::builder()
            .max_size(8)
            .build(manager)?;

        // 初始化连接池中的连接
        {
            let conn = pool.get()?;
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        }

        Ok(Arc::new(Self { config, db: pool }))
    }

    pub fn new_shared(config: Config) -> anyhow::Result<SharedState> {
        let state = Self::new(config)?;
        let _ = GLOBAL_STATE.set(state.clone());
        Ok(state)
    }
}

pub fn get_global_state() -> SharedState {
    GLOBAL_STATE.get().expect("全局状态未初始化").clone()
}