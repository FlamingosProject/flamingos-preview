// Build host-side tools and kernel.
// Original by Google Gemini Flash 2.5 2025-10-07

use std::env;
use std::fs;
use std::process::{self, Command, exit}; 

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("build") => {
            build(args);
        }
        _ => {
            println!("usage: cargo xtask build <board>");
            exit(1);
        }
    }
}

fn build(args: Vec<String>) {
    if args.len() != 3 {
        println!("usage: cargo xtask build <board>");
        exit(1);
    }
    let cpu = match args[2].as_str() {
        "rpiz2" => "rpi3",
        "rpi3" => "rpi3",
        "rpi4" => "rpi4",
        _ => {
            println!("usage: board must be one of rpiz2,rpi3,rpi4");
            exit(1);
        }
    };
    let bsp = format!("bsp_{cpu}");
    let features = format!("--features={bsp}");
    let pid = process::id();
    let copy_path = format!("/tmp/flamingo-kernel.{pid}");

    println!("--- build target");
    Command::new("cargo")
        .args([
            "build", "-p", "mingo", "--release",
            "--target", "aarch64-unknown-none-softfloat",
            &features,
        ]) 
        .status()
        .expect("ERROR: Failed to build kernel ('mingo' crate)");

    println!("--- build tools");
    Command::new("cargo")
        .args([
            "objcopy", "-p", "translation_table_tool", "--release",
            "--", "tools/bin/translation_table_tool",
        ])
        .status()
        .expect("ERROR: Failed to build tools ('tools/*' crates)");

    println!("--- copy kernel");
    Command::new("cargo")
        .args([
            "objcopy", "-p", "mingo", "--release",
            "--target", "aarch64-unknown-none-softfloat",
            &features,
            "--", &copy_path,
        ])
        .status()
        .expect("ERROR: Failed to copy kernel");

    println!("--- fix up kernel");
    Command::new("tools/bin/translation_table_tool")
        .args([cpu, &copy_path])
        .status()
        .expect("ERROR: Failed to fix up kernel");

    println!("--- acquire fixed-up kernel");
    Command::new("rust-objcopy")
        .args([
            "--strip-all", "-O", "binary",
            &copy_path, "kernel8.img",
        ])
        .status()
        .expect("ERROR: Failed to fix up kernel");
    fs::remove_file(&copy_path).expect("could not remove kernel copy");
    
    println!("--- build complete, kernel in kernel8.img");
}
