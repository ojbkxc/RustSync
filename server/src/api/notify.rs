use axum::{
    extract::State,
    Json,
};
use crate::data::models::Notify;
use crate::data::response::ApiResponse;
use crate::service::i18n;

/// GET /svr/notify - 获取通知配置列表
/// 匹配 Python notifyController.Notify.get()
pub async fn list_notifies(
    State(state): State<crate::state::SharedState>,
) -> Json<ApiResponse<Vec<Notify>>> {
    let db = state.db.read().await;
    let mut stmt = match db.prepare(
        "SELECT id, enable, method, params, createTime FROM notify ORDER BY id",
    ) {
        Ok(s) => s,
        Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
    };
    let rows = stmt.query_map([], |row| {
        Ok(Notify {
            id: row.get(0)?,
            enable: row.get::<_, i32>(1)? != 0,
            method: row.get(2)?,
            params: row.get(3)?,
            create_time: row.get(4)?,
        })
    });
    match rows {
        Ok(iter) => {
            let list: Vec<Notify> = iter.filter_map(|r| r.ok()).collect();
            Json(ApiResponse::ok(list))
        }
        Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))),
    }
}

/// POST /svr/notify - 添加通知或测试通知
/// Python body: {notify: {enable, method, params}} 添加
/// Python body: {notify: {method, params}} 测试（无 enable 字段）
pub async fn add_notify(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let notify_data = match body.get("notify") {
        Some(v) => v,
        None => return Json(ApiResponse::err("缺少通知配置(notify)")),
    };

    // 如果 notify 中包含 enable 字段，则为添加新通知
    if notify_data.get("enable").is_some() {
        let method = notify_data.get("method").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let params = notify_data.get("params").cloned().unwrap_or_default();
        let params_str = serde_json::to_string(&params).unwrap_or_default();
        let db = state.db.write().await;
        match db.execute(
            "INSERT INTO notify (enable, method, params) VALUES (1, ?, ?)",
            rusqlite::params![method, params_str],
        ) {
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("notify_added"))),
            Err(e) => Json(ApiResponse::err(&format!("添加失败: {}", e))),
        }
    } else {
        // 测试通知
        let method = notify_data.get("method").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let params = notify_data.get("params").cloned().unwrap_or_default();
        let params_str = serde_json::to_string(&params).unwrap_or_default();

        match send_notification(method, &params_str, "RustSync 测试通知", "这是一条测试消息，如果您收到此消息，说明通知配置正确。").await {
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), "测试通知发送成功")),
            Err(e) => Json(ApiResponse::err(&format!("发送失败: {}", e))),
        }
    }
}

/// PUT /svr/notify - 更新通知状态或编辑通知
/// Python body: {notifyId, enable} 切换启用状态
/// Python body: {notify: {id, method, params}} 编辑通知
pub async fn update_notify(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let db = state.db.write().await;

    // 判断是切换状态还是编辑通知
    if let Some(notify_id) = body.get("notifyId").and_then(|v| v.as_i64()) {
        // 切换启用/禁用状态
        let enable = body.get("enable").and_then(|v| v.as_bool()).unwrap_or(false);
        match db.execute(
            "UPDATE notify SET enable=? WHERE id=?",
            rusqlite::params![enable as i32, notify_id],
        ) {
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("notify_updated"))),
            Err(e) => Json(ApiResponse::err(&format!("更新失败: {}", e))),
        }
    } else if let Some(notify) = body.get("notify") {
        // 编辑通知配置
        let id = notify.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        if id == 0 {
            return Json(ApiResponse::err("缺少通知ID"));
        }
        let method = notify.get("method").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let params = notify.get("params").cloned().unwrap_or_default();
        let params_str = serde_json::to_string(&params).unwrap_or_default();

        match db.execute(
            "UPDATE notify SET method=?, params=? WHERE id=?",
            rusqlite::params![method, params_str, id],
        ) {
            Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("notify_updated"))),
            Err(e) => Json(ApiResponse::err(&format!("更新失败: {}", e))),
        }
    } else {
        Json(ApiResponse::err("缺少 notifyId 或 notify 参数"))
    }
}

/// DELETE /svr/notify - 删除通知
/// Python body: {notifyId}
pub async fn delete_notify(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let notify_id = match body.get("notifyId").and_then(|v| v.as_i64()) {
        Some(id) => id,
        None => return Json(ApiResponse::err("缺少 notifyId 参数")),
    };
    let db = state.db.write().await;
    match db.execute("DELETE FROM notify WHERE id=?", [notify_id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("notify_deleted"))),
        Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
    }
}

/// 发送通知到指定配置
pub async fn send_notification(method: i32, params: &str, title: &str, body: &str) -> anyhow::Result<()> {
    let params: serde_json::Value = serde_json::from_str(params)?;
    let client = reqwest::Client::new();

    match method {
        0 => {
            // 自定义 Webhook
            if let Some(url) = params.get("url").and_then(|v| v.as_str()) {
                let payload = serde_json::json!({
                    "title": title,
                    "body": body,
                });
                client.post(url).json(&payload).send().await?;
            }
        }
        1 => {
            // Server酱
            if let Some(key) = params.get("key").and_then(|v| v.as_str()) {
                let url = format!("https://sctapi.ftqq.com/{}.send", key);
                client
                    .post(&url)
                    .form(&[("title", title), ("desp", body)])
                    .send()
                    .await?;
            }
        }
        2 => {
            // 钉钉群机器人
            if let Some(webhook) = params.get("webhook").and_then(|v| v.as_str()) {
                let payload = serde_json::json!({
                    "msgtype": "text",
                    "text": {
                        "content": format!("{}\n{}", title, body)
                    }
                });
                client.post(webhook).json(&payload).send().await?;
            }
        }
        3 => {
            // 企业微信应用消息
            if let (Some(corp_id), Some(secret), Some(agent_id), Some(to_user)) = (
                params.get("corp_id").and_then(|v| v.as_str()),
                params.get("secret").and_then(|v| v.as_str()),
                params.get("agent_id").and_then(|v| v.as_str()),
                params.get("to_user").and_then(|v| v.as_str()),
            ) {
                // 获取 access_token
                let token_url = format!(
                    "https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}",
                    corp_id, secret
                );
                let token_resp: serde_json::Value = client.get(&token_url).send().await?.json().await?;
                if let Some(token) = token_resp.get("access_token").and_then(|v| v.as_str()) {
                    let msg_url = format!(
                        "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
                        token
                    );
                    let payload = serde_json::json!({
                        "touser": to_user,
                        "msgtype": "text",
                        "agentid": agent_id,
                        "text": {
                            "content": format!("{}\n{}", title, body)
                        }
                    });
                    client.post(&msg_url).json(&payload).send().await?;
                }
            }
        }
        4 => {
            // Lark 群机器人
            if let Some(webhook) = params.get("webhook").and_then(|v| v.as_str()) {
                let payload = serde_json::json!({
                    "msg_type": "text",
                    "content": {
                        "text": format!("{}\n{}", title, body)
                    }
                });
                client.post(webhook).json(&payload).send().await?;
            }
        }
        _ => {}
    }
    Ok(())
}