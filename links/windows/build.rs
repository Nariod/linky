fn main() {
    // CALLBACK is injected by `cargo build --env CALLBACK=<ip:port>`
    let callback = std::env::var("CALLBACK").unwrap_or_else(|_| "127.0.0.1:443".to_string());
    println!("cargo:rustc-env=CALLBACK={}", callback);
    println!("cargo:rerun-if-env-changed=CALLBACK");
}
