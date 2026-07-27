use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use crate::data::models::Job;

/// 调度器状态
pub struct Scheduler {
    /// 活跃的作业调度 (job_id -> join_handle)
    jobs: RwLock<HashMap<i64, tokio::task::JoinHandle<()>>>,
    /// 作业是否启用
    enabled: RwLock<HashMap<i64, bool>>,
}

pub type SharedScheduler = Arc<Scheduler>;

impl Scheduler {
    pub fn new() -> SharedScheduler {
        Arc::new(Self {
            jobs: RwLock::new(HashMap::new()),
            enabled: RwLock::new(HashMap::new()),
        })
    }

    /// 启动作业调度
    pub async fn start_job(self: &Arc<Self>, job: Job) {
        let scheduler = self.clone();
        let job_id = job.id;

        // 标记为启用
        self.enabled.write().await.insert(job_id, true);

        let handle = tokio::spawn(async move {
            scheduler.run_job_loop(job_id, job).await;
        });

        self.jobs.write().await.insert(job_id, handle);
    }

    /// 停止作业调度
    pub async fn stop_job(&self, job_id: i64) {
        self.enabled.write().await.insert(job_id, false);
        if let Some(handle) = self.jobs.write().await.remove(&job_id) {
            handle.abort();
        }
    }

    /// 作业调度循环
    async fn run_job_loop(&self, job_id: i64, job: Job) {
        loop {
            // 检查是否仍启用
            {
                let enabled = self.enabled.read().await;
                if !enabled.get(&job_id).copied().unwrap_or(false) {
                    break;
                }
            }

            // 每次循环从数据库重新读取作业配置，确保 cron 变更等能及时生效
            let current_job = {
                let state = crate::state::get_global_state();
                let db = state.db.get().unwrap();
                db.query_row(
                    "SELECT enable, isCron, start_date, interval,
                            year, month, day, week, day_of_week, hour, minute, second
                     FROM job WHERE id=?",
                    [job_id],
                    |row| {
                        Ok(Job {
                            id: job_id,
                            enable: row.get::<_, i32>(0)? != 0,
                            is_cron: row.get(1)?,
                            start_date: row.get(2)?,
                            interval: row.get(3)?,
                            year: row.get(4)?,
                            month: row.get(5)?,
                            day: row.get(6)?,
                            week: row.get(7)?,
                            day_of_week: row.get(8)?,
                            hour: row.get(9)?,
                            minute: row.get(10)?,
                            second: row.get(11)?,
                            ..Default::default()
                        })
                    },
                ).unwrap_or_else(|_| job.clone())
            }; // MutexGuard dropped here

            // 如果数据库中的 enable 变为 0，退出循环
            if !current_job.enable {
                self.enabled.write().await.insert(job_id, false);
                break;
            }

            // 如果变为手动模式，退出调度循环
            if current_job.is_cron == 2 {
                tracing::info!("作业 {} 变为手动模式，退出调度循环", job_id);
                break;
            }

            // 计算下次执行时间
            let delay = self.calculate_delay(&current_job);
            tracing::info!("作业 {} 将在 {} 秒后执行", job_id, delay);

            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;

            // 再次检查
            {
                let enabled = self.enabled.read().await;
                if !enabled.get(&job_id).copied().unwrap_or(false) {
                    break;
                }
            }

            tracing::info!("作业 {} 开始执行", job_id);
            // 执行同步逻辑
            if let Err(e) = crate::service::sync_engine::run_sync_for_job(job_id).await {
                tracing::error!("作业 {} 执行失败: {}", job_id, e);
            }

            // 如果是一次性 cron 任务，执行完退出
            if current_job.is_cron == 1 && current_job.start_date.is_some() {
                break;
            }
        }
    }

    /// 计算延迟时间（秒）
    fn calculate_delay(&self, job: &Job) -> u64 {
        if job.is_cron == 0 {
            // 间隔模式
            (job.interval.unwrap_or(60) as u64).max(1) * 60
        } else if job.is_cron == 1 {
            // Cron 模式 - 计算到下次触发的时间
            self.next_cron_delay(job)
        } else {
            // 仅手动，不自动调度
            3600
        }
    }

    fn next_cron_delay(&self, job: &Job) -> u64 {
        // 简化 cron 计算：使用 interval 作为回退
        // 完整 cron 实现需要 cron crate
        if let Some(ref expr) = job.minute {
            // 尝试解析 cron 表达式
            let cron_str = format!(
                "{} {} {} {} {} {}",
                job.second.as_deref().unwrap_or("0"),
                expr,
                job.hour.as_deref().unwrap_or("*"),
                job.day.as_deref().unwrap_or("*"),
                job.month.as_deref().unwrap_or("*"),
                job.day_of_week.as_deref().unwrap_or("*"),
            );
            if let Ok(schedule) = cron_str.parse::<cron::Schedule>() {
                use chrono::Utc;
                if let Some(next) = schedule.upcoming(Utc).next() {
                    let now = Utc::now();
                    let delay = (next - now).num_seconds().max(1);
                    return delay as u64;
                }
            }
        }
        // 回退到间隔
        (job.interval.unwrap_or(60) as u64).max(1) * 60
    }
}

