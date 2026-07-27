use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use crate::driver::base::{FileEntry, SyncOperation};

static ABORT_FLAGS: Mutex<Option<HashSet<i64>>> = Mutex::new(None);

pub fn set_abort_flag(job_id: i64) {
    if let Ok(mut flags) = ABORT_FLAGS.lock() {
        flags.get_or_insert_with(HashSet::new).insert(job_id);
    }
}

fn is_aborted(job_id: i64) -> bool {
    ABORT_FLAGS.lock().ok().and_then(|f| f.as_ref().map(|s| s.contains(&job_id))).unwrap_or(false)
}

fn clear_abort_flag(job_id: i64) {
    if let Ok(mut flags) = ABORT_FLAGS.lock() {
        if let Some(ref mut set) = *flags { set.remove(&job_id); }
    }
}

// ==================== 指纹计算 ====================

/// 计算文件指纹（用于变更检测和快照对比）
fn compute_fingerprint(entry: &FileEntry) -> String {
    format!("{}:{}:{}", entry.size, entry.is_dir, entry.modified.unwrap_or(0))
}

// ==================== 快照管理 ====================

/// 快照条目（从数据库加载）
#[derive(Debug, Clone)]
struct SnapshotEntry {
    path: String,
    is_dir: bool,
    size: Option<i64>,
    fingerprint: Option<String>,
}

/// 加载源快照
fn get_source_snapshot(conn: &rusqlite::Connection, job_id: i64) -> Option<HashMap<String, SnapshotEntry>> {
    let initialized: i32 = conn.query_row(
        "SELECT initialized FROM job_source_snapshot_meta WHERE jobId=?",
        [job_id], |row| row.get(0),
    ).unwrap_or(0);

    if initialized != 1 {
        return None;
    }

    let mut stmt = conn.prepare(
        "SELECT path, isDir, size, fingerprint FROM job_source_snapshot WHERE jobId=?"
    ).ok()?;

    let entries: HashMap<String, SnapshotEntry> = stmt.query_map([job_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            SnapshotEntry {
                path: row.get::<_, String>(0)?,
                is_dir: row.get::<_, i32>(1)? != 0,
                size: row.get::<_, Option<i64>>(2)?,
                fingerprint: row.get::<_, Option<String>>(3)?,
            },
        ))
    }).ok()?.filter_map(|r| r.ok()).map(|(path, entry)| (path.clone(), entry)).collect();

    Some(entries)
}

/// 保存源快照
fn save_source_snapshot(conn: &rusqlite::Connection, job_id: i64, entries: &[FileEntry]) -> anyhow::Result<()> {
    conn.execute("DELETE FROM job_source_snapshot WHERE jobId=?", [job_id])?;

    let mut stmt = conn.prepare(
        "INSERT INTO job_source_snapshot (jobId, path, isDir, size, fingerprint) VALUES (?, ?, ?, ?, ?)"
    )?;

    for entry in entries {
        let fingerprint = if entry.is_dir { None } else { Some(compute_fingerprint(entry)) };
        stmt.execute(rusqlite::params![
            job_id,
            entry.path,
            entry.is_dir as i32,
            if entry.is_dir { None } else { Some(entry.size) },
            fingerprint,
        ])?;
    }

    let now = crate::service::db::now_ts();
    conn.execute(
        "INSERT INTO job_source_snapshot_meta (jobId, initialized, scanTime, entryCount) VALUES (?, 1, ?, ?)
         ON CONFLICT(jobId) DO UPDATE SET initialized=1, scanTime=excluded.scanTime, entryCount=excluded.entryCount",
        rusqlite::params![job_id, now, entries.len() as i64],
    )?;

    Ok(())
}

/// 清除源快照
fn clear_source_snapshot(conn: &rusqlite::Connection, job_id: i64) {
    let _ = conn.execute("DELETE FROM job_source_snapshot WHERE jobId=?", [job_id]);
    let _ = conn.execute("DELETE FROM job_source_snapshot_meta WHERE jobId=?", [job_id]);
}

// ==================== 排除规则 ====================

