use axum::{
    extract::State,
    Json,
};
use crate::data::models::{Job, JobTask, JobTaskItem};
use crate::data::response::ApiResponse;
use crate::service::db::now_ts;
use crate::service::i18n;

/// 路径重叠检测 - 与 Python virtualPathsOverlap 一致
fn paths_overlap(first: &str, second: &str) -> bool {
    fn normalize(s: &str) -> String {
        let v = s.replace('\\', "/");
        let v = format!("/{}", v.trim_start_matches('/'));
        v.split('/').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("/")
    }
    let a = normalize(first);
    let b = normalize(second);
    a == b || a.starts_with(&format!("{}/", b)) || b.starts_with(&format!("{}/", a))
}

/// 文件大小标准化 - 与 Python normalizeFileSize 一致
fn normalize_file_size(value: &serde_json::Value) -> Result<Option<i64>, String> {
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Bool(_) => Err("文件大小格式无效".to_string()),
        serde_json::Value::Number(n) => {
            n.as_i64().ok_or_else(|| "文件大小格式无效".to_string())
                .and_then(|v| if v < 0 { Err("文件大小格式无效".to_string()) } else { Ok(Some(v)) })
        }
        serde_json::Value::String(s) => {
            s.parse::<i64>().map(Some).map_err(|_| "文件大小格式无效".to_string())
                .and_then(|v| if v.map_or(false, |x| x < 0) { Err("文件大小格式无效".to_string()) } else { Ok(v) })
        }
        _ => Err("文件大小格式无效".to_string()),
    }
}

/// sourceMode 标准化 - 与 Python normalizeSourceMode 一致
fn normalize_source_mode(value: &serde_json::Value) -> Result<i32, String> {
    match value {
        serde_json::Value::Null => Ok(0),
        serde_json::Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            let v = n.as_i64().unwrap_or(0) as i32;
            if v == 0 || v == 1 { Ok(v) } else { Err("sourceMode 无效".to_string()) }
        }
        serde_json::Value::String(s) => {
            match s.as_str() {
                "0" => Ok(0),
                "1" => Ok(1),
                _ => Err("sourceMode 无效".to_string()),
            }
        }
        _ => Err("sourceMode 无效".to_string()),
    }
}

/// 作业输入标准化与校验 - 与 Python cleanJobInput 一致
fn validate_and_normalize_job(body: &mut serde_json::Value) -> Result<(), String> {
    // isCron==2 且 enable!=1 时，强制 enable=1
    let is_cron = body.get("isCron").and_then(|v| v.as_i64()).unwrap_or(0);
    let enable = body.get("enable").and_then(|v| v.as_bool()).unwrap_or(true);
    if is_cron == 2 && !enable {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("enable".to_string(), serde_json::Value::Bool(true));
        }
    }

    if let Some(obj) = body.as_object_mut() {
        // 字符串字段 trim 并空字符串转 null
        let str_fields = ["remark", "srcPath", "dstPath", "year", "month", "day", "week",
            "dayOfWeek", "hour", "minute", "second", "startDate", "endDate", "exclude"];
        for field in &str_fields {
            if let Some(serde_json::Value::String(s)) = obj.get(*field) {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    obj.insert(field.to_string(), serde_json::Value::Null);
                } else if trimmed != s {
                    obj.insert(field.to_string(), serde_json::Value::String(trimmed.to_string()));
                }
            }
        }

        // exclude 标准化：用冒号连接并去除多余空格
        if let Some(serde_json::Value::String(s)) = obj.get("exclude").cloned() {
            let normalized: String = s.split(':')
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(":");
            if normalized.is_empty() {
                obj.insert("exclude".to_string(), serde_json::Value::Null);
            } else {
                obj.insert("exclude".to_string(), serde_json::Value::String(normalized));
            }
        }

        // minFileSize / maxFileSize 标准化
        if let Some(v) = obj.get("minFileSize").cloned() {
            if v.is_null() { obj.insert("minFileSize".to_string(), serde_json::Value::Null); }
            else { obj.insert("minFileSize".to_string(), serde_json::Value::from(normalize_file_size(&v)?)); }
        } else {
            obj.insert("minFileSize".to_string(), serde_json::Value::Null);
        }
        if let Some(v) = obj.get("maxFileSize").cloned() {
            if v.is_null() { obj.insert("maxFileSize".to_string(), serde_json::Value::Null); }
            else { obj.insert("maxFileSize".to_string(), serde_json::Value::from(normalize_file_size(&v)?)); }
        } else {
            obj.insert("maxFileSize".to_string(), serde_json::Value::Null);
        }

        // sourceMode 标准化
        if let Some(v) = obj.get("sourceMode").cloned() {
            obj.insert("sourceMode".to_string(), serde_json::Value::from(normalize_source_mode(&v)?));
        } else {
            obj.insert("sourceMode".to_string(), serde_json::Value::from(0));
        }
    }

    // 校验：文件大小范围
    let min = body.get("minFileSize").and_then(|v| v.as_i64());
    let max = body.get("maxFileSize").and_then(|v| v.as_i64());
    if let (Some(min_val), Some(max_val)) = (min, max) {
        if min_val > max_val {
            return Err(i18n::t("file_size_range_invalid"));
        }
    }

    // 校验：路径重叠
    let src_path = body.get("srcPath").and_then(|v| v.as_str()).unwrap_or("");
    let dst_path = body.get("dstPath").and_then(|v| v.as_str()).unwrap_or("");
    if !src_path.is_empty() && !dst_path.is_empty() {
        for dst in dst_path.split(':') {
            if paths_overlap(src_path, dst) {
                return Err(i18n::t("source_target_overlap"));
            }
        }
    }

    Ok(())
}

