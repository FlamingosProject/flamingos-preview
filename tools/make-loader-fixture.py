"""Write a static AArch64 ELF fixture with code, initialized data, and BSS."""

import pathlib
import struct

output = pathlib.Path("target/loader-fixture.elf")
output.parent.mkdir(parents=True, exist_ok=True)
ident = b"\x7fELF\x02\x01\x01" + bytes(9)
header_size = 64
segment_size = 56
code_offset = header_size + 2 * segment_size
data_offset = code_offset + 4
# ELF64 header: ET_EXEC, EM_AARCH64, no section table, two program headers.
header = struct.pack(
    "<16sHHIQQQIHHHHHH",
    ident, 2, 183, 1, 0x10000 + code_offset,
    header_size, 0, 0, header_size, segment_size, 2, 0, 0, 0,
)
# PT_LOAD flags 5 = read/execute, 6 = read/write. Offsets and virtual addresses
# are congruent modulo the kernel's 64 KiB page size; p_paddr is unused.
code = struct.pack("<IIQQQQQQ", 1, 5, 0, 0x10000, 0, data_offset, data_offset, 0x10000)
data = struct.pack("<IIQQQQQQ", 1, 6, data_offset, 0x20000 + data_offset, 0, 4, 4096, 0x10000)
# 0x14000000 encodes AArch64 `b .`; only four of 4096 data bytes are file-backed.
output.write_bytes(header + code + data + struct.pack("<I", 0x14000000) + b"data")
print(output)