/// 将 gitignore 风格的模式转换为 glob 模式
/// gitignore 语义:
///   - `*.txt` 匹配任意层级的 .txt 文件
///   - `/root.txt` 仅匹配根目录
///   - `dir/` 匹配目录
///   - `!pattern` 取反（暂不支持，跳过）
fn gitignore_to_glob_patterns(raw: &str) -> Vec<String> {
    raw.split(':')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.starts_with('!'))
        .map(|p| {
            let p = p.trim_start_matches('/');
            if p.contains('/') {
                // 包含路径分隔符，保持原样
                p.to_string()
            } else {
                // 简单文件名模式，匹配任意层级
                format!("**/{}", p)
            }
        })
        .collect()
}

/// 编译 glob 模式列表
fn compile_patterns(patterns: &[String]) -> Vec<glob::Pattern> {
    patterns.iter().filter_map(|p| glob::Pattern::new(p).ok()).collect()
}

/// 检查路径是否匹配排除规则
fn is_excluded(rel_path: &str, name: &str, patterns: &[glob::Pattern]) -> bool {
    patterns.iter().any(|p| p.matches(rel_path) || p.matches(name))
}

// ==================== 核心同步逻辑 ====================

/// 执行作业同步的核心逻辑
pub async fn run_sync_for_job(job_id: i64) -> anyhow::Result<()> {
    tracing::info!("执行作业 {} 同步逻辑", job_id);

    let state = crate::state::get_global_state();

    // 从数据库加载作业配置
    let (src_path, dst_path_str, method, source_mode, exclude, min_size, max_size) = {
        let db = state.db.get().unwrap();
        db.query_row(
            "SELECT srcPath, dstPath, method, sourceMode, exclude, minFileSize, maxFileSize FROM job WHERE id=?",
            [job_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                ))
            },
        )?
    }; // MutexGuard dropped

    let dst_roots: Vec<String> = dst_path_str.split(':').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let has_file_size_filter = min_size.is_some() || max_size.is_some();

    // 创建任务记录
    let task_id = {
        let db = state.db.get().unwrap();
        let ts = crate::service::db::now_ts();
        db.execute(
            "INSERT INTO job_task (jobId, status, runTime) VALUES (?, 1, ?)",
            rusqlite::params![job_id, ts],
        )?;
        db.last_insert_rowid()
    };

    tracing::info!("作业 {}: src={}, dst={}, method={}, sourceMode={}, taskId={}",
        job_id, src_path, dst_path_str, method, source_mode, task_id);

    // 解析排除规则 — gitignore 风格
    let exclude_str = exclude.unwrap_or_default();
    let pattern_strs = gitignore_to_glob_patterns(&exclude_str);
    let compiled_patterns = compile_patterns(&pattern_strs);

    // 扫描源目录
    tracing::info!("扫描源目录: {}", src_path);
    let src_path_clone = src_path.clone();
    let compiled_clone = compiled_patterns.clone();
    let src_entries = tokio::task::spawn_blocking(move || {
        scan_local_directory(&src_path_clone, &compiled_clone, min_size, max_size)
    }).await??;

    tracing::info!("作业 {}: 源文件 {} 个", job_id, src_entries.len());

    // 根据 sourceMode 决定同步策略
    let operations = if source_mode == 1 {
        // 快照模式：与上次快照对比，仅同步变更文件
        sync_with_snapshot(&state, job_id, &src_entries, method, &compiled_patterns, min_size, max_size)?
    } else {
        // 实时对比模式：扫描目标目录，逐文件对比
        sync_live_compare(&state, &src_path, &dst_path_str, &src_entries, &compiled_patterns, method, min_size, max_size)?
    };

    tracing::info!("作业 {}: 需要执行 {} 个操作", job_id, operations.len());

    if operations.is_empty() {
        tracing::info!("作业 {}: 无需同步", job_id);
        let db = state.db.get().unwrap();
        let _ = db.execute(
            "UPDATE job_task SET status=2, taskNum=? WHERE id=?",
            rusqlite::params![serde_json::json!({"total": 0, "success": 0}).to_string(), task_id],
        );
        send_task_notification(&state, job_id, task_id, 0, 0, 0).await;
        return Ok(());
    }

    // 快照模式：先创建缺失的目录（在操作执行前）
    let failed_directory_prefixes: Vec<String> = if source_mode == 1 {
        create_snapshot_directories(&state, &src_entries, &dst_roots, task_id).await
    } else {
        vec![]
    };

    // 执行操作 — 对所有目标路径执行
    let total = operations.len() * dst_roots.len();
    let mut success = 0;
    let mut failed = 0;
    let mut move_src_files: Vec<(String, i64)> = vec![]; // 记录移动模式下需要删除的源文件

    for dst_root in &dst_roots {
        for op in &operations {
            if is_aborted(job_id) {
                tracing::info!("作业 {} 被中止", job_id);
                break;
            }

            let (src_rel, dst_rel, file_name, file_size, op_type) = match op {
                SyncOperation::Copy { src, dst, size } => {
                    let name = std::path::Path::new(src).file_name()
                        .map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    (src.clone(), dst.clone(), name, *size, 0i32)
                }
                SyncOperation::Move { src, dst, size } => {
                    let name = std::path::Path::new(src).file_name()
                        .map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    (src.clone(), dst.clone(), name, *size, 2i32)
                }
                SyncOperation::Delete { path, is_dir } => {
                    let name = std::path::Path::new(path).file_name()
                        .map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    // 全同步模式 + 有文件大小过滤 + 目标独有目录：递归删除目录内符合条件的文件
                    if *is_dir && method == 1 && has_file_size_filter {
                        delete_target_only_dir(dst_root, path, &compiled_patterns, min_size, max_size, task_id, &state);
                        // 跳过后续的 Delete 条目处理（已在 delete_target_only_dir 中处理）
                        continue;
                    }
                    (String::new(), path.clone(), name, 0i64, 1i32)
                }
            };

            // 快照模式：跳过创建失败的目录内的文件
            if source_mode == 1 {
                if let SyncOperation::Copy { src, .. } | SyncOperation::Move { src, .. } = op {
                    if failed_directory_prefixes.iter().any(|prefix| path_within(src, prefix)) {
                        continue;
                    }
                }
            }

            let item_id = {
                let db = state.db.get().unwrap();
                let ts = crate::service::db::now_ts();
                db.execute(
                    "INSERT INTO job_task_item (taskId, srcPath, dstPath, isPath, fileName, fileSize, type, status, createTime)
                     VALUES (?, ?, ?, 0, ?, ?, ?, 0, ?)",
                    rusqlite::params![task_id, src_rel, dst_rel, file_name, file_size, op_type, ts],
                ).ok();
                db.last_insert_rowid()
            };

            // Delete 操作只对第一个目标路径执行（避免重复删除）
            if matches!(op, SyncOperation::Delete { .. }) && dst_root != &dst_roots[0] {
                let db = state.db.get().unwrap();
                let _ = db.execute("UPDATE job_task_item SET status=2 WHERE id=?", [item_id]);
                success += 1;
                continue;
            }

            // Move 操作：先复制到目标，暂不删除源文件（finalize 阶段统一删除）
            if matches!(op, SyncOperation::Move { .. }) {
                match execute_copy(&src_rel, &src_path, dst_root).await {
                    Ok(_) => {
                        success += 1;
                        let db = state.db.get().unwrap();
                        let _ = db.execute("UPDATE job_task_item SET status=2 WHERE id=?", [item_id]);
                        // 记录需要在 finalize 阶段删除的源文件
                        if dst_root == &dst_roots[0] {
                            move_src_files.push((src_rel.clone(), file_size));
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        let err_msg = format!("{}", e);
                        tracing::error!("作业 {} 移动(复制阶段)失败: {:?}, 错误: {}", job_id, op, e);
                        let db = state.db.get().unwrap();
                        let _ = db.execute(
                            "UPDATE job_task_item SET status=7, errMsg=? WHERE id=?",
                            rusqlite::params![err_msg, item_id],
                        );
                    }
                }
            } else {
                match execute_operation(op, &src_path, dst_root).await {
                    Ok(_) => {
                        success += 1;
                        let db = state.db.get().unwrap();
                        let _ = db.execute("UPDATE job_task_item SET status=2 WHERE id=?", [item_id]);
                    }
                    Err(e) => {
                        failed += 1;
                        let err_msg = format!("{}", e);
                        tracing::error!("作业 {} 操作失败: {:?}, 错误: {}", job_id, op, e);
                        let db = state.db.get().unwrap();
                        let _ = db.execute(
                            "UPDATE job_task_item SET status=7, errMsg=? WHERE id=?",
                            rusqlite::params![err_msg, item_id],
                        );
                    }
                }
            }
        }
        if is_aborted(job_id) { break; }
    }

    // Move 模式 finalization：所有目标复制成功后，验证并删除源文件
    if method == 2 && !is_aborted(job_id) && failed == 0 && !move_src_files.is_empty() {
        finalize_move(&state, &src_path, &move_src_files, task_id).await;
    }

    // 快照模式：同步成功后保存快照
    if source_mode == 1 && !is_aborted(job_id) && failed == 0 {
        let db = state.db.get().unwrap();
        if let Err(e) = save_source_snapshot(&db, job_id, &src_entries) {
            tracing::error!("作业 {} 保存快照失败: {}", job_id, e);
        }
    }

    // 更新任务统计
    let task_status = if failed == 0 { 2 } else if success > 0 { 3 } else { 6 };
    {
        let db = state.db.get().unwrap();
        let _ = db.execute(
            "UPDATE job_task SET status=?, taskNum=? WHERE id=?",
            rusqlite::params![
                task_status,
                serde_json::json!({"total": total, "success": success, "failed": failed}).to_string(),
                task_id
            ],
        );
    }

    clear_abort_flag(job_id);

    // 发送任务完成通知
    send_task_notification(&state, job_id, task_id, total, success, failed).await;

    tracing::info!("作业 {} 完成: 成功 {}/{}, 失败 {}", job_id, success, total, failed);
    Ok(())
}