/// 获取任务的 taskNum 统计 - 与 Python getCuTaskNum 一致
fn get_task_num_stats(db: &std::sync::MutexGuard<rusqlite::Connection>, task_id: i64) -> serde_json::Value {
    let wait_num: i64 = db.query_row(
        "SELECT count(id) FROM job_task_item WHERE status=0 AND taskId=?", [task_id], |row| row.get(0)
    ).unwrap_or(0);
    let running_num: i64 = db.query_row(
        "SELECT count(id) FROM job_task_item WHERE status=1 AND taskId=?", [task_id], |row| row.get(0)
    ).unwrap_or(0);
    let success_num: i64 = db.query_row(
        "SELECT count(id) FROM job_task_item WHERE status=2 AND taskId=?", [task_id], |row| row.get(0)
    ).unwrap_or(0);
    let fail_num: i64 = db.query_row(
        "SELECT count(id) FROM job_task_item WHERE status=7 AND taskId=?", [task_id], |row| row.get(0)
    ).unwrap_or(0);
    let other_num: i64 = db.query_row(
        "SELECT count(id) FROM job_task_item WHERE status NOT IN (0,1,2,7) AND taskId=?", [task_id], |row| row.get(0)
    ).unwrap_or(0);
    let all_num: i64 = db.query_row(
        "SELECT count(id) FROM job_task_item WHERE taskId=?", [task_id], |row| row.get(0)
    ).unwrap_or(0);
    serde_json::json!({
        "waitNum": wait_num,
        "runningNum": running_num,
        "successNum": success_num,
        "failNum": fail_num,
        "otherNum": other_num,
        "allNum": all_num,
    })
}

