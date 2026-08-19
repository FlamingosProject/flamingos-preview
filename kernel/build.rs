use std::{env, fs, path::PathBuf};

fn main() {
    let ld_script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bsp/raspberrypi");
    let linker_script = if env::var_os("CARGO_FEATURE_CHAINLOADER").is_some() {
        "chainloader.ld"
    } else {
        "kernel.ld"
    };

    let files = fs::read_dir(&ld_script_path).unwrap();
    files
        .filter_map(Result::ok)
        .filter(|d| {
            if let Some(e) = d.path().extension() {
                e == "ld"
            } else {
                false
            }
        })
        .for_each(|f| println!("cargo:rerun-if-changed={}", f.path().display()));

    println!("cargo:rustc-link-search={}", ld_script_path.display());
    println!("cargo:rustc-link-arg=--script={linker_script}");
}