/// 快照模式同步：与上次快照对比，仅同步变更文件
fn sync_with_snapshot(
    state: &crate::state::SharedState,
    job_id: i64,
    src_entries: &[FileEntry],
    method: i32,
    compiled_patterns: &[glob::Pattern],
    _min_size: Option<i64>,
    _max_size: Option<i64>,
) -> anyhow::Result<Vec<SyncOperation>> {
    let db = state.db.get().unwrap();
    let previous_snapshot = get_source_snapshot(&db, job_id);
    drop(db);

    match previous_snapshot {
        Some(prev) => {
            tracing::info!("作业 {}: 快照模式，上次快照有 {} 个条目", job_id, prev.len());
            let ops = compare_with_snapshot(src_entries, &prev, method);
            tracing::info!("作业 {}: 快照对比发现 {} 个变更", job_id, ops.len());
            Ok(ops)
        }
        None => {
            // 首次运行，无快照，回退到实时对比模式
            tracing::info!("作业 {}: 首次运行快照模式，执行完整对比", job_id);
            Ok(compare_entries_live(src_entries, &[], method, compiled_patterns))
        }
    }
}

/// 实时对比模式：扫描目标目录并逐文件对比
fn sync_live_compare(
    _state: &crate::state::SharedState,
    _src_path: &str,
    dst_path_str: &str,
    src_entries: &[FileEntry],
    compiled_patterns: &[glob::Pattern],
    method: i32,
    min_size: Option<i64>,
    max_size: Option<i64>,
) -> anyhow::Result<Vec<SyncOperation>> {
    let dst_path_clone = dst_path_str.to_string();
    let dst_entries: Vec<FileEntry> = {
        let mut entries = vec![];
        for dst in dst_path_clone.split(':') {
            let dst = dst.trim();
            if dst.is_empty() { continue; }
            if let Ok(e) = scan_local_directory(dst, compiled_patterns, min_size, max_size) {
                entries.extend(e);
            }
        }
        entries
    };

    tracing::info!("实时对比: 源文件 {} 个, 目标文件 {} 个", src_entries.len(), dst_entries.len());
    Ok(compare_entries_live(src_entries, &dst_entries, method, compiled_patterns))
}