/// 获取进行中的任务子项列表
fn get_doing_task_items(db: &std::sync::MutexGuard<rusqlite::Connection>, task_id: i64) -> Vec<serde_json::Value> {
    let mut stmt = match db.prepare(
        "SELECT srcPath, dstPath, fileName, fileSize, type, status, progress, errMsg, createTime
         FROM job_task_item WHERE taskId=? AND status=1 ORDER BY createTime ASC"
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let items: Vec<serde_json::Value> = stmt.query_map([task_id], |row| {
        Ok(serde_json::json!({
            "srcPath": row.get::<_, Option<String>>(0)?,
            "dstPath": row.get::<_, Option<String>>(1)?,
            "fileName": row.get::<_, Option<String>>(2)?,
            "fileSize": row.get::<_, Option<i64>>(3)?,
            "type": row.get::<_, i32>(4)?,
            "status": row.get::<_, i32>(5)?,
            "progress": row.get::<_, Option<f64>>(6)?,
            "errMsg": row.get::<_, Option<String>>(7)?,
            "createTime": row.get::<_, i64>(8)?,
        }))
    }).map(|rows| rows.filter_map(|r| r.ok()).collect()).unwrap_or_else(|_| vec![]);
    items
}

/// GET /svr/job - 获取作业列表/任务详情/当前执行状态
/// 前端调用: jobGetJob(params), jobGetTask(params), jobGetTaskItem(params), jobGetTaskCurrent(data)
pub async fn job_get(
    State(state): State<crate::state::SharedState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<serde_json::Value>> {
    let db = state.db.lock().unwrap();

    // 如果有 current=1，获取当前作业执行状态
    // 与 Python getJobCurrent 一致：返回当前正在执行的任务实时状态
    if params.get("current").map(|s| s.as_str()) == Some("1") {
        let job_id: i64 = params.get("id").and_then(|s| s.parse().ok()).unwrap_or(0);
        let status_filter = params.get("status").and_then(|s| s.parse::<i32>().ok());

        // 查询当前正在执行的任务（status=1 进行中，或按指定状态）
        let task = if let Some(st) = status_filter {
            db.query_row(
                "SELECT id, jobId, status, errMsg, runTime, taskNum, createTime
                 FROM job_task WHERE jobId=? AND status=? ORDER BY id DESC LIMIT 1",
                rusqlite::params![job_id, st],
                |row| Ok(JobTask {
                    id: row.get(0)?, job_id: row.get(1)?, status: row.get(2)?,
                    err_msg: row.get(3)?, run_time: row.get(4)?, task_num: row.get(5)?, create_time: row.get(6)?,
                }),
            ).ok()
        } else {
            db.query_row(
                "SELECT id, jobId, status, errMsg, runTime, taskNum, createTime
                 FROM job_task WHERE jobId=? AND status=1 ORDER BY id DESC LIMIT 1",
                [job_id],
                |row| Ok(JobTask {
                    id: row.get(0)?, job_id: row.get(1)?, status: row.get(2)?,
                    err_msg: row.get(3)?, run_time: row.get(4)?, task_num: row.get(5)?, create_time: row.get(6)?,
                }),
            ).ok()
        };

        // 与 Python getCurrent 一致：返回任务详情含 doingTask、scanFinish、num、size
        if let Some(ref t) = task {
            let task_num = get_task_num_stats(&db, t.id);
            let doing_items = get_doing_task_items(&db, t.id);
            let duration = if let Some(rt) = t.run_time {
                (now_ts() - rt).max(0)
            } else {
                0
            };
            return Json(ApiResponse::ok(serde_json::json!({
                "id": t.id,
                "jobId": t.job_id,
                "status": t.status,
                "errMsg": t.err_msg,
                "runTime": t.run_time,
                "createTime": t.create_time,
                "scanFinish": t.status > 1,  // 已完成即扫描结束
                "doingTask": doing_items,
                "duration": duration,
                "num": task_num,
                "size": {
                    "wait": 0, "running": 0, "success": 0, "fail": 0, "other": 0
                },
            })));
        }
        return Json(ApiResponse::ok(serde_json::json!(task)));
    }

    // 如果有 taskId，获取任务子项列表
    if let Some(task_id_str) = params.get("taskId") {
        let task_id: i64 = task_id_str.parse().unwrap_or(0);
        let page_num: i32 = params.get("pageNum").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i32 = params.get("pageSize").and_then(|s| s.parse().ok()).unwrap_or(10);
        let offset = (page_num - 1) * page_size;

        let total: i64 = db
            .query_row("SELECT count(*) FROM job_task_item WHERE taskId=?", [task_id], |row| row.get(0))
            .unwrap_or(0);

        let mut stmt = match db.prepare(
            "SELECT id, taskId, srcPath, dstPath, isPath, fileName, fileSize, type,
                    alistTaskId, status, progress, errMsg, createTime
             FROM job_task_item WHERE taskId=? ORDER BY id DESC LIMIT ? OFFSET ?",
        ) {
            Ok(s) => s,
            Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };
        let items: Vec<JobTaskItem> = match stmt.query_map(rusqlite::params![task_id, page_size, offset], |row| {
            Ok(JobTaskItem {
                id: row.get(0)?, task_id: row.get(1)?, src_path: row.get(2)?,
                dst_path: row.get(3)?, is_path: row.get::<_, i32>(4)? != 0,
                file_name: row.get(5)?, file_size: row.get(6)?, item_type: row.get(7)?,
                alist_task_id: row.get(8)?, status: row.get(9)?, progress: row.get(10)?,
                err_msg: row.get(11)?, create_time: row.get(12)?,
            })
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };
        return Json(ApiResponse::ok(serde_json::json!({
            "dataList": items, "count": total
        })));
    }

    // 如果有 id，获取作业的任务列表
    // 与 Python getTaskList 一致：返回任务列表并附加 taskNum 统计
    if let Some(job_id_str) = params.get("id") {
        let job_id: i64 = job_id_str.parse().unwrap_or(0);
        let page_num: i32 = params.get("pageNum").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i32 = params.get("pageSize").and_then(|s| s.parse().ok()).unwrap_or(10);
        let offset = (page_num - 1) * page_size;

        let total: i64 = db
            .query_row("SELECT count(*) FROM job_task WHERE jobId=?", [job_id], |row| row.get(0))
            .unwrap_or(0);

        let mut stmt = match db.prepare(
            "SELECT id, jobId, status, errMsg, runTime, taskNum, createTime
             FROM job_task WHERE jobId=? ORDER BY id DESC LIMIT ? OFFSET ?",
        ) {
            Ok(s) => s,
            Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };
        let list: Vec<serde_json::Value> = match stmt.query_map(rusqlite::params![job_id, page_size, offset], |row| {
            let id: i64 = row.get(0)?;
            let job_id: i64 = row.get(1)?;
            let status: i32 = row.get(2)?;
            let err_msg: Option<String> = row.get(3)?;
            let run_time: Option<i64> = row.get(4)?;
            let task_num_json: Option<String> = row.get(5)?;
            let create_time: i64 = row.get(6)?;
            Ok((id, job_id, status, err_msg, run_time, task_num_json, create_time))
        }) {
            Ok(iter) => {
                iter.filter_map(|r| r.ok()).map(|(id, job_id, status, err_msg, run_time, task_num_json, create_time)| {
                    // 与 Python getTaskList 一致：解析 taskNum JSON 并展开到每个 item
                    let mut base = serde_json::json!({
                        "id": id,
                        "jobId": job_id,
                        "status": status,
                        "errMsg": err_msg,
                        "runTime": run_time,
                        "taskNum": task_num_json,
                        "createTime": create_time,
                    });
                    // 如果有 taskNum JSON，解析并展开
                    if let Some(ref json_str) = task_num_json {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                            if let Some(obj) = base.as_object_mut() {
                                if let Some(parsed_obj) = parsed.as_object() {
                                    for (k, v) in parsed_obj {
                                        obj.insert(k.clone(), v.clone());
                                    }
                                }
                            }
                        }
                    } else {
                        // 旧版本无 taskNum，实时计算
                        let stats = get_task_num_stats(&db, id);
                        if let Some(obj) = base.as_object_mut() {
                            if let Some(stats_obj) = stats.as_object() {
                                for (k, v) in stats_obj {
                                    obj.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                    base
                }).collect()
            }
            Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };

        return Json(ApiResponse::ok(serde_json::json!({
            "dataList": list, "count": total
        })));
    }

    // 默认：作业列表
    let page_num: i32 = params.get("pageNum").and_then(|s| s.parse().ok()).unwrap_or(1);
    let page_size: i32 = params.get("pageSize").and_then(|s| s.parse().ok()).unwrap_or(10);
    let offset = (page_num - 1) * page_size;

    let total: i64 = db
        .query_row("SELECT count(*) FROM job", [], |row| row.get(0))
        .unwrap_or(0);

    let mut stmt = match db.prepare(
        "SELECT id, enable, remark, srcPath, dstPath, alistId, useCacheT, scanIntervalT,
                useCacheS, scanIntervalS, method, sourceMode, interval, isCron,
                year, month, day, week, day_of_week, hour, minute, second,
                start_date, end_date, exclude, minFileSize, maxFileSize, createTime
         FROM job ORDER BY id DESC LIMIT ? OFFSET ?",
    ) {
        Ok(s) => s,
        Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
    };

    let rows = stmt.query_map([page_size, offset], |row| {
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
    });

    match rows {
        Ok(iter) => {
            let list: Vec<Job> = iter.filter_map(|r| r.ok()).collect();
            Json(ApiResponse::ok(serde_json::json!({
                "dataList": list, "count": total
            })))
        }
        Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))),
    }
}

/// POST /svr/job - 添加作业或编辑作业
/// 前端: jobPost(data) - 如果 body 中有 id 则是编辑，否则是新增
pub async fn job_post(
    State(state): State<crate::state::SharedState>,
    Json(mut body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    // 输入校验与标准化（与 Python cleanJobInput 一致）
    if let Err(e) = validate_and_normalize_job(&mut body) {
        return Json(ApiResponse::err(&e));
    }

    let job_id = body.get("id").and_then(|v| v.as_i64());

    if let Some(id) = job_id {
        // ========== 编辑作业 ==========
        let (old_enable, old_is_cron, old_alist_id, old_src_path, old_dst_path, old_method, old_exclude, old_min_fs, old_max_fs): (i32, i32, Option<i64>, String, String, i32, Option<String>, Option<i64>, Option<i64>) = {
            let db = state.db.lock().unwrap();
            db.query_row(
                "SELECT enable, isCron, alistId, srcPath, dstPath, method, exclude, minFileSize, maxFileSize FROM job WHERE id=?",
                [id],
                |row| Ok((
                    row.get::<_, i32>(0)?, row.get::<_, i32>(1)?,
                    row.get::<_, Option<i64>>(2)?, row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?, row.get::<_, i32>(5)?,
                    row.get::<_, Option<String>>(6)?, row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                )),
            ).unwrap_or((0, 0, None, String::new(), String::new(), 0, None, None, None))
        };

        if old_enable == 1 && old_is_cron != 2 {
            return Json(ApiResponse::err(&i18n::t("disable_then_edit")));
        }

        // 检查 SOURCE_SNAPSHOT_FIELDS 是否有变化（与 Python 一致）
        let new_alist_id = body.get("alistId").and_then(|v| v.as_i64());
        let new_src_path = body.get("srcPath").and_then(|v| v.as_str()).unwrap_or("");
        let new_dst_path = body.get("dstPath").and_then(|v| v.as_str()).unwrap_or("");
        let new_method = body.get("method").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let new_exclude = body.get("exclude").and_then(|v| v.as_str()).map(|s| s.to_string());
        let new_min_fs = body.get("minFileSize").and_then(|v| v.as_i64());
        let new_max_fs = body.get("maxFileSize").and_then(|v| v.as_i64());
        let clear_snapshot = old_alist_id != new_alist_id
            || old_src_path != new_src_path
            || old_dst_path != new_dst_path
            || old_method != new_method
            || old_exclude != new_exclude
            || old_min_fs != new_min_fs
            || old_max_fs != new_max_fs;

        let enable = body.get("enable").and_then(|v| v.as_bool()).unwrap_or(true);
        let remark = body.get("remark").and_then(|v| v.as_str()).map(|s| s.to_string());
        let src_path = new_src_path;
        let dst_path = new_dst_path;
        let alist_id = new_alist_id;
        let use_cache_t = body.get("useCacheT").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let scan_interval_t = body.get("scanIntervalT").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let use_cache_s = body.get("useCacheS").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let scan_interval_s = body.get("scanIntervalS").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let method = new_method;
        let source_mode = body.get("sourceMode").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let interval = body.get("interval").and_then(|v| v.as_i64()).map(|v| v as i32);
        let is_cron = body.get("isCron").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let year = body.get("year").and_then(|v| v.as_str()).map(|s| s.to_string());
        let month = body.get("month").and_then(|v| v.as_str()).map(|s| s.to_string());
        let day = body.get("day").and_then(|v| v.as_str()).map(|s| s.to_string());
        let week = body.get("week").and_then(|v| v.as_str()).map(|s| s.to_string());
        let day_of_week = body.get("dayOfWeek").and_then(|v| v.as_str()).map(|s| s.to_string());
        let hour = body.get("hour").and_then(|v| v.as_str()).map(|s| s.to_string());
        let minute = body.get("minute").and_then(|v| v.as_str()).map(|s| s.to_string());
        let second = body.get("second").and_then(|v| v.as_str()).map(|s| s.to_string());
        let start_date = body.get("startDate").and_then(|v| v.as_str()).map(|s| s.to_string());
        let end_date = body.get("endDate").and_then(|v| v.as_str()).map(|s| s.to_string());
        let exclude = new_exclude;
        let min_file_size = new_min_fs;
        let max_file_size = new_max_fs;

        // 停止旧调度器
        crate::service::scheduler::get_scheduler().stop_job(id).await;

        {
            let db = state.db.lock().unwrap();
            match db.execute(
                "UPDATE job SET enable=?, remark=?, srcPath=?, dstPath=?, alistId=?, useCacheT=?,
                 scanIntervalT=?, useCacheS=?, scanIntervalS=?, method=?, sourceMode=?, interval=?,
                 isCron=?, year=?, month=?, day=?, week=?, day_of_week=?, hour=?, minute=?, second=?,
                 start_date=?, end_date=?, exclude=?, minFileSize=?, maxFileSize=?
                 WHERE id=?",
                rusqlite::params![
                    enable as i32, remark, src_path, dst_path, alist_id,
                    use_cache_t, scan_interval_t, use_cache_s, scan_interval_s,
                    method, source_mode, interval, is_cron,
                    year, month, day, week, day_of_week, hour, minute, second,
                    start_date, end_date, exclude, min_file_size, max_file_size, id,
                ],
            ) {
                Ok(_) => {
                    // 与 Python 一致：当 SOURCE_SNAPSHOT_FIELDS 变化时清除快照
                    if clear_snapshot {
                        let _ = db.execute("DELETE FROM job_source_snapshot WHERE jobId=?", [id]);
                        let _ = db.execute("DELETE FROM job_source_snapshot_meta WHERE jobId=?", [id]);
                    }
                }
                Err(e) => return Json(ApiResponse::err(&format!("更新失败: {}", e))),
            }
        }

        // 如果启用且非手动模式，启动新调度器
        if enable && is_cron != 2 {
            let new_job = {
                let db = state.db.lock().unwrap();
                db.query_row(
                    "SELECT id, enable, remark, srcPath, dstPath, alistId, useCacheT, scanIntervalT,
                            useCacheS, scanIntervalS, method, sourceMode, interval, isCron,
                            year, month, day, week, day_of_week, hour, minute, second,
                            start_date, end_date, exclude, minFileSize, maxFileSize, createTime
                     FROM job WHERE id=?",
                    [id],
                    |row| Ok(Job {
                        id: row.get(0)?, enable: row.get::<_, i32>(1)? != 0,
                        remark: row.get(2)?, src_path: row.get(3)?, dst_path: row.get(4)?,
                        alist_id: row.get(5)?, use_cache_t: row.get::<_, i32>(6)? != 0,
                        scan_interval_t: row.get(7)?, use_cache_s: row.get::<_, i32>(8)? != 0,
                        scan_interval_s: row.get(9)?, method: row.get(10)?,
                        source_mode: row.get::<_, i32>(11)? != 0, interval: row.get(12)?,
                        is_cron: row.get(13)?, year: row.get(14)?, month: row.get(15)?,
                        day: row.get(16)?, week: row.get(17)?, day_of_week: row.get(18)?,
                        hour: row.get(19)?, minute: row.get(20)?, second: row.get(21)?,
                        start_date: row.get(22)?, end_date: row.get(23)?, exclude: row.get(24)?,
                        min_file_size: row.get(25)?, max_file_size: row.get(26)?,
                        create_time: row.get(27)?,
                    }),
                ).ok()
            };
            if let Some(job) = new_job {
                crate::service::scheduler::get_scheduler().start_job(job).await;
            }
        }
        Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("job_updated")))
    } else {
        // ========== 新增作业 ==========
        let enable = body.get("enable").and_then(|v| v.as_bool()).unwrap_or(true);
        let is_cron = body.get("isCron").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let enable = if is_cron == 2 && !enable { true } else { enable };

        let remark = body.get("remark").and_then(|v| v.as_str()).map(|s| s.to_string());
        let src_path = body.get("srcPath").and_then(|v| v.as_str()).unwrap_or("");
        let dst_path = body.get("dstPath").and_then(|v| v.as_str()).unwrap_or("");
        let alist_id = body.get("alistId").and_then(|v| v.as_i64());
        let use_cache_t = body.get("useCacheT").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let scan_interval_t = body.get("scanIntervalT").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let use_cache_s = body.get("useCacheS").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let scan_interval_s = body.get("scanIntervalS").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let method = body.get("method").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let source_mode = body.get("sourceMode").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let interval = body.get("interval").and_then(|v| v.as_i64()).map(|v| v as i32);
        let year = body.get("year").and_then(|v| v.as_str()).map(|s| s.to_string());
        let month = body.get("month").and_then(|v| v.as_str()).map(|s| s.to_string());
        let day = body.get("day").and_then(|v| v.as_str()).map(|s| s.to_string());
        let week = body.get("week").and_then(|v| v.as_str()).map(|s| s.to_string());
        let day_of_week = body.get("dayOfWeek").and_then(|v| v.as_str()).map(|s| s.to_string());
        let hour = body.get("hour").and_then(|v| v.as_str()).map(|s| s.to_string());
        let minute = body.get("minute").and_then(|v| v.as_str()).map(|s| s.to_string());
        let second = body.get("second").and_then(|v| v.as_str()).map(|s| s.to_string());
        let start_date = body.get("startDate").and_then(|v| v.as_str()).map(|s| s.to_string());
        let end_date = body.get("endDate").and_then(|v| v.as_str()).map(|s| s.to_string());
        let exclude = body.get("exclude").and_then(|v| v.as_str()).map(|s| s.to_string());
        let min_file_size = body.get("minFileSize").and_then(|v| v.as_i64());
        let max_file_size = body.get("maxFileSize").and_then(|v| v.as_i64());

        let new_id = {
            let db = state.db.lock().unwrap();
            match db.execute(
                "INSERT INTO job (enable, remark, srcPath, dstPath, alistId, useCacheT, scanIntervalT,
                 useCacheS, scanIntervalS, method, sourceMode, interval, isCron,
                 year, month, day, week, day_of_week, hour, minute, second,
                 start_date, end_date, exclude, minFileSize, maxFileSize)
                 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    enable as i32, remark, src_path, dst_path, alist_id,
                    use_cache_t, scan_interval_t, use_cache_s, scan_interval_s,
                    method, source_mode, interval, is_cron,
                    year, month, day, week, day_of_week, hour, minute, second,
                    start_date, end_date, exclude, min_file_size, max_file_size,
                ],
            ) {
                Ok(_) => db.last_insert_rowid(),
                Err(e) => return Json(ApiResponse::err(&format!("添加失败: {}", e))),
            }
        };

        // 与 Python addJobClient 一致：新增后启动调度器
        if enable && is_cron != 2 {
            let new_job = {
                let db = state.db.lock().unwrap();
                db.query_row(
                    "SELECT id, enable, remark, srcPath, dstPath, alistId, useCacheT, scanIntervalT,
                            useCacheS, scanIntervalS, method, sourceMode, interval, isCron,
                            year, month, day, week, day_of_week, hour, minute, second,
                            start_date, end_date, exclude, minFileSize, maxFileSize, createTime
                     FROM job WHERE id=?",
                    [new_id],
                    |row| Ok(Job {
                        id: row.get(0)?, enable: row.get::<_, i32>(1)? != 0,
                        remark: row.get(2)?, src_path: row.get(3)?, dst_path: row.get(4)?,
                        alist_id: row.get(5)?, use_cache_t: row.get::<_, i32>(6)? != 0,
                        scan_interval_t: row.get(7)?, use_cache_s: row.get::<_, i32>(8)? != 0,
                        scan_interval_s: row.get(9)?, method: row.get(10)?,
                        source_mode: row.get::<_, i32>(11)? != 0, interval: row.get(12)?,
                        is_cron: row.get(13)?, year: row.get(14)?, month: row.get(15)?,
                        day: row.get(16)?, week: row.get(17)?, day_of_week: row.get(18)?,
                        hour: row.get(19)?, minute: row.get(20)?, second: row.get(21)?,
                        start_date: row.get(22)?, end_date: row.get(23)?, exclude: row.get(24)?,
                        min_file_size: row.get(25)?, max_file_size: row.get(26)?,
                        create_time: row.get(27)?,
                    }),
                ).ok()
            };
            if let Some(job) = new_job {
                crate::service::scheduler::get_scheduler().start_job(job).await;
            }
        }
        Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("job_added")))
    }
}

