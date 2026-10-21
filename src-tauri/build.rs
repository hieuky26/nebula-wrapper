fn main() {
    println!("cargo:rerun-if-changed=../nebula.config.json");
    tauri_build::build()
}
