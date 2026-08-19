use std::{env, fs, path::PathBuf};

fn git_output(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn export_git_revision() {
    let mut revision = git_output(&["rev-parse", "--short=8", "HEAD"])
        .filter(|revision| !revision.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());

    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false);
    if dirty {
        revision.push_str("-dirty");
    }

    println!("cargo:rustc-env=FLAMINGOS_REVISION={revision}");

    // A commit changes the worktree's HEAD file without changing any kernel source. Watch both HEAD
    // and its branch ref so a rebuild refreshes the embedded revision in linked and detached states.
    if let Some(head) = git_output(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }
    if let Some(reference) = git_output(&["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git_output(&["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={path}");
    }
}

fn main() {
    export_git_revision();

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