/// PUT /svr/job - 执行/暂停/启用/中止作业
/// 前端: jobPut(data) - body 中有 pause, abort, id 等参数
pub async fn job_put(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let pause = body.get("pause").and_then(|v| v.as_bool());

    match pause {
        Some(true) => {
            // 禁用/中止作业
            let job_id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let db = state.db.lock().unwrap();
            if body.get("abort").is_some() {
                // 中止作业 - 与 Python abortJob 一致
                let _ = db.execute(
                    "UPDATE job_task SET status=7 WHERE jobId=? AND status IN (0, 1)", [job_id],
                );
                Json(ApiResponse::ok_msg(serde_json::json!({}), "作业已中止"))
            } else {
                // 禁用作业 - 与 Python pauseJob 一致
                let is_cron: i32 = db.query_row("SELECT isCron FROM job WHERE id=?", [job_id], |row| row.get(0)).unwrap_or(0);
                if is_cron == 2 {
                    return Json(ApiResponse::err(&i18n::t("cannot_disable_manual_job")));
                }
                let _ = db.execute("UPDATE job SET enable=0 WHERE id=?", [job_id]);
                // 中止正在执行的任务
                let _ = db.execute("UPDATE job_task SET status=4 WHERE status IN (0, 1) AND jobId=?", [job_id]);
                drop(db);
                // 停止调度器
                crate::service::scheduler::get_scheduler().stop_job(job_id).await;
                Json(ApiResponse::ok_msg(serde_json::json!({}), "作业已禁用"))
            }
        }
        Some(false) => {
            // 启用作业 - 与 Python continueJob 一致
            let job_id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            {
                let db = state.db.lock().unwrap();
                let _ = db.execute("UPDATE job SET enable=1 WHERE id=?", [job_id]);
                // 启动调度器
                let job = db.query_row(
                    "SELECT id, enable, remark, srcPath, dstPath, alistId, useCacheT, scanIntervalT,
                            useCacheS, scanIntervalS, method, sourceMode, interval, isCron,
                            year, month, day, week, day_of_week, hour, minute, second,
                            start_date, end_date, exclude, minFileSize, maxFileSize, createTime
                     FROM job WHERE id=?",
                    [job_id],
                    |row| Ok(Job {
                        id: row.get(0)?, enable: row.get::<_, i32>(1)? != 0,
                        remark: row.get(2)?, src_path: row.get(3)?, dst_path: row.get(4)?,
                        alist_id: row.get(5)?, use_cache_t: row.get::<_, i32>(6)? != 0,
                        scan_interval_t: row.get(7)?, use_cache_s: row.get::<_, i32>(8)? != 0,
                        scan_interval_s: row.get(9)?, method: row.get(10)?,
                        source_mode: row.get::<_, i32>(11)? != 0, interval: row.get(12)?,
                        is_cron: row.get(13)?, year: row.get(14)?, month: row.get(15)?,
                        day: row.get(16)?, week: row.get(17)?, day_of_week: row.get(18)?,
                        hour: row.get(19)?, minute: row.get(20)?, second: row.get(21)?,
                        start_date: row.get(22)?, end_date: row.get(23)?, exclude: row.get(24)?,
                        min_file_size: row.get(25)?, max_file_size: row.get(26)?,
                        create_time: row.get(27)?,
                    }),
                ).ok();
                if let Some(job) = job {
                    if job.enable && job.is_cron != 2 {
                        crate::service::scheduler::get_scheduler().start_job(job).await;
                    }
                }
            }
            Json(ApiResponse::ok_msg(serde_json::json!({}), "作业已启用"))
        }
        None => {
            // 手动执行 - 与 Python doJobManual / doAllJobManual 一致
            if let Some(job_id) = body.get("id").and_then(|v| v.as_i64()) {
                // 执行单个作业
                let db = state.db.lock().unwrap();
                let enable: i32 = db.query_row("SELECT enable FROM job WHERE id=?", [job_id], |row| row.get(0)).unwrap_or(0);
                if enable != 1 {
                    return Json(ApiResponse::err(&i18n::t("disabled_job_cannot_run")));
                }
                // 检查是否已有正在执行的任务（与 Python runLock 一致）
                let running: i64 = db.query_row(
                    "SELECT count(*) FROM job_task WHERE jobId=? AND status IN (0, 1)", [job_id], |row| row.get(0)
                ).unwrap_or(0);
                if running > 0 {
                    return Json(ApiResponse::err(&i18n::t("job_running")));
                }
                drop(db);
                // 在后台异步执行同步（run_sync_for_job 内部会创建任务记录）
                tokio::spawn(async move {
                    if let Err(e) = crate::service::sync_engine::run_sync_for_job(job_id).await {
                        tracing::error!("手动执行作业 {} 失败: {}", job_id, e);
                    }
                });
                Json(ApiResponse::ok_msg(
                    serde_json::json!({}),
                    "作业已开始执行",
                ))
            } else {
                // 执行所有启用的作业
                let db = state.db.lock().unwrap();
                let mut stmt = match db.prepare("SELECT id FROM job WHERE enable=1") {
                    Ok(s) => s,
                    Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
                };
                let job_ids: Vec<i64> = match stmt.query_map([], |row| row.get(0)) {
                    Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
                    Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
                };
                if job_ids.is_empty() {
                    return Json(ApiResponse::err(&i18n::t("no_job_for_run")));
                }
                drop(stmt);
                drop(db);
                // 筛选可执行的作业（检查是否已有正在执行的任务）
                let db = state.db.lock().unwrap();
                let mut eligible_jobs = vec![];
                for job_id in &job_ids {
                    let running: i64 = db.query_row(
                        "SELECT count(*) FROM job_task WHERE jobId=? AND status IN (0, 1)", [job_id], |row| row.get(0)
                    ).unwrap_or(0);
                    if running == 0 {
                        eligible_jobs.push(*job_id);
                    }
                }
                drop(db);
                if eligible_jobs.is_empty() {
                    return Json(ApiResponse::err(&i18n::t("no_job_for_run")));
                }
                // 在后台异步执行所有作业的同步（run_sync_for_job 内部会创建任务记录）
                let task_count = eligible_jobs.len();
                tokio::spawn(async move {
                    for job_id in eligible_jobs {
                        if let Err(e) = crate::service::sync_engine::run_sync_for_job(job_id).await {
                            tracing::error!("批量执行作业 {} 失败: {}", job_id, e);
                        }
                    }
                });
                Json(ApiResponse::ok_msg(
                    serde_json::json!({"count": task_count}),
                    &format!("已启动 {} 个作业", task_count),
                ))
            }
        }
    }
}

