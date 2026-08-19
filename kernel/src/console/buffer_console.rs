// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2022-2023 Andre Richter <andre.o.richter@gmail.com>

//! A console that retains output produced before the real console is available.

use super::interface;
use crate::synchronization::{self, InitStateLock};

const BUFFER_SIZE: usize = 64 * 1024;

struct BufferConsoleInner {
    buffer: [u8; BUFFER_SIZE],
    len: usize,
    drained: bool,
}

pub struct BufferConsole {
    inner: InitStateLock<BufferConsoleInner>,
}

pub static BUFFER_CONSOLE: BufferConsole = BufferConsole {
    inner: InitStateLock::new(BufferConsoleInner {
        // A zero initializer keeps the storage in BSS instead of inflating the kernel image.
        buffer: [0; BUFFER_SIZE],
        len: 0,
        drained: false,
    }),
};

use synchronization::interface::ReadWriteEx;

impl BufferConsole {
    fn write_byte(&self, byte: u8) {
        self.inner.write(|inner| {
            if !inner.drained && inner.len < inner.buffer.len() {
                inner.buffer[inner.len] = byte;
                inner.len += 1;
            }
        });
    }

    /// Replay all retained output to `destination` exactly once.
    pub fn drain_to(&self, destination: &dyn interface::RawConsole) {
        self.inner.write(|inner| {
            if inner.drained {
                return;
            }

            destination.write_array(&inner.buffer[..inner.len]);
            destination.flush();
            inner.drained = true;
        });
    }
}

impl interface::RawWrite for BufferConsole {
    fn write_byte(&self, byte: u8) {
        self.write_byte(byte);
    }

    fn flush(&self) {}
}

impl interface::RawRead for BufferConsole {
    fn read_byte(&self) -> u8 {
        b' '
    }

    fn clear_rx(&self) {}
}

impl interface::Statistics for BufferConsole {
    fn bytes_written(&self) -> usize {
        self.inner.read(|inner| inner.len)
    }

    fn bytes_read(&self) -> usize {
        0
    }
}

impl interface::RawConsole for BufferConsole {}
