use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for key in ["RUSTC", "CARGO_ENCODED_RUSTFLAGS"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let rustc = Command::new(env::var_os("RUSTC").expect("Cargo provides RUSTC"))
        .arg("--version")
        .output()
        .expect("cannot read compiler version");
    assert!(rustc.status.success(), "rustc --version failed");
    println!(
        "cargo:rustc-env=VARYNTH_BENCH_RUSTC={}",
        String::from_utf8_lossy(&rustc.stdout).trim()
    );
    for key in ["TARGET", "PROFILE", "OPT_LEVEL", "CARGO_CFG_TARGET_FEATURE"] {
        println!(
            "cargo:rustc-env=VARYNTH_BENCH_{key}={}",
            env::var(key).unwrap_or_default()
        );
    }
    // Preserve flag boundaries rather than ambiguously joining with spaces.
    let flags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    println!(
        "cargo:rustc-env=VARYNTH_BENCH_RUSTFLAGS={:?}",
        flags
            .split('\x1f')
            .filter(|flag| !flag.is_empty())
            .collect::<Vec<_>>()
    );
}