/// 与快照对比，生成变更操作
fn compare_with_snapshot(
    current: &[FileEntry],
    previous: &HashMap<String, SnapshotEntry>,
    method: i32,
) -> Vec<SyncOperation> {
    let mut ops = vec![];
    let current_index: HashMap<&str, &FileEntry> = current.iter().map(|e| (e.path.as_str(), e)).collect();
    let is_move_mode = method == 2;

    // 新增和修改的文件
    for entry in current {
        if entry.is_dir {
            continue;
        }
        if let Some(prev) = previous.get(&entry.path) {
            // 检查是否变更：大小不同或指纹不同
            let current_fp = compute_fingerprint(entry);
            let changed = prev.size != Some(entry.size)
                || prev.fingerprint.as_deref() != Some(&current_fp);
            if changed {
                if is_move_mode {
                    ops.push(SyncOperation::Move {
                        src: entry.path.clone(),
                        dst: entry.path.clone(),
                        size: entry.size,
                    });
                } else {
                    ops.push(SyncOperation::Copy {
                        src: entry.path.clone(),
                        dst: entry.path.clone(),
                        size: entry.size,
                    });
                }
            }
        } else {
            // 新文件
            if is_move_mode {
                ops.push(SyncOperation::Move {
                    src: entry.path.clone(),
                    dst: entry.path.clone(),
                    size: entry.size,
                });
            } else {
                ops.push(SyncOperation::Copy {
                    src: entry.path.clone(),
                    dst: entry.path.clone(),
                    size: entry.size,
                });
            }
        }
    }

    // 全同步模式：删除快照中有但当前源中已删除的文件
    if method == 1 {
        for (path, prev_entry) in previous {
            if !current_index.contains_key(path.as_str()) {
                ops.push(SyncOperation::Delete {
                    path: path.clone(),
                    is_dir: prev_entry.is_dir,
                });
            }
        }
    }

    ops
}

