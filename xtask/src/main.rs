// Build host-side tools, kernel images, and kernel documentation.
// Original by Google Gemini Flash 2.5 2025-10-07

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{self, Command, exit};

use kernel_image::{ELF_STAGING_SIZE, KernelImage};

const KERNEL_TARGET: &str = "aarch64-unknown-none-softfloat";

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn kernel_cargo() -> Command {
    let mut command = Command::new("cargo");
    if let Some(flags) = env::var_os("FLAMINGOS_KERNEL_RUSTFLAGS") {
        command.env(
            "CARGO_TARGET_AARCH64_UNKNOWN_NONE_SOFTFLOAT_RUSTFLAGS",
            flags,
        );
    }
    command
}

fn kernel_cargo_args(args: &[String], bsp: &str) -> (String, Vec<String>) {
    let mut features = vec![bsp.to_owned()];
    let mut forwarded = Vec::new();
    let mut i = 0;
    let mut sd_card = false;

    while i < args.len() {
        if args[i] == "--sd-card" {
            sd_card = true;
            i += 1;
            continue;
        }
        let feature_value = if args[i] == "--features" {
            i += 1;
            let value = args.get(i).unwrap_or_else(|| {
                eprintln!("--features requires a value");
                exit(1);
            });
            Some(value.as_str())
        } else {
            args[i].strip_prefix("--features=")
        };

        if let Some(value) = feature_value {
            for feature in value
                .split([',', ' '])
                .filter(|feature| !feature.is_empty())
            {
                if !features.iter().any(|existing| existing == feature) {
                    features.push(feature.to_owned());
                }
            }
        } else {
            forwarded.push(args[i].clone());
        }

        i += 1;
    }

    if sd_card && features.iter().any(|feature| feature == "embedded-fs") {
        eprintln!("--sd-card cannot be combined with embedded-fs");
        exit(1);
    }
    if !sd_card
        && !features
            .iter()
            .any(|feature| feature == "chainloader" || feature == "embedded-fs")
    {
        features.push("embedded-fs".to_owned());
    }
    (format!("--features={}", features.join(",")), forwarded)
}

fn run(command: &mut Command, error: &str) {
    let status = command.status().expect(error);
    if !status.success() {
        eprintln!("{error}: {status}");
        exit(status.code().unwrap_or(1));
    }
}

