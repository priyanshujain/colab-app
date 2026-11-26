fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=libtailscale");
    let status = std::process::Command::new("go")
        .args(["build", "-trimpath", "-buildmode=c-archive", "-o"])
        .arg(out.join("libtailscale.a"))
        .current_dir("libtailscale")
        .status()
        .expect("building colab-server needs Go on PATH");
    assert!(status.success(), "go build of libtailscale failed");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=tailscale");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "macos" {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Security");
    }
}
