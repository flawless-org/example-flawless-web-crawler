use std::{sync::Arc, time::Duration};

use askama::Template;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Form, Json, Router,
};
use crawler::Job;
use flawless_utils::DeployedModule;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let flawless = flawless_utils::Server::new("http://localhost:27288", None);
    let flawless_module = flawless_utils::load_module_from_build!("crawler");
    let module = flawless.deploy(flawless_module).await.unwrap();

    let app = Router::new()
        .route("/", get(index))
        .route("/timeout", get(timeout))
        .route("/new-job", post(new_job))
        .route("/list", get(list))
        .route("/ui-update", post(ui_update))
        .route("/assets/*path", get(handle_assets))
        .with_state(AppState::new(module));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("The server is running at: http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Clone)]
struct AppState {
    inner: Arc<StateInner>,
}

#[derive(Debug)]
struct StateInner {
    module: DeployedModule,
    progress: Mutex<Vec<JobProgress>>,
}

impl AppState {
    fn new(module: DeployedModule) -> Self {
        AppState {
            inner: Arc::new(StateInner {
                module,
                progress: Mutex::new(Vec::new()),
            }),
        }
    }

    fn module(&self) -> &DeployedModule {
        &self.inner.module
    }

    async fn progress(&self) -> Vec<JobProgress> {
        self.inner.progress.lock().await.clone()
    }

    async fn add_job(&self, url: String) -> usize {
        let mut progress = self.inner.progress.lock().await;
        progress.push(JobProgress {
            url,
            list: Vec::new(),
        });
        progress.len() - 1
    }

    async fn update_job(&self, id: usize, status: Status, url: String) {
        let mut progress = self.inner.progress.lock().await;
        let job = progress.get_mut(id).unwrap();

        let list_element = if url == "last" {
            job.list.last_mut()
        } else {
            None
        };

        match list_element {
            Some(list_element) => {
                list_element.0 = status.to_color();
                list_element.1 = status.to_string();
            }
            None => job.list.push((status.to_color(), status.to_string(), url)),
        }
    }
}

async fn timeout() -> impl IntoResponse {
    tokio::time::sleep(Duration::from_secs(60)).await;
    Html("Response after 60 seconds")
}

#[derive(Debug, Clone)]
struct JobProgress {
    url: String,
    // color, status, url
    list: Vec<(String, String, String)>,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate;

async fn index() -> impl IntoResponse {
    Html(IndexTemplate.render().expect("index renders"))
}

#[derive(Deserialize)]
struct NewJob {
    url: String,
}

async fn new_job(State(state): State<AppState>, Form(job): Form<NewJob>) -> impl IntoResponse {
    let url = job.url;
    let id = state.add_job(url.clone()).await;

    state
        .module()
        .start::<crawler::start_crawler>(Job { id, url })
        .await
        .unwrap();

    list(State(state)).await
}

async fn ui_update(
    State(state): State<AppState>,
    Json(ui_update): Json<UpdateUI>,
) -> impl IntoResponse {
    state
        .update_job(ui_update.id, ui_update.status, ui_update.url)
        .await;
    Html("OK")
}

static STYLE_CSS: &str = include_str!("../assets/styles.css");

async fn handle_assets(Path(path): Path<String>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();

    if path == "styles.css" {
        headers.insert(header::CONTENT_TYPE, "text/css".parse().unwrap());
        (StatusCode::OK, headers, STYLE_CSS)
    } else {
        (StatusCode::NOT_FOUND, headers, "")
    }
}

#[derive(Template)]
#[template(path = "list.html")]
struct ListTemplate {
    progress: Vec<JobProgress>,
}

async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let progress = state.progress().await;
    // Display max 10 last requests.
    let progress = progress
        .iter()
        .map(|job| {
            let url = job.url.clone();
            let list = job.list.iter().rev().take(10).map(|e| e.clone()).collect();
            JobProgress { url, list }
        })
        .collect();
    let template = ListTemplate { progress };
    Html(template.render().expect("index renders"))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUI {
    pub id: usize,
    pub status: Status,
    pub url: String,
    pub urls_left: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Status {
    // Request in progress.
    Request,
    // Parsing in progress.
    Parse,
    // Finished.
    Done,
    // Error
    Error,
}

impl Status {
    fn to_string(&self) -> String {
        match self {
            Status::Request => "REQ".to_string(),
            Status::Parse => "PARS".to_string(),
            Status::Done => "DONE".to_string(),
            Status::Error => "ERR".to_string(),
        }
    }
    fn to_color(&self) -> String {
        match self {
            Status::Request => "orange".to_string(),
            Status::Parse => "yellow".to_string(),
            Status::Done => "green".to_string(),
            Status::Error => "red".to_string(),
        }
    }
}
