use md5::{Digest, Md5};
use rusqlite::Connection;
use crate::config::Config;

/// 当前数据库版本号（与 Python 版保持一致）
const DB_VERSION: i32 = 260718;

/// 初始化数据库表结构和迁移
pub fn init_database(conn: &Connection, config: &Config) -> anyhow::Result<Option<String>> {
    let mut generated_password: Option<String> = None;

    // 检查 user_list 表是否存在
    let user_table_exists: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE name='user_list'")
        .and_then(|mut s| s.exists([]))
        .unwrap_or(false);

    if user_table_exists {
        let user_count: i32 = conn
            .query_row("SELECT count(*) FROM user_list", [], |row| row.get(0))
            .unwrap_or(0);

        if user_count == 0 {
            // 首次初始化被中断导致空表，检查是否有业务数据
            let has_business_data = check_business_data(conn)?;
            if has_business_data {
                anyhow::bail!("database has storage data but no administrator user; refusing destructive recovery");
            }
            // 清理空表并重建
            cleanup_tables(conn)?;
            generated_password = create_tables(conn, config)?;
        } else {
            // 已有用户，执行迁移
            migrate_tables(conn)?;
        }
    } else {
        generated_password = create_tables(conn, config)?;
    }

    // 确保内置引擎存在
    ensure_builtin_engine(conn)?;

    Ok(generated_password)
}

