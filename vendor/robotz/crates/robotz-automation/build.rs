fn main() {
    #[cfg(target_os = "linux")]
    build_xi_helper();
}

/// Build the `robotz-xi-helper` C program that uses XIWarpPointer for mouse
/// positioning. Needed in VMware+Xorg where `xdotool mousemove` only updates
/// the XTEST slave pointer, not the XInput2 master pointer applications see.
///
/// Failure is non-fatal: `desktop_automation` falls back to `xdotool`.
#[cfg(target_os = "linux")]
fn build_xi_helper() {
    let src = std::path::Path::new("csrc/xi_helpers.c");
    println!("cargo:rerun-if-changed={}", src.display());

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = std::path::Path::new(&out_dir).join("robotz-xi-helper");

    let status = std::process::Command::new("gcc")
        .args([
            src.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
            "-lX11",
            "-lXi",
            "-O2",
        ])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!(
            "WARNING: failed to build robotz-xi-helper (gcc exit {:?}); \
             xdotool fallback will be used for mouse positioning.",
            s.code()
        ),
        Err(e) => eprintln!(
            "WARNING: could not run gcc for xi_helpers.c ({e}); \
             xdotool fallback will be used for mouse positioning."
        ),
    }
}
