fn main() {
    let macos_app = std::env::var("CARGO_FEATURE_MACOS_APP").is_ok();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if !macos_app || target_os != "macos" {
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");

    cc::Build::new()
        .file(format!("{manifest_dir}/macos/golden_gate_bridge.m"))
        .flag("-fobjc-arc")
        .include(format!("{manifest_dir}/macos"))
        .compile("golden_gate_bridge");

    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rerun-if-changed=macos/golden_gate_bridge.m");
    println!("cargo:rerun-if-changed=macos/golden_gate_bridge.h");

    let framework_dir = std::env::var("SPARKLE_FRAMEWORK_DIR")
        .unwrap_or_else(|_| format!("{manifest_dir}/macos/Frameworks"));
    let framework = format!("{framework_dir}/Sparkle.framework");

    let has_sparkle = std::path::Path::new(&framework).exists();
    if !has_sparkle {
        println!(
            "cargo:warning=Sparkle.framework not found at {framework}; building without in-app updates. Run scripts/fetch_sparkle.sh."
        );
        cc::Build::new()
            .file(format!("{manifest_dir}/macos/sparkle_updater_stub.c"))
            .include(format!("{manifest_dir}/macos"))
            .compile("sparkle_updater");
        return;
    }

    cc::Build::new()
        .file(format!("{manifest_dir}/macos/sparkle_updater.m"))
        .flag("-fobjc-arc")
        .include(format!("{manifest_dir}/macos"))
        .flag(format!("-F{framework_dir}"))
        .compile("sparkle_updater");

    println!("cargo:rustc-link-search=framework={framework_dir}");
    println!("cargo:rustc-link-lib=framework=Sparkle");
    println!("cargo:rerun-if-changed=macos/sparkle_updater.m");
    println!("cargo:rerun-if-changed=macos/sparkle_updater.h");
    println!("cargo:rerun-if-env-changed=SPARKLE_FRAMEWORK_DIR");
}
