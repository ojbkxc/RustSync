use axum::{
    extract::State,
    Json,
};
use crate::data::models::{Job, JobTask, JobTaskItem};
use crate::data::response::ApiResponse;
use crate::service::db::now_ts;
use crate::service::i18n;

/// GET /svr/job - 获取作业列表/任务详情/当前执行状态
/// 前端调用: jobGetJob(params), jobGetTask(params), jobGetTaskItem(params), jobGetTaskCurrent(data)
pub async fn job_get(
    State(state): State<crate::state::SharedState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<serde_json::Value>> {
    let db = state.db.read().await;

    // 如果有 current=1，获取当前作业执行状态
    if params.get("current").map(|s| s.as_str()) == Some("1") {
        let job_id: i64 = params.get("id").and_then(|s| s.parse().ok()).unwrap_or(0);
        let status = params.get("status").and_then(|s| s.parse::<i32>().ok());
        let task = if let Some(st) = status {
            // 按状态过滤
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
        return Json(ApiResponse::ok(serde_json::json!(task)));
    }

    // 如果有 taskId，获取任务子项列表
    if let Some(task_id_str) = params.get("taskId") {
        let task_id: i64 = task_id_str.parse().unwrap_or(0);
        let mut stmt = match db.prepare(
            "SELECT id, taskId, srcPath, dstPath, isPath, fileName, fileSize, type,
                    alistTaskId, status, progress, errMsg, createTime
             FROM job_task_item WHERE taskId=? ORDER BY id",
        ) {
            Ok(s) => s,
            Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };
        let items: Vec<JobTaskItem> = match stmt.query_map([task_id], |row| {
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
        return Json(ApiResponse::ok(serde_json::json!(items)));
    }

    // 如果有 id，获取作业的任务列表
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
        let list: Vec<JobTask> = match stmt.query_map(rusqlite::params![job_id, page_size, offset], |row| {
            Ok(JobTask {
                id: row.get(0)?, job_id: row.get(1)?, status: row.get(2)?,
                err_msg: row.get(3)?, run_time: row.get(4)?, task_num: row.get(5)?, create_time: row.get(6)?,
            })
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };

        return Json(ApiResponse::ok(serde_json::json!({
            "list": list, "total": total, "pageNum": page_num, "pageSize": page_size
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
                "list": list, "total": total, "pageNum": page_num, "pageSize": page_size
            })))
        }
        Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))),
    }
}

/// POST /svr/job - 添加作业或编辑作业
/// 前端: jobPost(data) - 如果 body 中有 id 则是编辑，否则是新增
pub async fn job_post(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let job_id = body.get("id").and_then(|v| v.as_i64());

    if let Some(id) = job_id {
        // 编辑作业
        let db = state.db.write().await;
        let (enable, is_cron): (i32, i32) = db
            .query_row("SELECT enable, isCron FROM job WHERE id=?", [id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap_or((0, 0));
        if enable == 1 && is_cron != 2 {
            return Json(ApiResponse::err(&i18n::t("disable_then_edit")));
        }

        let enable = body.get("enable").and_then(|v| v.as_bool()).unwrap_or(true);
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
        let is_cron = body.get("isCron").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let year = body.get("year").and_then(|v| v.as_str()).map(|s| s.to_string());
        let month = body.get("month").and_then(|v| v.as_str()).map(|s| s.to_string());
        let day = body.get("day").and_then(|v| v.as_str()).map(|s| s.to_string());
        let week = body.get("week").and_then(|v| v.as_str()).map(|s| s.to_string());
        let day_of_week = body.get("day_of_week").and_then(|v| v.as_str()).map(|s| s.to_string());
        let hour = body.get("hour").and_then(|v| v.as_str()).map(|s| s.to_string());
        let minute = body.get("minute").and_then(|v| v.as_str()).map(|s| s.to_string());
        let second = body.get("second").and_then(|v| v.as_str()).map(|s| s.to_string());
        let start_date = body.get("start_date").and_then(|v| v.as_str()).map(|s| s.to_string());
        let end_date = body.get("end_date").and_then(|v| v.as_str()).map(|s| s.to_string());
        let exclude = body.get("exclude").and_then(|v| v.as_str()).map(|s| s.to_string());
        let min_file_size = body.get("minFileSize").and_then(|v| v.as_i64());
        let max_file_size = body.get("maxFileSize").and_then(|v| v.as_i64());

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
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("job_updated"))),
            Err(e) => Json(ApiResponse::err(&format!("更新失败: {}", e))),
        }
    } else {
        // 新增作业
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
        let day_of_week = body.get("day_of_week").and_then(|v| v.as_str()).map(|s| s.to_string());
        let hour = body.get("hour").and_then(|v| v.as_str()).map(|s| s.to_string());
        let minute = body.get("minute").and_then(|v| v.as_str()).map(|s| s.to_string());
        let second = body.get("second").and_then(|v| v.as_str()).map(|s| s.to_string());
        let start_date = body.get("start_date").and_then(|v| v.as_str()).map(|s| s.to_string());
        let end_date = body.get("end_date").and_then(|v| v.as_str()).map(|s| s.to_string());
        let exclude = body.get("exclude").and_then(|v| v.as_str()).map(|s| s.to_string());
        let min_file_size = body.get("minFileSize").and_then(|v| v.as_i64());
        let max_file_size = body.get("maxFileSize").and_then(|v| v.as_i64());

        let db = state.db.write().await;
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
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("job_added"))),
            Err(e) => Json(ApiResponse::err(&format!("添加失败: {}", e))),
        }
    }
}

