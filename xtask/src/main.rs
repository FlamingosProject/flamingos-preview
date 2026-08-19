// Build host-side tools, kernel images, and kernel documentation.
// Original by Google Gemini Flash 2.5 2025-10-07

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command, exit};

use kernel_image::{ELF_STAGING_SIZE, KERNEL_LOAD_END, KERNEL_LOAD_START, KernelImage};

const KERNEL_TARGET: &str = "aarch64-unknown-none-softfloat";
const FLAT_ELF_OFFSET: usize = 0x1000;
const FLAT_IMAGE_MAX_SIZE: u64 = {
    let load_window = KERNEL_LOAD_END - KERNEL_LOAD_START;
    let staging_space = ELF_STAGING_SIZE - FLAT_ELF_OFFSET as u64;
    if load_window < staging_space {
        load_window
    } else {
        staging_space
    }
};

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

    while i < args.len() {
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
        Some("wrap-flat") => match &args[2..] {
            [raw, elf] => wrap_flat(Path::new(raw), Path::new(elf)),
            _ => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("usage: cargo xtask build <board> [--out-dir <dir>] [cargo args]");
    eprintln!(
        "       cargo xtask package <board> <kernel-elf> <output-image> [--bootstrap-elf <path>]"
    );
    eprintln!("       cargo xtask wrap-flat <raw-image> <output-elf>");
    eprintln!("       cargo xtask doc [board] [cargo doc args]");
    exit(1);
}

fn doc(args: &[String]) {
    let (board, args) = match args.first() {
        Some(board) if !board.starts_with('-') => (board.as_str(), &args[1..]),
        _ => ("rpi3", args),
    };
    let bsp = format!("bsp_{}", bsp_family(board));
    let (features, cargo_args) = kernel_cargo_args(args, &bsp);

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

/// Build a sectionless `AArch64` executable for a flat image linked at
/// the firmware entry. The output must fit the chainloader's staging area.
fn flat_elf(raw: &[u8]) -> Result<Vec<u8>, String> {
    const ELF_HEADER_SIZE: u16 = 64;
    const PROGRAM_HEADER_SIZE: u16 = 56;

    if !(4..=FLAT_IMAGE_MAX_SIZE).contains(&(raw.len() as u64)) {
        return Err(format!(
            "flat image must contain between 4 and {FLAT_IMAGE_MAX_SIZE} bytes"
        ));
    }

    let mut elf = Vec::with_capacity(FLAT_ELF_OFFSET + raw.len());
    elf.resize(FLAT_ELF_OFFSET, 0);
    let (header, program_header) = elf.split_at_mut(usize::from(ELF_HEADER_SIZE));
    header[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    header[16..18].copy_from_slice(&2_u16.to_le_bytes()); // ET_EXEC
    header[18..20].copy_from_slice(&183_u16.to_le_bytes()); // EM_AARCH64
    header[20..24].copy_from_slice(&1_u32.to_le_bytes()); // EV_CURRENT
    header[24..32].copy_from_slice(&KERNEL_LOAD_START.to_le_bytes()); // e_entry
    header[32..40].copy_from_slice(&u64::from(ELF_HEADER_SIZE).to_le_bytes()); // e_phoff
    header[52..54].copy_from_slice(&ELF_HEADER_SIZE.to_le_bytes());
    header[54..56].copy_from_slice(&PROGRAM_HEADER_SIZE.to_le_bytes());
    header[56..58].copy_from_slice(&1_u16.to_le_bytes()); // e_phnum

    let offset = FLAT_ELF_OFFSET as u64;
    let size = raw.len() as u64;
    program_header[..4].copy_from_slice(&1_u32.to_le_bytes()); // PT_LOAD
    program_header[4..8].copy_from_slice(&5_u32.to_le_bytes()); // PF_R | PF_X
    program_header[8..16].copy_from_slice(&offset.to_le_bytes()); // p_offset
    program_header[16..24].copy_from_slice(&KERNEL_LOAD_START.to_le_bytes()); // p_vaddr
    program_header[24..32].copy_from_slice(&KERNEL_LOAD_START.to_le_bytes()); // p_paddr
    program_header[32..40].copy_from_slice(&size.to_le_bytes()); // p_filesz
    program_header[40..48].copy_from_slice(&size.to_le_bytes()); // p_memsz
    program_header[48..56].copy_from_slice(&offset.to_le_bytes()); // p_align

    elf.extend_from_slice(raw);
    KernelImage::parse(&elf).map_err(|error| format!("invalid generated ELF: {error:?}"))?;
    Ok(elf)
}

/// Snapshot the helper before writing the ELF and refuse to replace an input alias.
fn wrap_flat(raw: &Path, elf: &Path) {
    let raw = fs::canonicalize(raw).expect("ERROR: Failed to resolve the flat image");
    let input = File::open(&raw).expect("ERROR: Failed to open the flat image");
    if fs::canonicalize(elf).ok().as_ref() == Some(&raw) || is_hard_link(&input, elf) {
        eprintln!("ERROR: the flat image and ELF output must use separate paths");
        exit(1);
    }

    let mut bytes = Vec::new();
    input
        .take(FLAT_IMAGE_MAX_SIZE + 1)
        .read_to_end(&mut bytes)
        .expect("ERROR: Failed to read the flat image");
    let wrapped = flat_elf(&bytes).unwrap_or_else(|error| {
        eprintln!("ERROR: Cannot wrap {}: {error}", raw.display());
        exit(1);
    });

    if let Some(parent) = elf.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent).expect("ERROR: Failed to create the ELF output directory");
    }
    let temporary = elf.with_extension(format!("elf.{}.tmp", process::id()));
    let mut output = File::create_new(&temporary).expect("ERROR: Failed to create temporary ELF");
    let written = output.write_all(&wrapped);
    drop(output);
    if let Err(error) = written.and_then(|()| fs::rename(&temporary, elf)) {
        let _ = fs::remove_file(&temporary);
        eprintln!("ERROR: Failed to save {}: {error}", elf.display());
        exit(1);
    }
}

/// Compare file identities to catch hard links that canonical paths cannot detect.
#[cfg(unix)]
fn is_hard_link(input: &File, output: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    input.metadata().is_ok_and(|input| {
        fs::metadata(output)
            .is_ok_and(|output| input.dev() == output.dev() && input.ino() == output.ino())
    })
}

#[cfg(not(unix))]
fn is_hard_link(_input: &File, _output: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_helper_wraps_as_one_executable_load() {
        let raw = [0xa1, 0x00, 0x38, 0xd5, 0x21, 0x04, 0x40, 0x92];
        let elf = flat_elf(&raw).unwrap();
        assert_eq!(&elf[0x1000..], &raw);
        let image = KernelImage::parse(&elf).unwrap();
        assert_eq!(image.entry(), KERNEL_LOAD_START);
        assert!(image.symbols().unwrap().is_none());
        let mut segments = image.segments();
        let load = segments.next().unwrap();
        assert_eq!(load.physical_address, KERNEL_LOAD_START);
        assert_eq!(load.virtual_address, KERNEL_LOAD_START);
        assert_eq!(load.flags, 5);
        assert_eq!(load.memory_size, raw.len() as u64);
        assert_eq!(load.file_bytes, raw);
        assert!(segments.next().is_none());
    }

    #[test]
    fn flat_helper_rejects_incomplete_entry_and_out_of_bounds_image() {
        assert!(flat_elf(&[]).is_err());
        assert!(flat_elf(&[0; 3]).is_err());
        assert!(flat_elf(&[0; 4]).is_ok());
        let max_size = usize::try_from(FLAT_IMAGE_MAX_SIZE).unwrap();
        assert!(flat_elf(&vec![0; max_size + 1]).is_err());
    }

    #[test]
    fn merges_feature_arguments_and_preserves_other_arguments() {
        let args = [
            "--locked".to_owned(),
            "--features=test_build,boot_trace".to_owned(),
            "--features".to_owned(),
            "boot_trace extra".to_owned(),
        ];

        let (features, forwarded) = kernel_cargo_args(&args, "bsp_rpi3");

        assert_eq!(features, "--features=bsp_rpi3,test_build,boot_trace,extra");
        assert_eq!(forwarded, ["--locked"]);
    }
}