fn bsp_family(board: &str) -> &'static str {
    match board {
        "rpiz2" | "rpi3" => "rpi3",
        "rpi4" => "rpi4",
        _ => {
            eprintln!("board must be one of rpiz2,rpi3,rpi4");
            exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("build") => build(args.get(2).unwrap_or_else(|| usage()), &args[3..]),
        Some("doc") => doc(&args[2..]),
        Some("test-init") => build_test_init(args.get(2).map(String::as_str).unwrap_or("rpi3")),
        Some("boot-image") => {
            build_boot_image(args.get(2).map(String::as_str).unwrap_or("rpi3"), false)
        }
        Some("package") => match &args[2..] {
            [board, elf, image] => package(board, Path::new(elf), Path::new(image), None),
            [board, elf, image, option, bootstrap_elf] if option == "--bootstrap-elf" => {
                package(
                    board,
                    Path::new(elf),
                    Path::new(image),
                    Some(Path::new(bootstrap_elf)),
                );
            }
            _ => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("usage: cargo xtask build <board> [--out-dir <dir>] [--sd-card] [cargo args]");
    eprintln!(
        "       cargo xtask package <board> <kernel-elf> <output-image> [--bootstrap-elf <path>]"
    );
    eprintln!("       cargo xtask doc [board] [--sd-card] [cargo doc args]");
    eprintln!("       cargo xtask test-init [board]");
    eprintln!("       cargo xtask boot-image [board]");
    exit(1);
}

fn build_test_init(board: &str) {
    build_boot_image(board, true);
}

fn build_boot_image(board: &str, test: bool) {
    let family = bsp_family(board);
    let root = repo_root();
    let build_directory = if test { "init-qemu" } else { "init-embedded" };
    let target = root.join("target").join(build_directory).join(family);
    let features = if test {
        format!("bsp_{family},qemu_test")
    } else {
        format!("bsp_{family}")
    };
    run(
        Command::new("cargo")
            .current_dir(root.join("init"))
            .env("CARGO_TARGET_DIR", &target)
            .args(["build", "--locked", "--release", "--features", &features]),
        "ERROR: Failed to build init for the boot filesystem",
    );
    let executable = fs::read(target.join("aarch64-unknown-none-softfloat/release/init"))
        .expect("ERROR: Failed to read init ELF");
    let image = boot_image::build(&executable).expect("ERROR: Failed to build sparse filesystem");
    let logical_size = boot_image::ReadOnlyDisk::decode(&image).unwrap().byte_len();
    let output = root
        .join("target")
        .join(if test { "bootfs-qemu" } else { "bootfs" })
        .join(family)
        .join("init.sparse");
    fs::create_dir_all(output.parent().unwrap())
        .expect("ERROR: Failed to create boot-image directory");
    if fs::read(&output).ok().as_deref() != Some(image.as_slice()) {
        fs::write(&output, &image).expect("ERROR: Failed to write boot image");
    }
    println!(
        "Boot filesystem: {} logical bytes, {} payload bytes -> {}",
        logical_size,
        image.len(),
        output.display()
    );
}

fn doc(args: &[String]) {
    let (board, args) = match args.first() {
        Some(board) if !board.starts_with('-') => (board.as_str(), &args[1..]),
        _ => ("rpi3", args),
    };
    let bsp = format!("bsp_{}", bsp_family(board));
    let (features, cargo_args) = kernel_cargo_args(args, &bsp);

    if features
        .trim_start_matches("--features=")
        .split(',')
        .any(|feature| feature == "embedded-fs")
    {
        build_boot_image(board, false);
    }

    run(
        kernel_cargo()
            .args([
                "doc",
                "--package",
                "mingo",
                "--target",
                KERNEL_TARGET,
                "--release",
                "--no-deps",
                "--document-private-items",
                &features,
            ])
            .args(&cargo_args),
        "ERROR: Failed to document kernel ('mingo' crate)",
    );
}

fn build(board: &str, args: &[String]) {
    let bsp = format!("bsp_{}", bsp_family(board));
    let mut output_dir = PathBuf::from(".");
    let mut build_args = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--out-dir" {
            output_dir = PathBuf::from(args.next().unwrap_or_else(|| usage()));
        } else {
            build_args.push(arg.clone());
        }
    }
    let (features, cargo_args) = kernel_cargo_args(&build_args, &bsp);
    let has_feature = |name| {
        features
            .trim_start_matches("--features=")
            .split(',')
            .any(|feature| feature == name)
    };
    let chainloader = has_feature("chainloader");
    if has_feature("embedded-kernel") {
        eprintln!("use the package command to embed a kernel ELF in a bootstrap");
        exit(1);
    }
    if chainloader && has_feature("embedded-fs") {
        eprintln!("the chainloader cannot include the boot filesystem");
        exit(1);
    }
    if has_feature("test_build") {
        build_test_init(board);
    } else if has_feature("embedded-fs") {
        build_boot_image(board, false);
    }

    fs::create_dir_all(&output_dir).expect("ERROR: Failed to create image output directory");
    let mut objcopy = kernel_cargo();
    objcopy
        .args([
            "objcopy",
            "-p",
            "mingo",
            "--bin",
            "kernel",
            "--release",
            "--target",
            KERNEL_TARGET,
            &features,
        ])
        .args(&cargo_args)
        .arg("--");

    if chainloader {
        let image = output_dir.join("chainloader8.img");
        run(
            objcopy.args(["--strip-all", "-O", "binary"]).arg(&image),
            "ERROR: Failed to build the serial chainloader",
        );
        println!("Serial chainloader: {}", image.display());
    } else {
        let elf = output_dir.join("kernel.elf");
        let copy = output_dir.join(format!("kernel.elf.{}", process::id()));
        run(
            objcopy
                .env("CARGO_PROFILE_RELEASE_STRIP", "none")
                .arg("--strip-debug")
                .arg(&copy),
            "ERROR: Failed to build the kernel ELF",
        );
        fs::rename(&copy, &elf).expect("ERROR: Failed to save the kernel ELF");
        println!("Kernel ELF: {}", elf.display());
        package(board, &elf, &output_dir.join("kernel8.img"), None);
    }
}

