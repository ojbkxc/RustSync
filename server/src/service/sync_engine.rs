use crate::driver::base::{FileEntry, SyncOperation};

/// 执行作业同步的核心逻辑
pub async fn run_sync_for_job(job_id: i64) -> anyhow::Result<()> {
    tracing::info!("执行作业 {} 同步逻辑", job_id);

    // 从数据库加载作业配置
    let state = crate::state::get_global_state();
    let job = {
        let db = state.db.lock().unwrap();
        db.query_row(
            "SELECT id, srcPath, dstPath, alistId, method, sourceMode, exclude, minFileSize, maxFileSize
             FROM job WHERE id=?",
            [job_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, i32>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                ))
            },
        )?
    }; // MutexGuard dropped here

    let (_id, src_path, dst_path, _alist_id, method, source_mode, exclude, min_size, max_size) = job;

    // dst_path 可能包含冒号分隔的多路径（用于扫描），取第一个路径作为写入目标
    let dst_root = dst_path.split(':').next().unwrap_or(&dst_path).trim().to_string();

    // 创建任务记录
    let task_id = {
        let db = state.db.lock().unwrap();
        let ts = crate::service::db::now_ts();
        db.execute(
            "INSERT INTO job_task (jobId, status, runTime) VALUES (?, 1, ?)",
            rusqlite::params![job_id, ts],
        )?;
        db.last_insert_rowid()
    }; // MutexGuard dropped here

    tracing::info!("作业 {}: src={}, dst={}, method={}, taskId={}", job_id, src_path, dst_path, method, task_id);

    // 解析排除规则
    let exclude_patterns: Vec<String> = exclude
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    // 扫描源目录（spawn_blocking 避免阻塞异步运行时）
    tracing::info!("扫描源目录: {}", src_path);
    let src_path_clone = src_path.clone();
    let exclude_clone = exclude_patterns.clone();
    let src_entries = tokio::task::spawn_blocking(move || {
        scan_local_directory(&src_path_clone, &exclude_clone, min_size, max_size)
    }).await??;

    // 扫描目标目录（spawn_blocking 避免阻塞异步运行时）
    let dst_path_clone = dst_path.clone();
    let dst_entries: Vec<FileEntry> = tokio::task::spawn_blocking(move || {
        let mut entries = vec![];
        for dst in dst_path_clone.split(':') {
            let dst = dst.trim();
            if let Ok(e) = scan_local_directory(dst, &[], None, None) {
                entries.extend(e);
            }
        }
        entries
    }).await?;

    tracing::info!(
        "作业 {}: 源文件 {} 个, 目标文件 {} 个",
        job_id,
        src_entries.len(),
        dst_entries.len()
    );

    // 对比差异，生成操作列表
    let operations = compare_entries(&src_entries, &dst_entries, method, source_mode != 0);

    tracing::info!("作业 {}: 需要执行 {} 个操作", job_id, operations.len());

    if operations.is_empty() {
        tracing::info!("作业 {}: 无需同步", job_id);
        // 更新任务状态为成功
        let db = state.db.lock().unwrap();
        let _ = db.execute(
            "UPDATE job_task SET status=2, taskNum=? WHERE id=?",
            rusqlite::params![serde_json::json!({"total": 0, "success": 0}).to_string(), task_id],
        );
        return Ok(());
    }

    // 执行操作（传入源和目标根目录以构造完整路径）
    let total = operations.len();
    let mut success = 0;
    let mut failed = 0;

    for op in &operations {
        // 创建逐文件操作记录（与 Python CopyItem 一致）
        let (src_rel, dst_rel, file_name, file_size, op_type) = match op {
            SyncOperation::Copy { src, dst, size } => {
                let name = std::path::Path::new(src)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                (src.clone(), dst.clone(), name, *size, 0i32)
            }
            SyncOperation::Move { src, dst, size } => {
                let name = std::path::Path::new(src)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                (src.clone(), dst.clone(), name, *size, 2i32)
            }
            SyncOperation::Delete { path, is_dir: _ } => {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                (String::new(), path.clone(), name, 0i64, 1i32)
            }
        };

        let item_id = {
            let db = state.db.lock().unwrap();
            let ts = crate::service::db::now_ts();
            db.execute(
                "INSERT INTO job_task_item (taskId, srcPath, dstPath, isPath, fileName, fileSize, type, status, createTime)
                 VALUES (?, ?, ?, 0, ?, ?, ?, 0, ?)",
                rusqlite::params![task_id, src_rel, dst_rel, file_name, file_size, op_type, ts],
            ).ok();
            db.last_insert_rowid()
        };

        match execute_operation(op, &src_path, &dst_root).await {
            Ok(_) => {
                success += 1;
                // 更新为成功状态
                let db = state.db.lock().unwrap();
                let _ = db.execute(
                    "UPDATE job_task_item SET status=2 WHERE id=?",
                    [item_id],
                );
            }
            Err(e) => {
                failed += 1;
                let err_msg = format!("{}", e);
                tracing::error!("作业 {} 操作失败: {:?}, 错误: {}", job_id, op, e);
                // 更新为失败状态
                let db = state.db.lock().unwrap();
                let _ = db.execute(
                    "UPDATE job_task_item SET status=7, errMsg=? WHERE id=?",
                    rusqlite::params![err_msg, item_id],
                );
            }
        }
    }

    // 更新任务统计
    let db = state.db.lock().unwrap();
    let task_status = if failed == 0 { 2 } else if success > 0 { 3 } else { 6 };
    let _ = db.execute(
        "UPDATE job_task SET status=?, taskNum=? WHERE id=?",
        rusqlite::params![
            task_status,
            serde_json::json!({"total": total, "success": success, "failed": failed}).to_string(),
            task_id
        ],
    );

    tracing::info!("作业 {} 完成: 成功 {}/{}, 失败 {}", job_id, success, total, failed);
    Ok(())
}

