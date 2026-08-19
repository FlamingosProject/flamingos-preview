use std::{env, fs, path::PathBuf};

fn main() {
    let ld_script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bsp/raspberrypi");
    println!(
        "cargo:rustc-env=LD_SCRIPT_PATH={}",
        ld_script_path.display()
    );
    let out_dir = env::var("OUT_DIR").unwrap();
    let linker_script = if env::var_os("CARGO_FEATURE_CHAINLOADER").is_some() {
        ld_script_path.join("chainloader.ld")
    } else {
        let full_script = PathBuf::from(out_dir).join("kernel-full.ld");
        let mut contents =
            fs::read_to_string(ld_script_path.join("kernel_virt_addr_space_size.ld")).unwrap();
        contents += ";\n";
        contents += &fs::read_to_string(ld_script_path.join("kernel.ld")).unwrap();
        fs::write(&full_script, contents).unwrap();
        full_script
    };

    println!("cargo:rustc-link-arg=-T{}", linker_script.display());

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
}
