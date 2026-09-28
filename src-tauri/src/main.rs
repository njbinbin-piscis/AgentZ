#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Must run before GTK/WebKit or any worker threads read the environment.
    #[cfg(target_os = "linux")]
    if std::env::args().any(|arg| arg == "--software-rendering") {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        eprintln!("AgentZ: Linux software rendering enabled");
    }
    agentz_desktop_lib::run();
}
