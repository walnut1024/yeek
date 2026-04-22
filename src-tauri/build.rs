#[cfg(feature = "tauri-shell")]
fn main() {
    tauri_build::build()
}

#[cfg(not(feature = "tauri-shell"))]
fn main() {}