/// 扫描本地目录
fn scan_local_directory(
    path: &str,
    exclude_patterns: &[String],
    min_size: Option<i64>,
    max_size: Option<i64>,
) -> anyhow::Result<Vec<FileEntry>> {
    let mut entries = vec![];
    let dir_path = std::path::Path::new(path);

    if !dir_path.exists() {
        return Ok(entries);
    }

    scan_dir_recursive(dir_path, dir_path, &mut entries, exclude_patterns, min_size, max_size)?;
    Ok(entries)
}

fn scan_dir_recursive(
    root: &std::path::Path,
    current: &std::path::Path,
    entries: &mut Vec<FileEntry>,
    exclude_patterns: &[String],
    min_size: Option<i64>,
    max_size: Option<i64>,
) -> anyhow::Result<()> {
    if !current.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // 计算相对路径
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        // 检查排除规则
        if exclude_patterns.iter().any(|p| {
            let p = p.trim().trim_start_matches('/');
            rel_path == p || rel_path.starts_with(p) || name == p
        }) {
            continue;
        }

        let meta = entry.metadata()?;
        let is_dir = meta.is_dir();
        let size = meta.len() as i64;

        // 文件大小过滤
        if !is_dir {
            if let Some(min) = min_size {
                if size < min {
                    continue;
                }
            }
            if let Some(max) = max_size {
                if size > max {
                    continue;
                }
            }
        }

        let modified = meta.modified().ok().map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        entries.push(FileEntry {
            path: rel_path.clone(),
            name,
            is_dir,
            size,
            modified,
            fingerprint: None,
        });

        if is_dir {
            scan_dir_recursive(
                root,
                &path,
                entries,
                exclude_patterns,
                min_size,
                max_size,
            )?;
        }
    }
    Ok(())
}