/// PUT /svr/job - 执行/暂停/启用/中止作业
/// 前端: jobPut(data) - body 中有 pause, abort, id 等参数
pub async fn job_put(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let pause = body.get("pause").and_then(|v| v.as_bool());
    let abort = body.get("abort");

    match pause {
        Some(true) => {
            // 禁用/中止作业
            let job_id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let db = state.db.write().await;
            if abort.is_some() {
                // 中止作业
                let _ = db.execute(
                    "UPDATE job_task SET status=7 WHERE jobId=? AND status IN (0, 1)", [job_id],
                );
                Json(ApiResponse::ok_msg(serde_json::json!({}), "作业已中止"))
            } else {
                // 禁用作业
                let is_cron: i32 = db.query_row("SELECT isCron FROM job WHERE id=?", [job_id], |row| row.get(0)).unwrap_or(0);
                if is_cron == 2 {
                    return Json(ApiResponse::err(&i18n::t("cannot_disable_manual_job")));
                }
                let _ = db.execute("UPDATE job SET enable=0 WHERE id=?", [job_id]);
                Json(ApiResponse::ok_msg(serde_json::json!({}), "作业已禁用"))
            }
        }
        Some(false) => {
            // 启用作业
            let job_id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let db = state.db.write().await;
            let _ = db.execute("UPDATE job SET enable=1 WHERE id=?", [job_id]);
            Json(ApiResponse::ok_msg(serde_json::json!({}), "作业已启用"))
        }
        None => {
            // 手动执行
            if let Some(job_id) = body.get("id").and_then(|v| v.as_i64()) {
                // 执行单个作业
                let db = state.db.read().await;
                let enable: i32 = db.query_row("SELECT enable FROM job WHERE id=?", [job_id], |row| row.get(0)).unwrap_or(0);
                if enable != 1 {
                    return Json(ApiResponse::err(&i18n::t("disabled_job_cannot_run")));
                }
                drop(db);
                let db = state.db.write().await;
                let ts = now_ts();
                match db.execute(
                    "INSERT INTO job_task (jobId, status, runTime) VALUES (?, 1, ?)",
                    rusqlite::params![job_id, ts],
                ) {
                    Ok(_) => {
                        let task_id = db.last_insert_rowid();
                        Json(ApiResponse::ok_msg(
                            serde_json::json!({"taskId": task_id}),
                            "作业已开始执行",
                        ))
                    }
                    Err(e) => Json(ApiResponse::err(&format!("执行失败: {}", e))),
                }
            } else {
                // 执行所有启用的作业
                let db = state.db.read().await;
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
                drop(db);
                let db = state.db.write().await;
                let ts = now_ts();
                let mut tasks = vec![];
                for job_id in &job_ids {
                    if let Ok(_) = db.execute(
                        "INSERT INTO job_task (jobId, status, runTime) VALUES (?, 1, ?)",
                        rusqlite::params![job_id, ts],
                    ) {
                        tasks.push(db.last_insert_rowid());
                    }
                }
                Json(ApiResponse::ok_msg(
                    serde_json::json!({"tasks": tasks, "count": tasks.len()}),
                    &format!("已启动 {} 个作业", tasks.len()),
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
    let db = state.db.write().await;

    if let Some(task_id) = body.get("taskId").and_then(|v| v.as_i64()) {
        // 删除任务
        let _ = db.execute("DELETE FROM job_task_item WHERE taskId=?", [task_id]);
        match db.execute("DELETE FROM job_task WHERE id=?", [task_id]) {
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), "任务已删除")),
            Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
        }
    } else if let Some(job_id) = body.get("id").and_then(|v| v.as_i64()) {
        // 删除作业
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