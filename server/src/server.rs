use actix_web::{web, App, HttpServer};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::ServerConfig;
use std::sync::{Arc, Mutex};

use crate::error::Result;
use crate::links::Links;
use crate::routes::{ok_handler, stage1_handler, stage2_handler, stage3_handler, AppState};
use obfstr::obfstr as s;

pub async fn start(links: Arc<Mutex<Links>>, bind_addr: &str) -> Result<()> {
    let tls_config = build_tls_config();
    let state = web::Data::new(AppState { links });

    // 64 KB limit for registration (lightweight payload).
    let json_cfg_small = web::JsonConfig::default().limit(65_536);
    // 16 MB limit for stage 3: file upload payloads are base64-encoded and can be large.
    let json_cfg_large = web::JsonConfig::default().limit(16 * 1024 * 1024);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .app_data(json_cfg_small.clone())
            .route(s!("/"), web::get().to(ok_handler))
            .route(s!("/js"), web::get().to(stage1_handler))
            .route(s!("/static/register"), web::post().to(stage2_handler))
            .service(
                web::resource(s!("/static/get"))
                    .app_data(json_cfg_large.clone())
                    .route(web::post().to(stage3_handler)),
            )
    })
    .bind_rustls_0_23(bind_addr, tls_config)?
    .run()
    .await
    .map_err(|e| crate::error::LinkyError::ActixWeb(e.into()))
}

/// Generate a self-signed TLS certificate via rcgen (no external openssl needed).
fn build_tls_config() -> ServerConfig {
    use rcgen::{generate_simple_self_signed, CertifiedKey};

    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()])
            .expect("rcgen: failed to generate cert");

    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("rustls: invalid cert/key")
}
