// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2026 Bart Massey

//! Cargo target runner for harness-free kernel tests.

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEFAULT_TIMEOUT_SECONDS: u64 = 10;

fn required_env(name: &str) -> Result<OsString, String> {
    env::var_os(name).ok_or_else(|| format!("environment variable {name} is not set"))
}

fn image_path(elf: &Path) -> PathBuf {
    let mut image = elf.as_os_str().to_owned();
    image.push(".img");
    image.into()
}

fn make_image(elf: &Path, image: &Path) -> Result<(), String> {
    let objcopy =
        env::var_os("KERNEL_TEST_OBJCOPY").unwrap_or_else(|| OsString::from("rust-objcopy"));
    let status = Command::new(&objcopy)
        .args(["--strip-all", "-O", "binary"])
        .arg(elf)
        .arg(image)
        .status()
        .map_err(|err| format!("failed to execute {objcopy:?}: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{objcopy:?} failed with {status}"))
    }
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                child
                    .kill()
                    .map_err(|err| format!("failed to terminate QEMU: {err}"))?;
                child
                    .wait()
                    .map_err(|err| format!("failed to reap QEMU: {err}"))?;
                return Err(format!(
                    "QEMU timed out after {} seconds",
                    timeout.as_secs()
                ));
            }
            Err(err) => return Err(format!("failed while waiting for QEMU: {err}")),
        }
    }
}

fn run() -> Result<(), String> {
    let elf = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: kernel_test_runner <test-elf>".to_owned())?;
    let image = image_path(&elf);

    make_image(&elf, &image)?;

    let qemu = required_env("KERNEL_TEST_QEMU")?;
    let qemu_args = env::var("KERNEL_TEST_QEMU_ARGS").unwrap_or_default();
    let timeout = env::var("KERNEL_TEST_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS);

    let mut child = Command::new(&qemu)
        .args(qemu_args.split_ascii_whitespace())
        .arg("-kernel")
        .arg(&image)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| format!("failed to execute {qemu:?}: {err}"))?;

    let result = wait_with_timeout(&mut child, Duration::from_secs(timeout)).and_then(|status| {
        status
            .success()
            .then_some(())
            .ok_or_else(|| format!("QEMU failed with {status}"))
    });
    let _ = fs::remove_file(image);
    result
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kernel test failed: {err}");
            ExitCode::FAILURE
        }
    }
}
