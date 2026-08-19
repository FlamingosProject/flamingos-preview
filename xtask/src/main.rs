// Build host-side tools, kernel images, and kernel documentation.
// Original by Google Gemini Flash 2.5 2025-10-07

use std::env;
use std::fs;
use std::process::{self, Command, exit};

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
        Some("build") => {
            build(args);
        }
        Some("doc") => doc(&args[2..]),
        _ => {
            println!("usage: cargo xtask build <board> [cargo args]");
            println!("       cargo xtask doc [board] [cargo doc args]");
            exit(1);
        }
    }
}

fn doc(args: &[String]) {
    let (board, args) = match args.first() {
        Some(board) if !board.starts_with('-') => (board.as_str(), &args[1..]),
        _ => ("rpi3", args),
    };
    let bsp = format!("bsp_{}", bsp_family(board));
    let (features, cargo_args) = kernel_cargo_args(args, &bsp);

    run(
        Command::new("cargo")
            .args([
                "doc",
                "--package",
                "mingo",
                "--target",
                "aarch64-unknown-none-softfloat",
                "--release",
                "--no-deps",
                "--document-private-items",
                &features,
            ])
            .args(&cargo_args),
        "ERROR: Failed to document kernel ('mingo' crate)",
    );
}

fn build(args: Vec<String>) {
    if args.len() < 3 {
        println!("usage: cargo xtask build <board>");
        exit(1);
    }
    let cpu = bsp_family(&args[2]);
    let bsp = format!("bsp_{cpu}");
    let (features, cargo_args) = kernel_cargo_args(&args[3..], &bsp);
    let chainloader = features
        .trim_start_matches("--features=")
        .split(',')
        .any(|feature| feature == "chainloader");
    let pid = process::id();
    let copy_path = format!("target/xtask-kernel.{pid}");
    let symbols_path = format!("{copy_path}.symbols");
    let generated_symbols_path = format!("{copy_path}_symbols.rs");

    println!("--- build target");
    run(
        Command::new("cargo")
            .args([
                "build",
                "-p",
                "mingo",
                "--release",
                "--target",
                "aarch64-unknown-none-softfloat",
                &features,
            ])
            .args(&cargo_args),
        "ERROR: Failed to build kernel ('mingo' crate)",
    );

    if !chainloader {
        println!("--- build tools");
        run(
            Command::new("cargo").args([
                "objcopy",
                "-p",
                "translation_table_tool",
                "--release",
                "--",
                "tools/bin/translation_table_tool",
            ]),
            "ERROR: Failed to build translation_table_tool",
        );
        run(
            Command::new("cargo").args([
                "build",
                "-p",
                "kernel_symbols_tool",
                "--release",
                "--bin",
                "kernel-elf-symbol",
            ]),
            "ERROR: Failed to build kernel_symbols_tool",
        );
    }

    println!("--- copy kernel");
    run(
        Command::new("cargo")
            .args([
                "objcopy",
                "-p",
                "mingo",
                "--release",
                "--target",
                "aarch64-unknown-none-softfloat",
                &features,
            ])
            .args(&cargo_args)
            .args(["--", &copy_path]),
        "ERROR: Failed to copy kernel",
    );

    if !chainloader {
        println!("--- fix up kernel");
        run(
            Command::new("tools/bin/translation_table_tool").args([cpu, &copy_path]),
            "ERROR: Failed to fix up kernel",
        );
        run(
            Command::new("make")
                .args(["--no-print-directory", "-f", "kernel_symbols.mk"])
                .env("KERNEL_SYMBOLS_TOOL_PATH", "tools/kernel_symbols_tool")
                .env("TARGET", "aarch64-unknown-none-softfloat")
                .env("KERNEL_SYMBOLS_INPUT_ELF", &copy_path)
                .env("KERNEL_SYMBOLS_OUTPUT_ELF", &symbols_path),
            "ERROR: Failed to add kernel symbols",
        );
    }

    println!("--- acquire fixed-up kernel");
    let output = if chainloader {
        "chainloader8.img"
    } else {
        "kernel8.img"
    };
    let final_elf = if chainloader {
        &copy_path
    } else {
        &symbols_path
    };
    run(
        Command::new("rust-objcopy").args(["--strip-all", "-O", "binary", final_elf, output]),
        "ERROR: Failed to acquire fixed-up kernel",
    );
    fs::remove_file(&copy_path).expect("could not remove kernel copy");
    if !chainloader {
        fs::remove_file(&symbols_path).expect("could not remove kernel symbol copy");
        fs::remove_file(&generated_symbols_path)
            .expect("could not remove generated kernel symbols");
    }

    println!("--- build complete, kernel in {output}");
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

        assert_eq!(features, "--features=bsp_rpi3,test_build,boot_trace,extra");
        assert_eq!(forwarded, ["--locked"]);
    }
}