fn check_business_data(conn: &Connection) -> anyhow::Result<bool> {
    let tables = [
        "job_source_snapshot", "job_source_snapshot_meta",
        "storage_mount", "notify", "job_task_item", "job_task", "job",
    ];
    for table in tables {
        let exists: bool = conn
            .prepare(&format!("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='{}'", table))
            .and_then(|mut s| s.exists([]))
            .unwrap_or(false);
        if exists {
            let count: i32 = conn
                .query_row(&format!("SELECT count(*) FROM {}", table), [], |row| row.get(0))
                .unwrap_or(0);
            if count > 0 {
                return Ok(true);
            }
        }
    }
    // 检查 alist_list 是否有外部引擎
    let alist_exists: bool = conn
        .prepare("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='alist_list'")
        .and_then(|mut s| s.exists([]))
        .unwrap_or(false);
    if alist_exists {
        let count: i32 = conn
            .query_row("SELECT count(*) FROM alist_list WHERE url <> 'rustsync://internal'", [], |row| row.get(0))
            .unwrap_or(0);
        if count > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn cleanup_tables(conn: &Connection) -> anyhow::Result<()> {
    let tables = [
        "job_source_snapshot", "job_source_snapshot_meta",
        "storage_mount", "notify", "job_task_item", "job_task",
        "job", "alist_list", "user_list",
    ];
    for table in tables {
        let _ = conn.execute(&format!("DROP TABLE IF EXISTS {}", table), []);
    }
    Ok(())
}

fn create_tables(conn: &Connection, config: &Config) -> anyhow::Result<Option<String>> {
    let (passwd, log_pwd) = if config.password == "RANDOM" || config.password.is_empty() {
        let pwd = generate_random_password();
        tracing::info!("生成随机管理员密码: {}", pwd);
        let _ = std::fs::create_dir_all(&config.log_dir);
        let mut log_path = std::path::PathBuf::from(&config.log_dir);
        log_path.push("password.log");
        let _ = std::fs::write(&log_path, format!("初始管理员密码: {}\n", pwd));
        (pwd.clone(), Some(pwd))
    } else {
        (config.password.clone(), None)
    };

    let pwd_hash = passwd2hash(&passwd, &config.password_str);

    conn.execute_batch(&format!(
        "CREATE TABLE user_list(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            userName TEXT,
            passwd TEXT,
            sqlVersion INTEGER DEFAULT {},
            createTime INTEGER DEFAULT (strftime('%s', 'now'))
        );
        INSERT INTO user_list(userName, passwd) VALUES ('admin', '{}');

        CREATE TABLE alist_list(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remark TEXT,
            url TEXT,
            userName TEXT,
            token TEXT,
            engineType TEXT DEFAULT 'alist',
            systemKey TEXT DEFAULT NULL,
            protected INTEGER DEFAULT 0,
            createTime INTEGER DEFAULT (strftime('%s', 'now')),
            UNIQUE (url, userName)
        );
        CREATE UNIQUE INDEX idx_alist_system_key ON alist_list(systemKey) WHERE systemKey IS NOT NULL;
        INSERT INTO alist_list (remark, url, userName, token, engineType, systemKey, protected)
            VALUES (NULL, 'rustsync://internal', 'RustSync', NULL, 'rustsync', 'rustsync', 1);

        CREATE TABLE storage_mount(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            engineId INTEGER NOT NULL,
            name TEXT NOT NULL,
            driverType TEXT NOT NULL,
            config TEXT NOT NULL,
            enabled INTEGER DEFAULT 1,
            configVersion INTEGER DEFAULT 1,
            authVersion INTEGER DEFAULT 1,
            createTime INTEGER DEFAULT (strftime('%s', 'now')),
            UNIQUE (engineId, name)
        );

        CREATE TABLE job(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            enable INTEGER DEFAULT 1,
            remark TEXT,
            srcPath TEXT,
            dstPath TEXT,
            alistId INTEGER,
            useCacheT INTEGER DEFAULT 0,
            scanIntervalT INTEGER DEFAULT 0,
            useCacheS INTEGER DEFAULT 0,
            scanIntervalS INTEGER DEFAULT 0,
            method INTEGER,
            sourceMode INTEGER DEFAULT 0,
            interval INTEGER,
            isCron INTEGER DEFAULT 0,
            year TEXT DEFAULT NULL,
            month TEXT DEFAULT NULL,
            day TEXT DEFAULT NULL,
            week TEXT DEFAULT NULL,
            day_of_week TEXT DEFAULT NULL,
            hour TEXT DEFAULT NULL,
            minute TEXT DEFAULT NULL,
            second TEXT DEFAULT NULL,
            start_date TEXT DEFAULT NULL,
            end_date TEXT DEFAULT NULL,
            exclude TEXT DEFAULT NULL,
            minFileSize INTEGER DEFAULT NULL,
            maxFileSize INTEGER DEFAULT NULL,
            createTime INTEGER DEFAULT (strftime('%s', 'now')),
            UNIQUE (srcPath, dstPath, alistId)
        );

        CREATE TABLE job_source_snapshot_meta(
            jobId INTEGER PRIMARY KEY,
            initialized INTEGER DEFAULT 0,
            scanTime INTEGER DEFAULT NULL,
            entryCount INTEGER DEFAULT 0
        );

        CREATE TABLE job_source_snapshot(
            jobId INTEGER NOT NULL,
            path TEXT NOT NULL,
            isDir INTEGER DEFAULT 0,
            size INTEGER DEFAULT NULL,
            fingerprint TEXT DEFAULT NULL,
            PRIMARY KEY (jobId, path)
        );

        CREATE TABLE job_task(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            jobId INTEGER,
            status INTEGER DEFAULT 1,
            errMsg TEXT,
            runTime INTEGER,
            taskNum TEXT,
            createTime INTEGER DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE job_task_item(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            taskId INTEGER,
            srcPath TEXT,
            dstPath TEXT,
            isPath INTEGER DEFAULT 0,
            fileName TEXT,
            fileSize INTEGER,
            type INTEGER,
            alistTaskId TEXT,
            status INTEGER DEFAULT 0,
            progress REAL,
            errMsg TEXT,
            createTime INTEGER DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE notify(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            enable INTEGER DEFAULT 1,
            method INTEGER,
            params TEXT,
            createTime INTEGER DEFAULT (strftime('%s', 'now'))
        );",
        DB_VERSION, pwd_hash
    ))?;

    Ok(log_pwd)
}

fn migrate_tables(conn: &Connection) -> anyhow::Result<()> {
    let sql_version: i32 = conn
        .query_row("SELECT sqlVersion FROM user_list LIMIT 1", [], |row| row.get(0))
        .unwrap_or(0);

    if sql_version < DB_VERSION {
        // 按版本号逐步迁移（与 Python 版保持一致）
        if sql_version < 240731 {
            conn.execute_batch(&format!(
                "ALTER TABLE user_list ADD COLUMN sqlVersion INTEGER DEFAULT {};
                 ALTER TABLE job_task ADD COLUMN errMsg TEXT;",
                DB_VERSION
            ))?;
        }
        if sql_version < 240813 {
            conn.execute_batch(
                "ALTER TABLE job ADD COLUMN isCron INTEGER DEFAULT 0;
                 ALTER TABLE job ADD COLUMN year TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN month TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN day TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN week TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN day_of_week TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN hour TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN minute TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN second TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN start_date TEXT DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN end_date TEXT DEFAULT NULL;"
            )?;
        }
        if sql_version < 240905 {
            conn.execute_batch("ALTER TABLE job ADD COLUMN exclude TEXT DEFAULT NULL;")?;
        }
        if sql_version < 241014 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS notify(
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    enable INTEGER DEFAULT 1,
                    method INTEGER,
                    params TEXT,
                    createTime INTEGER DEFAULT (strftime('%s', 'now'))
                );"
            )?;
        }
        if sql_version < 250307 {
            conn.execute_batch("ALTER TABLE job_task ADD COLUMN taskNum TEXT;")?;
        }
        if sql_version < 250416 {
            conn.execute_batch("ALTER TABLE job ADD COLUMN remark TEXT;")?;
        }
        if sql_version < 250520 {
            conn.execute_batch("ALTER TABLE job_task_item ADD COLUMN isPath INTEGER DEFAULT 0;")?;
        }
        if sql_version < 250608 {
            conn.execute_batch(
                "ALTER TABLE job ADD COLUMN useCacheT INTEGER DEFAULT 0;
                 ALTER TABLE job ADD COLUMN scanIntervalT INTEGER DEFAULT 0;
                 ALTER TABLE job ADD COLUMN useCacheS INTEGER DEFAULT 0;
                 ALTER TABLE job ADD COLUMN scanIntervalS INTEGER DEFAULT 0;
                 UPDATE job SET scanIntervalT = 10, useCacheT = 0 WHERE useCacheT = 2;"
            )?;
        }
        if sql_version < 260715 {
            conn.execute_batch(
                "ALTER TABLE job ADD COLUMN minFileSize INTEGER DEFAULT NULL;
                 ALTER TABLE job ADD COLUMN maxFileSize INTEGER DEFAULT NULL;"
            )?;
        }
        if sql_version < 260716 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS alist_list(
                    id INTEGER PRIMARY KEY AUTOINCREMENT, remark TEXT, url TEXT,
                    userName TEXT, token TEXT,
                    createTime INTEGER DEFAULT (strftime('%s', 'now')),
                    UNIQUE (url, userName));
                 ALTER TABLE alist_list ADD COLUMN engineType TEXT DEFAULT 'alist';
                 ALTER TABLE alist_list ADD COLUMN systemKey TEXT DEFAULT NULL;
                 ALTER TABLE alist_list ADD COLUMN protected INTEGER DEFAULT 0;"
            )?;
            let _ = conn.execute("UPDATE alist_list SET engineType='alist' WHERE engineType IS NULL", []);
            let _ = conn.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_alist_system_key ON alist_list(systemKey) WHERE systemKey IS NOT NULL",
                [],
            );
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS storage_mount(
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    engineId INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    driverType TEXT NOT NULL,
                    config TEXT NOT NULL,
                    enabled INTEGER DEFAULT 1,
                    configVersion INTEGER DEFAULT 1,
                    authVersion INTEGER DEFAULT 1,
                    createTime INTEGER DEFAULT (strftime('%s', 'now')),
                    UNIQUE (engineId, name));"
            )?;
        }
        if sql_version < 260717 {
            let _ = conn.execute("ALTER TABLE job ADD COLUMN sourceMode INTEGER DEFAULT 0", []);
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS job_source_snapshot_meta(
                    jobId INTEGER PRIMARY KEY,
                    initialized INTEGER DEFAULT 0,
                    scanTime INTEGER DEFAULT NULL,
                    entryCount INTEGER DEFAULT 0);
                 CREATE TABLE IF NOT EXISTS job_source_snapshot(
                    jobId INTEGER NOT NULL,
                    path TEXT NOT NULL,
                    isDir INTEGER DEFAULT 0,
                    size INTEGER DEFAULT NULL,
                    fingerprint TEXT DEFAULT NULL,
                    PRIMARY KEY (jobId, path));"
            )?;
        }
        if sql_version < 260718 {
            let _ = conn.execute(
                "ALTER TABLE job_source_snapshot ADD COLUMN fingerprint TEXT DEFAULT NULL",
                [],
            );
        }
        conn.execute(
            &format!("UPDATE user_list SET sqlVersion={}", DB_VERSION),
            [],
        )?;
    }
    Ok(())
}

