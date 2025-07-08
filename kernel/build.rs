use std::{env, fs};

fn main() {
    let ld_script_base = env::var("CARGO_MANIFEST_DIR").unwrap();
    let ld_script_path = format!("{ld_script_base}/src/bsp/raspberrypi");
    println!("cargo:rustc-env=LD_SCRIPT_PATH={ld_script_path}");

    let out_dir = env::var("OUT_DIR").unwrap();
    let full_script = format!("{out_dir}/kernel-full.ld");

    let script = format!("{ld_script_path}/kernel_virt_addr_space_size.ld");
    let mut contents = fs::read_to_string(&script).unwrap();
    contents += ";\n";
    let script = format!("{ld_script_path}/kernel.ld");
    contents += &fs::read_to_string(&script).unwrap();
    fs::write(&full_script, contents).unwrap();

    println!("cargo:rustc-link-arg=-T{full_script}");

    let files = match fs::read_dir(&ld_script_path) {
        Ok(dir) => dir,
        Err(e) => panic!("{ld_script_path}: {e}"),
    };
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
