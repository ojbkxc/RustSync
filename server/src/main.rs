use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

use crate::config::get_listen_port;
use crate::state::AppState;

mod api;
mod config;
mod data;
mod driver;
mod service;
mod state;

async fn ensure_dirs(config: &config::Config) {
    let _ = tokio::fs::create_dir_all(&config.data_dir).await;
    let _ = tokio::fs::create_dir_all(&config.log_dir).await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = config::get_config();
    ensure_dirs(&config).await;

    // 初始化状态和数据库
    let state = AppState::new_shared(config.clone())?;

    // 初始化数据库
    let generated_password = service::db::init_database(&*state.db.lock().unwrap(), &config)?;

    // 初始化国际化
    service::i18n::load_languages()?;

    let listen_port = get_listen_port();

    // 启动时加载所有启用的作业
    let scheduler_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = service::scheduler::init_all_jobs(&scheduler_state).await {
            tracing::error!("初始化作业失败: {}", e);
        }
    });

    // 构建路由 - 与 Python 单端点模式完全一致
    // Python baseController: 所有非 /svr/noAuth 路径都需要认证
    let auth_routes = Router::new()
        // 用户 - 匹配 Python systemController.User
        .route("/svr/user",
            get(service::auth::get_user)
            .put(service::auth::change_password))
        // 语言 - 匹配 Python systemController.Language
        // GET=获取语言列表, POST=设置语言
        .route("/svr/language",
            get(api::system::get_language)
            .post(api::system::set_language))
        // 引擎 - 匹配 Python jobController.Alist
        // GET=获取引擎列表/子路径, POST=添加引擎, PUT=更新引擎, DELETE=删除引擎
        .route("/svr/alist",
            get(api::engine::alist_get)
            .post(api::engine::alist_post)
            .put(api::engine::alist_put)
            .delete(api::engine::alist_delete))
        // 存储挂载 - 匹配 Python jobController.Storage
        // GET=获取挂载列表/浏览/发现, POST=添加挂载/测试/浏览, PUT=更新挂载, DELETE=删除挂载
        .route("/svr/storage",
            get(api::engine::storage_get)
            .post(api::engine::storage_post)
            .put(api::engine::storage_put)
            .delete(api::engine::storage_delete))
        // 作业/任务 - 匹配 Python jobController.Job
        // GET=获取作业列表/任务列表/任务子项, POST=添加/编辑作业, PUT=执行/暂停/启用/中止, DELETE=删除作业/任务
        .route("/svr/job",
            get(api::job::job_get)
            .post(api::job::job_post)
            .put(api::job::job_put)
            .delete(api::job::job_delete))
        // 通知 - 匹配 Python notifyController.Notify
        // GET=获取通知列表, POST=添加/测试通知, PUT=更新状态/编辑通知, DELETE=删除通知
        .route("/svr/notify",
            get(api::notify::list_notifies)
            .post(api::notify::add_notify)
            .put(api::notify::update_notify)
            .delete(api::notify::delete_notify))
        // 文件管理器
        .route("/svr/files/list", get(api::files::list_files))
        .route("/svr/files/read", get(api::files::read_file))
        .route("/svr/files/write", post(api::files::write_file))
        .route("/svr/files/mkdir", post(api::files::create_dir))
        .route("/svr/files/touch", post(api::files::create_file))
        .route("/svr/files/delete", post(api::files::delete_file))
        .route("/svr/files/rename", post(api::files::rename_file))
        .route("/svr/files/copy", post(api::files::copy_file))
        .route("/svr/files/info", get(api::files::file_info))
        .route("/svr/files/upload", post(api::files::upload_file))
        .route("/svr/files/download", get(api::files::download_file))
        .route("/svr/files/dirsize", get(api::files::dir_size))
        // 日志查看
        .route("/svr/log/list", get(api::system::log_list))
        .route("/svr/log/read", get(api::system::log_read))
        .route("/svr/log/clear", post(api::system::log_clear))
        // 认证中间件 - 匹配 Python baseController.handle_request 的鉴权逻辑
        .route_layer(axum::middleware::from_fn(service::auth::require_auth));

    let app = Router::new()
        // 系统首页
        .route("/", get(api::system::index))
        // 认证（无需鉴权）- 匹配 Python systemController.Login
        // POST=登录, PUT=重置密码, DELETE=登出
        .route("/svr/noAuth/login",
            post(service::auth::login)
            .put(service::auth::reset_password)
            .delete(service::auth::logout))
        .merge(auth_routes)
        // SPA fallback - 必须放在最后
        .route("/*path", get(api::system::spa_fallback))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port));

    // 打印启动信息
    let success_msg = service::i18n::t_format(
        "running_success",
        &[("url", &format!("http://127.0.0.1:{}/", listen_port))],
    );
    tracing::info!("{}", success_msg);

    if let Some(pwd) = generated_password {
        tracing::info!("初始管理员密码: {}", pwd);
    }

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}