fn ensure_builtin_engine(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS alist_list(
            id INTEGER PRIMARY KEY AUTOINCREMENT, remark TEXT, url TEXT,
            userName TEXT, token TEXT,
            createTime INTEGER DEFAULT (strftime('%s', 'now')),
            UNIQUE (url, userName));"
    )?;
    // 仅在列不存在时添加，避免每次启动都执行 ALTER TABLE
    let existing_cols = get_table_columns(conn, "alist_list")?;
    for (col, default_val) in &[("engineType", "alist"), ("systemKey", ""), ("protected", "0")] {
        if !existing_cols.iter().any(|c| c == col) {
            conn.execute(
                &format!("ALTER TABLE alist_list ADD COLUMN {} TEXT DEFAULT '{}'", col, default_val),
                [],
            )?;
        }
    }
    let _ = conn.execute("UPDATE alist_list SET engineType='alist' WHERE engineType IS NULL", []);
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_alist_system_key ON alist_list(systemKey) WHERE systemKey IS NOT NULL",
        [],
    );
    let exists: bool = conn
        .prepare("SELECT id FROM alist_list WHERE systemKey='rustsync' LIMIT 1")
        .and_then(|mut s| s.exists([]))
        .unwrap_or(false);
    if !exists {
        conn.execute(
            "INSERT INTO alist_list (remark, url, userName, token, engineType, systemKey, protected)
             VALUES (NULL, 'rustsync://internal', 'RustSync', NULL, 'rustsync', 'rustsync', 1)",
            [],
        )?;
    } else {
        conn.execute(
            "UPDATE alist_list SET userName='RustSync', url='rustsync://internal',
             engineType='rustsync', protected=1 WHERE systemKey='rustsync'",
            [],
        )?;
    }
    Ok(())
}

// ==================== 密码工具 ====================

/// 密码哈希 - 与 Python commonUtils.passwd2md5 一致
/// Python: hashlib.md5((passwd + passwdStr).encode()).hexdigest()
pub fn passwd2hash(password: &str, secret_key: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(password.as_bytes());
    hasher.update(secret_key.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 使用全局配置的密码哈希
pub fn hash_password(password: &str) -> String {
    let config = crate::config::get_config();
    passwd2hash(password, &config.password_str)
}

pub fn generate_random_password() -> String {
    use rand::Rng;
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();
    let mut rng = rand::thread_rng();
    (0..12).map(|_| chars[rng.gen_range(0..chars.len())]).collect()
}

/// 获取当前 Unix 时间戳
pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

/// 获取表的所有列名
fn get_table_columns(conn: &Connection, table: &str) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(cols)
}