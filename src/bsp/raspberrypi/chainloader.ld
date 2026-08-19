/* SPDX-License-Identifier: MIT OR Apache-2.0 */

__rpi_phys_binary_load_addr = 0x80000;
ENTRY(__rpi_phys_binary_load_addr)

PHDRS
{
    segment_boot_core_stack PT_LOAD FLAGS(6);
    segment_code            PT_LOAD FLAGS(5);
    segment_data            PT_LOAD FLAGS(6);
}

SECTIONS
{
    /* Relocate the loader to 32 MiB, away from the payload at 0x80000. */
    . = 0x2000000;

    .boot_core_stack (NOLOAD) :
    {
        . += __rpi_phys_binary_load_addr;
        __boot_core_stack_end_exclusive = .;
    } :segment_boot_core_stack

    __binary_nonzero_start = .;
    .text :
    {
        KEEP(*(.text._start))
        *(.text._start_arguments)
        *(.text._start_rust)
        *(.text*)
    } :segment_code
    .rodata : ALIGN(8) { *(.rodata*) } :segment_code
    .data : { *(.data*) } :segment_data

    . = ALIGN(8);
    __binary_nonzero_end_exclusive = .;

    .bss (NOLOAD) : ALIGN(16)
    {
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end_exclusive = .;
        __device_tree_start = .;
    } :segment_data

    .got : { *(.got*) }
    ASSERT(SIZEOF(.got) == 0, "Relocation support not expected")
    /DISCARD/ : { *(.comment*) }
}