/// 对比源和目标，生成同步操作
fn compare_entries(
    src: &[FileEntry],
    dst: &[FileEntry],
    method: i32,
    _source_mode: bool,
) -> Vec<SyncOperation> {
    let mut ops = vec![];

    // 构建目标索引
    let dst_index: std::collections::HashMap<&str, &FileEntry> =
        dst.iter().map(|e| (e.path.as_str(), e)).collect();

    // 移动模式: 只生成 Move 操作，跳过 Copy
    let is_move_mode = method == 2;

    for src_entry in src {
        if let Some(dst_entry) = dst_index.get(src_entry.path.as_str()) {
            if src_entry.is_dir {
                continue;
            }
            // 文件已存在，比较大小
            if src_entry.size != dst_entry.size {
                if is_move_mode {
                    ops.push(SyncOperation::Move {
                        src: src_entry.path.clone(),
                        dst: src_entry.path.clone(),
                        size: src_entry.size,
                    });
                } else {
                    ops.push(SyncOperation::Copy {
                        src: src_entry.path.clone(),
                        dst: src_entry.path.clone(),
                        size: src_entry.size,
                    });
                }
            }
        } else {
            // 目标中不存在
            if is_move_mode {
                if !src_entry.is_dir {
                    ops.push(SyncOperation::Move {
                        src: src_entry.path.clone(),
                        dst: src_entry.path.clone(),
                        size: src_entry.size,
                    });
                }
            } else {
                ops.push(SyncOperation::Copy {
                    src: src_entry.path.clone(),
                    dst: src_entry.path.clone(),
                    size: if src_entry.is_dir { 0 } else { src_entry.size },
                });
            }
        }
    }

    // 全同步模式：删除目标中多余的文件
    if method == 1 {
        let src_index: std::collections::HashSet<&str> =
            src.iter().map(|e| e.path.as_str()).collect();

        for dst_entry in dst {
            if !src_index.contains(dst_entry.path.as_str()) {
                ops.push(SyncOperation::Delete {
                    path: dst_entry.path.clone(),
                    is_dir: dst_entry.is_dir,
                });
            }
        }
    }

    ops
}

/// 执行单个同步操作
/// src_root: 源目录根路径，dst_root: 目标目录根路径
async fn execute_operation(
    op: &SyncOperation,
    src_root: &str,
    dst_root: &str,
) -> anyhow::Result<()> {
    use std::path::Path;
    match op {
        SyncOperation::Copy { src, dst, size: _ } => {
            let src_full = Path::new(src_root).join(src);
            let dst_full = Path::new(dst_root).join(dst);
            if src_full.is_dir() {
                // 目录：创建目标目录即可
                tokio::fs::create_dir_all(&dst_full).await?;
                tracing::debug!("创建目录: {}", dst_full.display());
            } else {
                if let Some(parent) = dst_full.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::copy(&src_full, &dst_full).await?;
                tracing::debug!("复制: {} -> {}", src_full.display(), dst_full.display());
            }
        }
        SyncOperation::Delete { path, is_dir } => {
            let full_path = Path::new(dst_root).join(path);
            if *is_dir {
                if full_path.is_dir() {
                    tokio::fs::remove_dir_all(&full_path).await?;
                }
            } else {
                tokio::fs::remove_file(&full_path).await?;
            }
            tracing::debug!("删除: {}", full_path.display());
        }
        SyncOperation::Move { src, dst, size: _ } => {
            let src_full = Path::new(src_root).join(src);
            let dst_full = Path::new(dst_root).join(dst);
            if let Some(parent) = dst_full.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            // 先尝试 rename（同文件系统），失败则 copy + delete
            if tokio::fs::rename(&src_full, &dst_full).await.is_err() {
                tokio::fs::copy(&src_full, &dst_full).await?;
                tokio::fs::remove_file(&src_full).await?;
            }
            tracing::debug!("移动: {} -> {}", src_full.display(), dst_full.display());
        }
    }
    Ok(())
}