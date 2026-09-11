fn main() {
    println!("cargo:rerun-if-changed=binaries/screen-translate");
    println!("cargo:rerun-if-changed=../addons/screen-translate");
    tauri_build::build()
}
