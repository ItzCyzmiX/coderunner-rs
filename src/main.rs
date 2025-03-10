use axum::{routing::post, Router, Json, extract::State};
use serde::{Deserialize, Serialize};
use std::{process::Command, fs, env};
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use tower_http::auth::RequireAuthorizationLayer;

#[derive(Deserialize)]
struct CodeRequest {
    code: String,
    lang: String,
    timeout: Option<u64>,
}

#[derive(Serialize)]
struct CodeResponse {
    output: String,
    error: Option<String>,
}

async fn run_code(State(api_key): State<String>, Json(req): Json<CodeRequest>) -> Json<CodeResponse> {
    if env::var("API_AUTH_KEY").unwrap_or_default() != api_key {
        return Json(CodeResponse {
            output: String::new(),
            error: Some("Invalid authentication credentials".to_string()),
        });
    }
    
    let ext = match req.lang.as_str() {
        "javascript" => ".js",
        _ => ".py",
    };
    
    let filename = format!("/tmp/{}.{}", Uuid::new_v4(), ext);
    if fs::write(&filename, &req.code).is_err() {
        return Json(CodeResponse {
            output: String::new(),
            error: Some("Failed to write temp file".to_string()),
        });
    }
    
    let command = match req.lang.as_str() {
        "javascript" => "node",
        _ => "python3",
    };
    
    let timeout_duration = Duration::from_secs(req.timeout.unwrap_or(5));
    let output = timeout(timeout_duration, async {
        Command::new(command)
            .arg(&filename)
            .output()
    }).await;
    
    let response = match output {
        Ok(Ok(out)) => CodeResponse {
            output: String::from_utf8_lossy(&out.stdout).to_string(),
            error: if out.stderr.is_empty() { None } else { Some(String::from_utf8_lossy(&out.stderr).to_string()) },
        },
        Ok(Err(_)) => CodeResponse {
            output: String::new(),
            error: Some("Failed to execute command".to_string()),
        },
        Err(_) => CodeResponse {
            output: String::new(),
            error: Some("Execution timed out".to_string()),
        },
    };
    
    let _ = fs::remove_file(filename);
    Json(response)
}

#[tokio::main]
async fn main() {
    let api_key = env::var("API_AUTH_KEY").unwrap_or_else(|_| "default_key".to_string());
    let app = Router::new()
        .route("/run", post(run_code))
        .layer(RequireAuthorizationLayer::bearer(&api_key))
        .with_state(api_key);
    
    axum::Server::bind(&"0.0.0.0:8000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