/// 初始化所有启用的作业
pub async fn init_all_jobs(state: &crate::state::SharedState) -> anyhow::Result<()> {
    // 重启后，将运行中/等待中的任务标记为中止
    {
        let db = state.db.get().unwrap();
        let _ = db.execute("UPDATE job_task SET status=4 WHERE status IN (0, 1)", []);
        let _ = db.execute("UPDATE job_task_item SET status=4 WHERE status IN (0, 1)", []);
    }

    // 启动日志和任务历史清理
    start_cleanup_scheduler(state);

    let jobs: Vec<Job> = {
        let db = state.db.get().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, enable, remark, srcPath, dstPath, alistId, useCacheT, scanIntervalT,
                    useCacheS, scanIntervalS, method, sourceMode, interval, isCron,
                    year, month, day, week, day_of_week, hour, minute, second,
                    start_date, end_date, exclude, minFileSize, maxFileSize, createTime
             FROM job WHERE enable=1 AND isCron != 2"
        )?;

        let x = stmt.query_map([], |row| {
            Ok(Job {
                id: row.get(0)?,
                enable: row.get::<_, i32>(1)? != 0,
                remark: row.get(2)?,
                src_path: row.get(3)?,
                dst_path: row.get(4)?,
                alist_id: row.get(5)?,
                use_cache_t: row.get::<_, i32>(6)? != 0,
                scan_interval_t: row.get(7)?,
                use_cache_s: row.get::<_, i32>(8)? != 0,
                scan_interval_s: row.get(9)?,
                method: row.get(10)?,
                source_mode: row.get::<_, i32>(11)? != 0,
                interval: row.get(12)?,
                is_cron: row.get(13)?,
                year: row.get(14)?,
                month: row.get(15)?,
                day: row.get(16)?,
                week: row.get(17)?,
                day_of_week: row.get(18)?,
                hour: row.get(19)?,
                minute: row.get(20)?,
                second: row.get(21)?,
                start_date: row.get(22)?,
                end_date: row.get(23)?,
                exclude: row.get(24)?,
                min_file_size: row.get(25)?,
                max_file_size: row.get(26)?,
                create_time: row.get(27)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        x
    }; // MutexGuard dropped here, safe to .await below

    let scheduler = crate::service::scheduler::get_scheduler();
    for job in jobs {
        scheduler.start_job(job).await;
    }

    tracing::info!("已初始化 {} 个启用的作业", scheduler.jobs.read().await.len());
    Ok(())
}

static SCHEDULER: OnceLock<SharedScheduler> = OnceLock::new();

pub fn get_scheduler() -> SharedScheduler {
    SCHEDULER.get_or_init(|| Scheduler::new()).clone()
}

// ==================== 日志和任务历史清理 ====================

/// 启动日志和任务历史清理调度器
fn start_cleanup_scheduler(state: &crate::state::SharedState) {
    let log_save = state.config.log_save;
    let task_save = state.config.task_save;

    if log_save == 0 && task_save == 0 {
        return;
    }

    let state = state.clone();
    tokio::spawn(async move {
        // 立即执行一次清理
        run_cleanup(&state, log_save, task_save);

        // 每天午夜执行清理
        loop {
            let delay = seconds_until_midnight();
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            run_cleanup(&state, log_save, task_save);
        }
    });
}

fn run_cleanup(state: &crate::state::SharedState, log_save: u32, task_save: u32) {
    if log_save > 0 {
        cleanup_old_logs(state, log_save);
    }
    if task_save > 0 {
        cleanup_old_tasks(state, task_save);
    }
}

fn cleanup_old_logs(state: &crate::state::SharedState, log_save: u32) {
    let log_dir = std::path::Path::new(&state.config.log_dir);
    if !log_dir.exists() {
        return;
    }

    let cutoff = chrono::Utc::now().timestamp() - (log_save as i64 * 86400);

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".log") {
                continue;
            }
            // 检查文件修改时间
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                        if dur.as_secs() as i64 < cutoff {
                            let _ = std::fs::remove_file(&path);
                            tracing::info!("已清理过期日志: {}", name);
                        }
                    }
                }
            }
        }
    }
}

fn cleanup_old_tasks(state: &crate::state::SharedState, task_save: u32) {
    let cutoff = crate::service::db::now_ts() - (task_save as i64 * 86400);
    let db = state.db.get().unwrap();
    let _ = db.execute(
        "DELETE FROM job_task_item WHERE taskId IN (SELECT id FROM job_task WHERE runTime < ?)",
        [cutoff],
    );
    let deleted = db.execute("DELETE FROM job_task WHERE runTime < ?", [cutoff]).unwrap_or(0);
    if deleted > 0 {
        tracing::info!("已清理 {} 条过期任务记录", deleted);
    }
}

fn seconds_until_midnight() -> u64 {
    let now = chrono::Local::now();
    let midnight = (now + chrono::Duration::days(1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|naive| naive.and_local_timezone(chrono::Local).single())
        .unwrap_or(now + chrono::Duration::hours(1));
    let dur = midnight - now;
    dur.num_seconds().max(60) as u64
}