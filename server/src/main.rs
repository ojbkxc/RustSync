use axum::{
    routing::{get, post, put, delete},
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

    let state = AppState::new_shared(config.clone())?;

    let generated_password = {
        let conn = state.db.get()?;
        service::db::init_database(&conn, &config)?
    };

    service::i18n::load_languages()?;

    let listen_port = get_listen_port();

    let scheduler_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = service::scheduler::init_all_jobs(&scheduler_state).await {
            tracing::error!("初始化作业失败: {}", e);
        }
    });

    // ========== 无需认证的路由 ==========
    let public_routes = Router::new()
        .route("/api/auth/login", post(service::auth::login))
        .route("/api/auth/reset-password", put(service::auth::reset_password))
        .route("/api/auth/logout", delete(service::auth::logout))
        .route("/", get(api::system::index));

    // ========== 需要认证的路由 ==========
    let auth_routes = Router::new()
        // 用户
        .route("/api/user", get(service::auth::get_user))
        .route("/api/user/password", put(service::auth::change_password))
        // 引擎
        .route("/api/engines", get(api::engine::list_engines).post(api::engine::add_engine))
        .route("/api/engines/:id", put(api::engine::update_engine).delete(api::engine::delete_engine))
        .route("/api/engines/:id/browse", get(api::engine::browse_engine))
        // 存储
        .route("/api/storage", get(api::engine::list_storage).post(api::engine::add_storage))
        .route("/api/storage/:id", put(api::engine::update_storage).delete(api::engine::delete_storage))
        .route("/api/storage/local-browse", get(api::engine::local_browse))
        .route("/api/storage/smb-discover", get(api::engine::smb_discover))
        .route("/api/storage/sftp-test", post(api::engine::sftp_test))
        .route("/api/storage/sftp-browse", post(api::engine::sftp_browse))
        // 作业
        .route("/api/jobs", get(api::job::list_jobs))
        .route("/api/jobs", post(api::job::create_job))
        .route("/api/jobs/:id", put(api::job::update_job))
        .route("/api/jobs/:id", delete(api::job::delete_job))
        .route("/api/jobs/:id/run", post(api::job::run_job))
        .route("/api/jobs/:id/pause", post(api::job::pause_job))
        .route("/api/jobs/:id/resume", post(api::job::resume_job))
        .route("/api/jobs/:id/current", get(api::job::job_current))
        .route("/api/jobs/:id/tasks", get(api::job::job_tasks))
        .route("/api/tasks/:id", delete(api::job::delete_task))
        .route("/api/tasks/:id/items", get(api::job::task_items))
        // 通知
        .route("/api/notifications", get(api::notify::list_notifies).post(api::notify::add_notify))
        .route("/api/notifications/test", post(api::notify::test_notify))
        .route("/api/notifications/:id", put(api::notify::update_notify).delete(api::notify::delete_notify))
        .route("/api/notifications/:id/toggle", put(api::notify::toggle_notify))
        // 文件管理
        .route("/api/files/list", get(api::files::list_files))
        .route("/api/files/read", get(api::files::read_file))
        .route("/api/files/write", post(api::files::write_file))
        .route("/api/files/mkdir", post(api::files::create_dir))
        .route("/api/files/touch", post(api::files::create_file))
        .route("/api/files/delete", post(api::files::delete_file))
        .route("/api/files/rename", post(api::files::rename_file))
        .route("/api/files/copy", post(api::files::copy_file))
        .route("/api/files/info", get(api::files::file_info))
        .route("/api/files/upload", post(api::files::upload_file))
        .route("/api/files/download", get(api::files::download_file))
        .route("/api/files/dirsize", get(api::files::dir_size))
        // 系统
        .route("/api/system/language", get(api::system::get_language).post(api::system::set_language))
        .route("/api/system/logs", get(api::system::log_list))
        .route("/api/system/logs/read", get(api::system::log_read))
        .route("/api/system/logs/clear", post(api::system::log_clear))
        .with_state(state.clone())
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), service::auth::require_auth));

    let app = Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .route("/*path", get(api::system::spa_fallback))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port));

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