fn package(board: &str, kernel_elf: &Path, image: &Path, bootstrap_elf: Option<&Path>) {
    let family = bsp_family(board);
    let kernel_elf = fs::canonicalize(kernel_elf).expect("ERROR: Failed to resolve the kernel ELF");
    if bootstrap_elf == Some(image)
        || std::iter::once(image)
            .chain(bootstrap_elf)
            .any(|output| fs::canonicalize(output).ok().as_ref() == Some(&kernel_elf))
    {
        eprintln!("ERROR: the kernel ELF and package outputs must use separate paths");
        exit(1);
    }
    let mut bytes = Vec::new();
    File::open(&kernel_elf)
        .expect("ERROR: Failed to open the kernel ELF")
        .take(ELF_STAGING_SIZE + 1)
        .read_to_end(&mut bytes)
        .expect("ERROR: Failed to read the kernel ELF");
    if bytes.len() as u64 > ELF_STAGING_SIZE {
        eprintln!(
            "ERROR: {} exceeds the {ELF_STAGING_SIZE}-byte ELF staging area",
            kernel_elf.display()
        );
        exit(1);
    }
    if let Err(error) = KernelImage::parse(&bytes).and_then(|kernel| kernel.symbols()) {
        eprintln!(
            "ERROR: Invalid kernel ELF {}: {error:?}",
            kernel_elf.display()
        );
        exit(1);
    }

    let target = repo_root().join("target/elf-bootstrap").join(family);
    fs::create_dir_all(&target).expect("ERROR: Failed to create the bootstrap build directory");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(target.join("package.lock"))
        .expect("ERROR: Failed to open the bootstrap build lock");
    // Keep the shared Cargo artifact stable until all output files are copied.
    lock.lock()
        .expect("ERROR: Failed to lock the bootstrap build directory");

    let input = target.join("kernel.elf");
    // Embed the checked bytes and let Cargo reuse an unchanged snapshot.
    if fs::read(&input).ok().as_deref() != Some(bytes.as_slice()) {
        fs::write(&input, bytes).expect("ERROR: Failed to stage the kernel ELF for packaging");
    }
    run(
        kernel_cargo()
            .current_dir(repo_root())
            .env("FLAMINGOS_KERNEL_ELF", &input)
            .env("CARGO_TARGET_DIR", &target)
            .args([
                "build",
                "--locked",
                "-p",
                "mingo",
                "--bin",
                "kernel",
                "--release",
                "--target",
                KERNEL_TARGET,
                "--features",
            ])
            .arg(format!("bsp_{family},chainloader,embedded-kernel")),
        "ERROR: Failed to build the firmware bootstrap",
    );
    let artifact = target.join(KERNEL_TARGET).join("release/kernel");
    run(
        Command::new("rust-objcopy")
            .args(["--strip-all", "-O", "binary"])
            .arg(&artifact)
            .arg(image),
        "ERROR: Failed to create the firmware image",
    );
    if let Some(bootstrap_elf) = bootstrap_elf {
        fs::copy(&artifact, bootstrap_elf).expect("ERROR: Failed to save the bootstrap ELF");
    }
    println!("Firmware image: {}", image.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_feature_arguments_and_preserves_other_arguments() {
        let args = [
            "--locked".to_owned(),
            "--features=test_build,boot_trace".to_owned(),
            "--features".to_owned(),
            "boot_trace extra".to_owned(),
        ];

        let (features, forwarded) = kernel_cargo_args(&args, "bsp_rpi3");

        assert_eq!(
            features,
            "--features=bsp_rpi3,test_build,boot_trace,extra,embedded-fs"
        );
        assert_eq!(forwarded, ["--locked"]);
    }

    #[test]
    fn defaults_to_embedded_but_sd_card_is_an_explicit_override() {
        let (features, forwarded) = kernel_cargo_args(&[], "bsp_rpi3");
        assert_eq!(features, "--features=bsp_rpi3,embedded-fs");
        assert!(forwarded.is_empty());
        let (features, forwarded) =
            kernel_cargo_args(&["--sd-card".to_owned(), "--locked".to_owned()], "bsp_rpi4");
        assert_eq!(features, "--features=bsp_rpi4");
        assert_eq!(forwarded, ["--locked"]);
    }

    #[test]
    fn chainloader_keeps_its_storage_independent_bootstrap() {
        let (features, _) = kernel_cargo_args(&["--features=chainloader".to_owned()], "bsp_rpi3");
        assert_eq!(features, "--features=bsp_rpi3,chainloader");
    }
}