/// 实时对比源和目标，生成同步操作
fn compare_entries_live(
    src: &[FileEntry],
    dst: &[FileEntry],
    method: i32,
    _compiled_patterns: &[glob::Pattern],
) -> Vec<SyncOperation> {
    let mut ops = vec![];
    let dst_index: HashMap<&str, &FileEntry> = dst.iter().map(|e| (e.path.as_str(), e)).collect();
    let is_move_mode = method == 2;

    for src_entry in src {
        if let Some(dst_entry) = dst_index.get(src_entry.path.as_str()) {
            if src_entry.is_dir {
                continue;
            }
            // 文件已存在，比较大小和修改时间
            if src_entry.size != dst_entry.size || src_entry.modified != dst_entry.modified {
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
        let src_index: HashSet<&str> = src.iter().map(|e| e.path.as_str()).collect();
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

/// Move 模式 finalization：验证源文件未变更后删除
async fn finalize_move(
    state: &crate::state::SharedState,
    src_path: &str,
    move_src_files: &[(String, i64)],
    task_id: i64,
) {
    use std::path::Path;
    tracing::info!("Move 模式 finalization: 验证并删除 {} 个源文件", move_src_files.len());

    for (src_rel, expected_size) in move_src_files {
        let src_full = Path::new(src_path).join(src_rel);
        if !src_full.exists() {
            // 源文件已不存在（可能已被 rename 移动），跳过
            tracing::debug!("Move finalization: 源文件已不存在: {}", src_full.display());
            continue;
        }

        // 验证源文件未变更
        let current_size = match std::fs::metadata(&src_full) {
            Ok(meta) => meta.len() as i64,
            Err(e) => {
                tracing::error!("Move finalization: 无法获取源文件元数据 {}: {}", src_full.display(), e);
                continue;
            }
        };

        if current_size != *expected_size {
            tracing::warn!(
                "Move finalization: 源文件大小已变更 {} (期望 {}, 实际 {}), 跳过删除",
                src_full.display(), expected_size, current_size
            );
            // 标记对应的 task_item 为失败
            let db = state.db.get().unwrap();
            let _ = db.execute(
                "UPDATE job_task_item SET status=7, errMsg=? WHERE taskId=? AND srcPath=? AND type=2",
                rusqlite::params![format!("源文件在移动过程中已变更（期望大小 {}, 实际大小 {}）", expected_size, current_size), task_id, src_rel],
            );
            continue;
        }

        // 删除源文件
        match tokio::fs::remove_file(&src_full).await {
            Ok(_) => {
                tracing::debug!("Move finalization: 已删除源文件: {}", src_full.display());
            }
            Err(e) => {
                tracing::error!("Move finalization: 删除源文件失败 {}: {}", src_full.display(), e);
            }
        }
    }
}

// ==================== 快照模式目录管理 ====================

/// 快照模式：为目标路径创建缺失的目录
async fn create_snapshot_directories(
    state: &crate::state::SharedState,
    src_entries: &[FileEntry],
    dst_roots: &[String],
    task_id: i64,
) -> Vec<String> {
    use std::path::Path;
    let mut failed_prefixes = vec![];

    // 收集所有源目录
    let src_dirs: Vec<&FileEntry> = src_entries.iter().filter(|e| e.is_dir).collect();
    if src_dirs.is_empty() {
        return failed_prefixes;
    }

    for dst_root in dst_roots {
        for dir_entry in &src_dirs {
            let dst_dir = Path::new(dst_root).join(&dir_entry.path);
            let ts = crate::service::db::now_ts();

            match tokio::fs::create_dir_all(&dst_dir).await {
                Ok(_) => {
                    let db = state.db.get().unwrap();
                    let _ = db.execute(
                        "INSERT INTO job_task_item (taskId, srcPath, dstPath, isPath, fileName, fileSize, type, status, createTime)
                         VALUES (?, ?, ?, 1, NULL, NULL, 0, 2, ?)",
                        rusqlite::params![task_id, dir_entry.path, dir_entry.path, ts],
                    );
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    tracing::error!("创建目录失败 {}: {}", dst_dir.display(), err_msg);
                    failed_prefixes.push(dir_entry.path.clone());
                    let db = state.db.get().unwrap();
                    let _ = db.execute(
                        "INSERT INTO job_task_item (taskId, srcPath, dstPath, isPath, fileName, fileSize, type, status, createTime, errMsg)
                         VALUES (?, ?, ?, 1, NULL, NULL, 0, ?, ?, ?)",
                        rusqlite::params![task_id, dir_entry.path, dir_entry.path, 7, ts, err_msg],
                    );
                }
            }
        }
    }

    failed_prefixes
}

/// 全同步模式 + 文件大小过滤：递归删除目标独有目录中符合条件的文件
fn delete_target_only_dir(
    dst_root: &str,
    dir_path: &str,
    patterns: &[glob::Pattern],
    min_size: Option<i64>,
    max_size: Option<i64>,
    task_id: i64,
    state: &crate::state::SharedState,
) {
    use std::path::Path;
    let full_dir = Path::new(dst_root).join(dir_path);
    if !full_dir.is_dir() {
        return;
    }

    let ts = crate::service::db::now_ts();
    let mut to_delete = vec![];

    if let Ok(entries) = std::fs::read_dir(&full_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let rel_path = format!("{}/{}", dir_path.trim_end_matches('/'), name);

            if is_excluded(&rel_path, &name, patterns) {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    delete_target_only_dir(dst_root, &rel_path, patterns, min_size, max_size, task_id, state);
                } else {
                    let size = meta.len() as i64;
                    let size_allowed = min_size.map_or(true, |min| size >= min)
                        && max_size.map_or(true, |max| size <= max);
                    if size_allowed {
                        to_delete.push((name, size));
                    }
                }
            }
        }
    }

    for (name, size) in to_delete {
        let full_path = full_dir.join(&name);
        let status = if std::fs::remove_file(&full_path).is_ok() { 2 } else { 7 };
        let db = state.db.get().unwrap();
        let _ = db.execute(
            "INSERT INTO job_task_item (taskId, srcPath, dstPath, isPath, fileName, fileSize, type, status, createTime)
             VALUES (?, NULL, ?, 0, ?, ?, 1, ?, ?)",
            rusqlite::params![task_id, dir_path, name, size, status, ts],
        );
    }
}

/// 检查路径是否在指定前缀下
fn path_within(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{}/", prefix))
}

// ==================== 通知 ====================

/// 发送任务完成通知
async fn send_task_notification(
    state: &crate::state::SharedState,
    job_id: i64,
    _task_id: i64,
    total: usize,
    success: usize,
    failed: usize,
) {
    let (src_path, dst_path, remark) = {
        let db = state.db.get().unwrap();
        db.query_row(
            "SELECT srcPath, dstPath, remark FROM job WHERE id=?",
            [job_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?)),
        ).unwrap_or_default()
    };

    let notify_list: Vec<(i32, String)> = {
        let db = state.db.get().unwrap();
        let mut stmt = match db.prepare("SELECT method, params FROM notify WHERE enable=1") {
            Ok(s) => s,
            Err(_) => return,
        };
        stmt.query_map([], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?)))
            .map(|iter| iter.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    };

    if notify_list.is_empty() { return; }

    let dst_display = dst_path.replace(':', "\u{3001}");
    let status_name = if failed == 0 && total == 0 {
        "无需同步"
    } else if failed == 0 {
        "成功"
    } else if success > 0 {
        "部分失败"
    } else {
        "失败"
    };

    let title = if let Some(ref r) = remark {
        if r.is_empty() { format!("RustSync: {}", status_name) }
        else { format!("{}: {}", r, status_name) }
    } else {
        format!("RustSync: {}", status_name)
    };

    let content = format!(
        "来源: {}\n目标: {}\n总数: {}  成功: {}  失败: {}",
        src_path, dst_display, total, success, failed
    );

    let need_not_sync = failed == 0 && total == 0;

    for (method, params) in notify_list {
        if need_not_sync {
            if let Ok(params_json) = serde_json::from_str::<serde_json::Value>(&params) {
                if params_json.get("notSendNull").and_then(|v| v.as_bool()).unwrap_or(false) {
                    continue;
                }
            }
        }
        let title_clone = title.clone();
        let content_clone = content.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::api::notify::send_notification(method, &params, &title_clone, &content_clone).await {
                tracing::error!("发送通知失败 (method={}, job={}): {}", method, job_id, e);
            }
        });
    }
}

