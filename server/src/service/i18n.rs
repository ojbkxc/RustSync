use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LanguagePack {
    #[serde(flatten)]
    pub entries: HashMap<String, String>,
}

static CURRENT_LANG: RwLock<String> = RwLock::new(String::new());
static LANG_PACKS: RwLock<HashMap<String, LanguagePack>> = RwLock::new(HashMap::new());

/// 加载所有语言包
pub fn load_languages() -> anyhow::Result<()> {
    let locales_dir = "locales";
    if !std::path::Path::new(locales_dir).exists() {
        // 如果没有 locales 目录，加载内置语言包
        load_builtin_languages()?;
    } else {
        let mut packs = HashMap::new();
        for entry in std::fs::read_dir(locales_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "yaml" || e == "yml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let content = std::fs::read_to_string(&path)?;
                    let pack: LanguagePack = serde_yaml::from_str(&content)?;
                    packs.insert(stem.to_string(), pack);
                }
            }
        }

        if packs.is_empty() {
            load_builtin_languages()?;
        } else {
            let mut lang = LANG_PACKS.write().unwrap();
            *lang = packs;
        }
    }

    // 从 data/language.txt 恢复上次的语言设置（与 Python 一致）
    let config = crate::config::get_config();
    let lang_file = std::path::Path::new(&config.data_dir).join("language.txt");
    if lang_file.exists() {
        if let Ok(saved_lang) = std::fs::read_to_string(&lang_file) {
            let saved_lang = saved_lang.trim().to_string();
            if !saved_lang.is_empty() {
                let packs = LANG_PACKS.read().unwrap();
                let lang_key = if packs.contains_key(&saved_lang) {
                    saved_lang
                } else {
                    // 兼容旧值 zh_cn/eng
                    match saved_lang.as_str() {
                        "zh_cn" | "zh_CN" => "zh-CN".to_string(),
                        "eng" => "en".to_string(),
                        _ => "zh-CN".to_string(),
                    }
                };
                *CURRENT_LANG.write().unwrap() = lang_key;
                return Ok(());
            }
        }
    }

    // 默认中文
    let mut cur = CURRENT_LANG.write().unwrap();
    if cur.is_empty() {
        *cur = "zh-CN".to_string();
    }

    Ok(())
}

