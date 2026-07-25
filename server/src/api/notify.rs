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
    let db = state.db.lock().unwrap();
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
        let db = state.db.lock().unwrap();
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
    let db = state.db.lock().unwrap();

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
    let db = state.db.lock().unwrap();
    match db.execute("DELETE FROM notify WHERE id=?", [notify_id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("notify_deleted"))),
        Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
    }
}

/// 发送通知到指定配置
/// 与 Python notifyService.sendNotify 行为一致
pub async fn send_notification(method: i32, params: &str, title: &str, body: &str) -> anyhow::Result<()> {
    let params: serde_json::Value = serde_json::from_str(params)?;
    let client = reqwest::Client::new();

    // 检查 notSendNull 参数（与 Python 一致，sync_engine 调用时使用）
    let _not_send_null = params.get("notSendNull").and_then(|v| v.as_bool()).unwrap_or(false);

    match method {
        0 => {
            // 自定义 Webhook - 与 Python 完全一致
            let url = params.get("url").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("自定义 Webhook 缺少 url 参数"))?;
            let http_method = params.get("method").and_then(|v| v.as_str()).unwrap_or("POST");
            let content_type = params.get("contentType").and_then(|v| v.as_str()).unwrap_or("application/json");
            let need_content = params.get("needContent").and_then(|v| v.as_bool()).unwrap_or(true);
            let title_name = params.get("titleName").and_then(|v| v.as_str()).unwrap_or("title");
            let content_name = params.get("contentName").and_then(|v| v.as_str()).unwrap_or("content");

            let mut req_data = serde_json::json!({
                title_name: title,
            });
            if need_content {
                req_data[content_name] = serde_json::Value::String(body.to_string());
            }

            let req = match http_method {
                "GET" => client.get(url).query(&req_data.as_object().map(|m| {
                    m.iter().map(|(k, v)| (k.as_str(), v.as_str().unwrap_or(""))).collect::<Vec<_>>()
                }).unwrap_or_default()),
                "POST" => {
                    if content_type == "application/json" {
                        client.post(url).json(&req_data)
                    } else if content_type == "application/x-www-form-urlencoded" {
                        client.post(url).form(&req_data)
                    } else {
                        return Err(anyhow::anyhow!("ContentType not allowed"));
                    }
                }
                "PUT" => {
                    if content_type == "application/json" {
                        client.put(url).json(&req_data)
                    } else if content_type == "application/x-www-form-urlencoded" {
                        client.put(url).form(&req_data)
                    } else {
                        return Err(anyhow::anyhow!("ContentType not allowed"));
                    }
                }
                _ => return Err(anyhow::anyhow!("Method not supported")),
            };

            let resp = req.send().await?;
            if resp.status() != 200 {
                let body_text = resp.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!("自定义 Webhook 返回非 200: {}", body_text));
            }
        }
        1 => {
            // Server酱 - 与 Python sc.send 一致
            if let Some(key) = params.get("sendKey").and_then(|v| v.as_str()) {
                let url = format!("https://sctapi.ftqq.com/{}.send", key);
                let resp = client
                    .post(&url)
                    .form(&[("title", title), ("desp", body)])
                    .send()
                    .await?;
                if resp.status() != 200 {
                    let body_text = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("Server酱发送失败: {}", body_text));
                }
            }
        }
        2 => {
            // 钉钉群机器人 - 与 Python 一致，检查 errcode
            if let Some(webhook) = params.get("url").and_then(|v| v.as_str()) {
                let payload = serde_json::json!({
                    "msgtype": "text",
                    "text": {
                        "content": format!("{}\n\n{}", title, body)
                    }
                });
                let resp = client.post(webhook).json(&payload).send().await?;
                let rst: serde_json::Value = resp.json().await?;
                if rst.get("errcode").and_then(|v| v.as_i64()).unwrap_or(-1) != 0 {
                    return Err(anyhow::anyhow!("钉钉发送失败: {}",
                        rst.get("errmsg").and_then(|v| v.as_str()).unwrap_or("unknown")));
                }
            }
        }
        3 => {
            // 企业微信应用消息 - 与 Python 一致，使用 corpsecret 字段名
            let corp_id = params.get("corpid").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("企业微信缺少 corpid"))?;
            let corp_secret = params.get("corpsecret").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("企业微信缺少 corpsecret"))?;
            let agent_id = params.get("agentid").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("企业微信缺少 agentid"))?;
            let to_user = params.get("touser").and_then(|v| v.as_str()).unwrap_or("@all");

            // 获取 access_token
            let token_url = format!(
                "https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}",
                corp_id, corp_secret
            );
            let token_resp: serde_json::Value = client.get(&token_url).send().await?.json().await?;
            if token_resp.get("errcode").and_then(|v| v.as_i64()).unwrap_or(-1) != 0 {
                return Err(anyhow::anyhow!("获取企业微信 access_token 失败: {}",
                    token_resp.get("errmsg").and_then(|v| v.as_str()).unwrap_or("unknown")));
            }
            let access_token = token_resp.get("access_token").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("企业微信 access_token 为空"))?;

            let msg_url = format!(
                "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
                access_token
            );
            let payload = serde_json::json!({
                "touser": to_user,
                "msgtype": "text",
                "agentid": agent_id,
                "text": {
                    "content": format!("{}\n-------------------\n{}", title, body)
                },
                "safe": 0,
                "enable_id_trans": 0,
                "enable_duplicate_check": 0
            });
            let resp = client.post(&msg_url).json(&payload).send().await?;
            let rst: serde_json::Value = resp.json().await?;
            if rst.get("errcode").and_then(|v| v.as_i64()).unwrap_or(-1) != 0 {
                return Err(anyhow::anyhow!("发送企业微信消息失败: {}",
                    rst.get("errmsg").and_then(|v| v.as_str()).unwrap_or("unknown")));
            }
        }
        4 => {
            // Lark 群机器人 - 与 Python 一致，使用 interactive card 格式
            if let Some(webhook) = params.get("url").and_then(|v| v.as_str()) {
                let payload = serde_json::json!({
                    "msg_type": "interactive",
                    "card": {
                        "config": {
                            "wide_screen_mode": true
                        },
                        "elements": [{
                            "tag": "markdown",
                            "content": body
                        }],
                        "header": {
                            "template": "blue",
                            "title": {
                                "content": title,
                                "tag": "plain_text"
                            }
                        }
                    }
                });
                let resp = client.post(webhook).json(&payload).send().await?;
                let rst: serde_json::Value = resp.json().await?;
                if rst.get("code").and_then(|v| v.as_i64()).unwrap_or(-1) != 0 {
                    return Err(anyhow::anyhow!("Lark 群机器人发送失败: {}",
                        rst.get("msg").and_then(|v| v.as_str()).unwrap_or("unknown")));
                }
            }
        }
        _ => return Err(anyhow::anyhow!("不支持的通知方式: {}", method)),
    }
    Ok(())
}