/// DELETE /svr/job - 删除作业或任务
/// 前端: jobDelete(data) - body 中有 id 或 taskId
pub async fn job_delete(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    if let Some(task_id) = body.get("taskId").and_then(|v| v.as_i64()) {
        // 删除任务
        let db = state.db.lock().unwrap();
        let _ = db.execute("DELETE FROM job_task_item WHERE taskId=?", [task_id]);
        match db.execute("DELETE FROM job_task WHERE id=?", [task_id]) {
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), "任务已删除")),
            Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
        }
    } else if let Some(job_id) = body.get("id").and_then(|v| v.as_i64()) {
        // 删除作业 - 与 Python removeJobClient 一致：先停止调度器
        crate::service::scheduler::get_scheduler().stop_job(job_id).await;
        let db = state.db.lock().unwrap();
        let _ = db.execute("DELETE FROM job_source_snapshot WHERE jobId=?", [job_id]);
        let _ = db.execute("DELETE FROM job_source_snapshot_meta WHERE jobId=?", [job_id]);
        let _ = db.execute("DELETE FROM job_task_item WHERE taskId IN (SELECT id FROM job_task WHERE jobId=?)", [job_id]);
        let _ = db.execute("DELETE FROM job_task WHERE jobId=?", [job_id]);
        match db.execute("DELETE FROM job WHERE id=?", [job_id]) {
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("job_deleted"))),
            Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
        }
    } else {
        Json(ApiResponse::err("缺少作业ID或任务ID"))
    }
}