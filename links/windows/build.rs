fn main() {
    // CALLBACK is injected by `cargo build --env CALLBACK=<ip:port>`
    let callback = std::env::var("CALLBACK").unwrap_or_else(|_| "127.0.0.1:443".to_string());
    println!("cargo:rustc-env=CALLBACK={}", callback);

    // IMPLANT_SECRET is injected by generate.rs for per-implant key derivation
    let secret = std::env::var("IMPLANT_SECRET").unwrap_or_else(|_| {
        // Fallback to a default secret if not provided (for testing)
        // Use a simple hex string representation of zeros
        "0000000000000000000000000000000000000000000000000000000000000000".to_string()
    });
    println!("cargo:rustc-env=IMPLANT_SECRET={}", secret);

    println!("cargo:rerun-if-env-changed=CALLBACK");
    println!("cargo:rerun-if-env-changed=IMPLANT_SECRET");
}
