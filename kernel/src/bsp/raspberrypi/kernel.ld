/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>
 */

/* INCLUDE kernel_virt_addr_space_size.ld; */

PAGE_SIZE = 64K;
PAGE_MASK = PAGE_SIZE - 1;

/* This chapter keeps the kernel identity-mapped; Chapter 16 moves it high. */
__kernel_virt_start_addr = 0x80000;

/* Preserve low memory used by firmware to park secondary cores. */
__rpi_phys_boot_stack_start = 64K;

/* The physical address at which the the kernel binary will be loaded by the Raspberry's firmware */
__rpi_phys_binary_load_addr = 0x80000;

/* The stage-1 loader reserves the source ELF for the life of the kernel. */
__boot_elf_phys_start = 0x04000000;
__boot_elf_virt_start = __boot_elf_phys_start;
__boot_elf_virt_end_exclusive = __boot_elf_virt_start + 0x02000000;

/* Symbols expected by the unwinding crate's fde-static finder. */
__executable_start = ADDR(.text);
__etext = ADDR(.eh_frame) + SIZEOF(.eh_frame);
__eh_frame = ADDR(.eh_frame);

ENTRY(__rpi_phys_binary_load_addr)

/* Flags:
 *     4 == R
 *     5 == RX
 *     6 == RW
 *
 * Stage 1 copies PT_LOAD file bytes and zeros the corresponding memory tails.
 */
PHDRS
{
    segment_code            PT_LOAD FLAGS(5);
    segment_data            PT_LOAD FLAGS(6);
    segment_boot_core_stack PT_LOAD FLAGS(6);
}

SECTIONS
{
    /* Preserve firmware memory; this chapter still uses identity mappings. */
    . = __rpi_phys_boot_stack_start;
    .boot_core_stack (NOLOAD) : AT(__rpi_phys_boot_stack_start)
    {
        __boot_core_stack_start = .;
        . += __rpi_phys_binary_load_addr - __rpi_phys_boot_stack_start;
        __boot_core_stack_end_exclusive = .;
    } :segment_boot_core_stack

    . =  __kernel_virt_start_addr;

    ASSERT((. & PAGE_MASK) == 0, "Start of address space is not page aligned")

    /***********************************************************************************************
    * Code + RO Data + Global Offset Table
    ***********************************************************************************************/
    __code_start = .;
    .text : AT(__rpi_phys_binary_load_addr)
    {
        KEEP(*(.text._start))
        *(.text._start_arguments) /* Constants (or statics in Rust speak) read by _start(). */
        *(.text._start_rust)      /* The Rust entry point */
        *(.text*)                 /* Everything else */

        /* Keep in sync with BootMemoryLayout. Addresses are 64-bit words. */
        . = ALIGN(8);
        __boot_memory_layout = .;
        QUAD(__code_start)
        QUAD(__code_end_exclusive)
        QUAD(LOADADDR(.text))
        QUAD(__data_start)
        QUAD(__data_end_exclusive)
        QUAD(LOADADDR(.data))
        QUAD(__boot_core_stack_start)
        QUAD(__boot_core_stack_end_exclusive)
        QUAD(LOADADDR(.boot_core_stack))
        QUAD(__boot_elf_virt_start)
        QUAD(__boot_elf_virt_end_exclusive)
        QUAD(__boot_elf_phys_start)
    } :segment_code

    .rodata         : ALIGN(8) { *(.rodata*) } :segment_code

    /* Unwind tables must be in a mapped, readable segment. */
    .eh_frame_hdr : ALIGN(8) { *(.eh_frame_hdr*) } :segment_code
    .eh_frame     : ALIGN(8) { *(.eh_frame*) } :segment_code

    . = ALIGN(PAGE_SIZE);
    __code_end_exclusive = .;

    /***********************************************************************************************
    * Data + BSS
    ***********************************************************************************************/
    __data_start = .;
    .data : AT(__rpi_phys_binary_load_addr + (__data_start - __code_start))
    {
        *(.data*)
    } :segment_data

    /* Section is zeroed in pairs of u64. Align start and end to 16 bytes */
    .bss (NOLOAD) : ALIGN(16)
    {
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end_exclusive = .;
    } :segment_data

    __phys_bss_start = LOADADDR(.bss);
    __phys_bss_end_exclusive = LOADADDR(.bss) + SIZEOF(.bss);
    __phys_kernel_tables = LOADADDR(.bss) + (KERNEL_TABLES - ADDR(.bss));
    __phys_device_tree_buffer = LOADADDR(.bss) + (DEVICE_TREE_BUFFER - ADDR(.bss));
    __phys_boot_elf_size = LOADADDR(.bss) + (BOOT_ELF_SIZE - ADDR(.bss));

    ASSERT((__phys_kernel_tables & PAGE_MASK) == 0, "Kernel tables are not page aligned")

    . = ALIGN(PAGE_SIZE);
    __data_end_exclusive = .;

    ASSERT(LOADADDR(.bss) + SIZEOF(.bss) <= 0x02000000,
           "Kernel load regions overlap relocated bootstrap")

    /***********************************************************************************************
    * MMIO Remap Reserved
    ***********************************************************************************************/
    __mmio_remap_start = .;
    . += 8 * 1024 * 1024;
    __mmio_remap_end_exclusive = .;

    ASSERT((. & PAGE_MASK) == 0, "MMIO remap reservation is not page aligned")

    /***********************************************************************************************
    * Guard Page
    ***********************************************************************************************/
    . += PAGE_SIZE;

    /***********************************************************************************************
    * Boot Core Stack
    ***********************************************************************************************/
    __phys_boot_core_stack_end_exclusive = LOADADDR(.boot_core_stack) + SIZEOF(.boot_core_stack);

    ASSERT((. & PAGE_MASK) == 0, "End of boot core stack is not page aligned")
    ASSERT(. <= __boot_elf_virt_start, "Kernel layout overlaps retained ELF window")

    /***********************************************************************************************
    * Misc
    ***********************************************************************************************/
    .got : { *(.got*) }
    ASSERT(SIZEOF(.got) == 0, "Relocation support not expected")

    /DISCARD/ : { *(.comment*) }
}