fn load_builtin_languages() -> anyhow::Result<()> {
    let mut packs = HashMap::new();

    // 内置中文
    let mut zh = HashMap::new();
    zh.insert("running_success".to_string(), "RustSync 已启动: {url}".to_string());
    zh.insert("wrong_password".to_string(), "密码错误".to_string());
    zh.insert("user_not_found".to_string(), "用户不存在".to_string());
    zh.insert("not_logged_in".to_string(), "未登录".to_string());
    zh.insert("login_success".to_string(), "登录成功".to_string());
    zh.insert("logout_success".to_string(), "已登出".to_string());
    zh.insert("password_changed".to_string(), "密码修改成功".to_string());
    zh.insert("old_password_wrong".to_string(), "原密码错误".to_string());
    zh.insert("engine_added".to_string(), "引擎添加成功".to_string());
    zh.insert("engine_updated".to_string(), "引擎更新成功".to_string());
    zh.insert("engine_deleted".to_string(), "引擎删除成功".to_string());
    zh.insert("engine_not_found".to_string(), "引擎不存在".to_string());
    zh.insert("builtin_engine_protected".to_string(), "内置引擎不可删除".to_string());
    zh.insert("job_added".to_string(), "作业添加成功".to_string());
    zh.insert("job_updated".to_string(), "作业更新成功".to_string());
    zh.insert("job_deleted".to_string(), "作业删除成功".to_string());
    zh.insert("job_not_found".to_string(), "作业不存在".to_string());
    zh.insert("disable_then_edit".to_string(), "请先禁用作业再编辑".to_string());
    zh.insert("disabled_job_cannot_run".to_string(), "禁用的作业不可手动执行".to_string());
    zh.insert("cannot_disable_manual_job".to_string(), "仅手动执行的作业不可禁用".to_string());
    zh.insert("no_job_for_run".to_string(), "没有可执行的作业".to_string());
    zh.insert("notify_added".to_string(), "通知配置添加成功".to_string());
    zh.insert("notify_updated".to_string(), "通知配置更新成功".to_string());
    zh.insert("notify_deleted".to_string(), "通知配置删除成功".to_string());
    zh.insert("notify_not_found".to_string(), "通知配置不存在".to_string());
    zh.insert("mount_added".to_string(), "挂载目录添加成功".to_string());
    zh.insert("mount_updated".to_string(), "挂载目录更新成功".to_string());
    zh.insert("mount_deleted".to_string(), "挂载目录删除成功".to_string());
    zh.insert("mount_not_found".to_string(), "挂载目录不存在".to_string());
    zh.insert("source_target_overlap".to_string(), "源目录与目标目录存在包含关系".to_string());
    zh.insert("file_size_invalid".to_string(), "文件大小格式无效".to_string());
    zh.insert("file_size_range_invalid".to_string(), "最小文件大小不能大于最大文件大小".to_string());
    zh.insert("source_mode_invalid".to_string(), "源目录模式参数无效".to_string());
    packs.insert("zh-CN".to_string(), LanguagePack { entries: zh });

    // 内置英文
    let mut en = HashMap::new();
    en.insert("running_success".to_string(), "RustSync started: {url}".to_string());
    en.insert("wrong_password".to_string(), "Wrong password".to_string());
    en.insert("user_not_found".to_string(), "User not found".to_string());
    en.insert("not_logged_in".to_string(), "Not logged in".to_string());
    en.insert("login_success".to_string(), "Login successful".to_string());
    en.insert("logout_success".to_string(), "Logged out".to_string());
    en.insert("password_changed".to_string(), "Password changed".to_string());
    en.insert("old_password_wrong".to_string(), "Old password is wrong".to_string());
    en.insert("engine_added".to_string(), "Engine added".to_string());
    en.insert("engine_updated".to_string(), "Engine updated".to_string());
    en.insert("engine_deleted".to_string(), "Engine deleted".to_string());
    en.insert("engine_not_found".to_string(), "Engine not found".to_string());
    en.insert("builtin_engine_protected".to_string(), "Built-in engine cannot be deleted".to_string());
    en.insert("job_added".to_string(), "Job added".to_string());
    en.insert("job_updated".to_string(), "Job updated".to_string());
    en.insert("job_deleted".to_string(), "Job deleted".to_string());
    en.insert("job_not_found".to_string(), "Job not found".to_string());
    en.insert("disable_then_edit".to_string(), "Please disable the job first".to_string());
    en.insert("disabled_job_cannot_run".to_string(), "Disabled job cannot be run manually".to_string());
    en.insert("cannot_disable_manual_job".to_string(), "Manual-only job cannot be disabled".to_string());
    en.insert("no_job_for_run".to_string(), "No jobs available to run".to_string());
    en.insert("notify_added".to_string(), "Notification config added".to_string());
    en.insert("notify_updated".to_string(), "Notification config updated".to_string());
    en.insert("notify_deleted".to_string(), "Notification config deleted".to_string());
    en.insert("notify_not_found".to_string(), "Notification config not found".to_string());
    en.insert("mount_added".to_string(), "Mount added".to_string());
    en.insert("mount_updated".to_string(), "Mount updated".to_string());
    en.insert("mount_deleted".to_string(), "Mount deleted".to_string());
    en.insert("mount_not_found".to_string(), "Mount not found".to_string());
    en.insert("source_target_overlap".to_string(), "Source and target directories overlap".to_string());
    en.insert("file_size_invalid".to_string(), "Invalid file size format".to_string());
    en.insert("file_size_range_invalid".to_string(), "Min file size cannot exceed max file size".to_string());
    en.insert("source_mode_invalid".to_string(), "Invalid source mode parameter".to_string());
    packs.insert("en".to_string(), LanguagePack { entries: en });

    let mut lang = LANG_PACKS.write().unwrap();
    *lang = packs;
    Ok(())
}

/// 获取当前语言
pub fn get_current_lang() -> String {
    CURRENT_LANG.read().unwrap().clone()
}

/// 设置当前语言并持久化到 data/language.txt
/// 与 Python language() 函数行为一致
pub fn set_current_lang(lang: &str) {
    let supported = LANG_PACKS.read().unwrap();
    let lang_key = if supported.contains_key(lang) {
        lang.to_string()
    } else {
        "zh-CN".to_string()
    };
    *CURRENT_LANG.write().unwrap() = lang_key.clone();

    // 持久化到 data/language.txt（与 Python 一致）
    let config = crate::config::get_config();
    let lang_file = std::path::Path::new(&config.data_dir).join("language.txt");
    let _ = std::fs::write(&lang_file, &lang_key);
}

/// 获取翻译文本
pub fn t(key: &str) -> String {
    let lang = CURRENT_LANG.read().unwrap();
    let packs = LANG_PACKS.read().unwrap();
    let lang_key = if lang.is_empty() { "zh-CN" } else { &lang };

    packs
        .get(lang_key)
        .and_then(|p| p.entries.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// 带参数替换的翻译
pub fn t_format(key: &str, params: &[(&str, &str)]) -> String {
    let mut text = t(key);
    for (k, v) in params {
        text = text.replace(&format!("{{{}}}", k), v);
    }
    text
}

/// 获取支持的语言列表
pub fn get_supported_languages() -> Vec<String> {
    LANG_PACKS.read().unwrap().keys().cloned().collect()
}