// ==================== 目录扫描 ====================

/// 扫描本地目录
fn scan_local_directory(
    path: &str,
    exclude_patterns: &[glob::Pattern],
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
    exclude_patterns: &[glob::Pattern],
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
        if is_excluded(&rel_path, &name, exclude_patterns) {
            continue;
        }

        let meta = entry.metadata()?;
        let is_dir = meta.is_dir();
        let size = meta.len() as i64;

        // 文件大小过滤
        if !is_dir {
            if let Some(min) = min_size {
                if size < min { continue; }
            }
            if let Some(max) = max_size {
                if size > max { continue; }
            }
        }

        let modified = meta.modified().ok().map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        let entry_data = FileEntry {
            path: rel_path.clone(),
            name,
            is_dir,
            size,
            modified,
            fingerprint: if is_dir { None } else { Some(compute_fingerprint_raw(size, false, modified)) },
        };

        entries.push(entry_data);

        if is_dir {
            scan_dir_recursive(root, &path, entries, exclude_patterns, min_size, max_size)?;
        }
    }
    Ok(())
}

fn compute_fingerprint_raw(size: i64, is_dir: bool, modified: Option<i64>) -> String {
    format!("{}:{}:{}", size, is_dir, modified.unwrap_or(0))
}

// ==================== 操作执行 ====================

