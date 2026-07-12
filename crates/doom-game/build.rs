//! Build script for doom-game.
//!
//! When DOOM_VERIFY=1 is set, runs the Verus verifier on proofs.rs.
//! Regular `cargo build` / `cargo test` skip this step entirely.
//!
//! Mirrors `doom-types/build.rs`.

fn main() {
    // `verus_keep_ghost` is set by the Verus verifier, never by rustc; declare
    // it so normal builds don't warn about the `#[cfg(verus_keep_ghost)]` on the
    // proofs module.
    println!("cargo::rustc-check-cfg=cfg(verus_keep_ghost)");
    // Emit rebuild trigger for the proof file.
    println!("cargo:rerun-if-changed=src/proofs.rs");
    println!("cargo:rerun-if-env-changed=DOOM_VERIFY");

    if std::env::var("DOOM_VERIFY").as_deref() == Ok("1") {
        verify_proofs();
    }
}

fn verify_proofs() {
    // Locate verus binary.
    let verus = if cfg!(windows) {
        r"C:\Users\markm\verus\verus.exe"
    } else {
        "verus"
    };

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let proof_file = format!("{manifest_dir}/src/proofs.rs");

    let status = std::process::Command::new(verus)
        .args(["--crate-type", "lib", &proof_file])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:warning=Verus proofs verified successfully.");
        }
        Ok(s) => {
            panic!("Verus verification failed (exit code: {s}). Fix proofs in src/proofs.rs.");
        }
        Err(e) => {
            // Verus not installed: warn but don't fail the build.
            println!("cargo:warning=Verus not found ({e}). Skipping formal verification.");
            println!(
                "cargo:warning=Install verus at C:\\Users\\markm\\verus\\verus.exe to enable."
            );
        }
    }
}