/// 执行复制操作（用于 Move 模式的复制阶段）
async fn execute_copy(src_rel: &str, src_root: &str, dst_root: &str) -> anyhow::Result<()> {
    use std::path::Path;
    let src_full = Path::new(src_root).join(src_rel);
    let dst_full = Path::new(dst_root).join(src_rel);

    if src_full.is_dir() {
        tokio::fs::create_dir_all(&dst_full).await?;
    } else {
        if let Some(parent) = dst_full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(&src_full, &dst_full).await?;
    }
    Ok(())
}

/// 执行单个同步操作
async fn execute_operation(
    op: &SyncOperation,
    src_root: &str,
    dst_root: &str,
) -> anyhow::Result<()> {
    use std::path::Path;
    match op {
        SyncOperation::Copy { src, dst: _, size: _ } => {
            execute_copy(src, src_root, dst_root).await
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
            Ok(())
        }
        SyncOperation::Move { src, dst, size: _ } => {
            // Move 操作在 execute_operation 中作为 fallback 处理
            // 正常流程中 Move 操作已在 run_sync_for_job 中特殊处理
            let src_full = Path::new(src_root).join(src);
            let dst_full = Path::new(dst_root).join(dst);
            if let Some(parent) = dst_full.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            if tokio::fs::rename(&src_full, &dst_full).await.is_err() {
                tokio::fs::copy(&src_full, &dst_full).await?;
                tokio::fs::remove_file(&src_full).await?;
            }
            tracing::debug!("移动: {} -> {}", src_full.display(), dst_full.display());
            Ok(())
        }
    }
}