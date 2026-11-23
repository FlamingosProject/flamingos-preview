// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2021-2022 Andre Richter <andre.o.richter@gmail.com>

//--------------------------------------------------------------------------------------------------
// Definitions
//--------------------------------------------------------------------------------------------------

// Load the address of a symbol into a register, PC-relative.
//
// The symbol must lie within +/- 4 GiB of the Program Counter.
//
// # Resources
//
// - https://sourceware.org/binutils/docs-2.36/as/AArch64_002dRelocations.html
.macro ADR_REL register, symbol
	adrp	\register, \symbol
	add	\register, \register, #:lo12:\symbol
.endm

// Load the address of a symbol into a register, absolute.
//
// # Resources
//
// - https://sourceware.org/binutils/docs-2.36/as/AArch64_002dRelocations.html
.macro ADR_ABS register, symbol
	movz	\register, #:abs_g3:\symbol
	movk	\register, #:abs_g2_nc:\symbol
	movk	\register, #:abs_g1_nc:\symbol
	movk	\register, #:abs_g0_nc:\symbol
.endm

.macro BLINK_CODE code
	mov	x0, {CONST_BOOT_TRACE}
	cbz	x0, .L_no_blink_\@
	mov	x0, \code
	mov	x1, #1
	bl	_blink_code
.L_no_blink_\@:
.endm

.macro PANIC code
	mov	x0, \code
	b	_panic_code
.endm

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------
.section .text._start

//------------------------------------------------------------------------------
// fn _start()
//------------------------------------------------------------------------------
_start:
	// Preserve the device tree and optional retained ELF handoff.
	mov	x19, x0
	mov	x20, x1
	mov	x21, x2

	// Only proceed on the boot core. Park other cores before using the shared boot stack.
	mrs	x1, MPIDR_EL1
	and	x1, x1, {CONST_CORE_ID_MASK}
	ldr	x2, BOOT_CORE_ID      // provided by bsp/__board_name__/cpu.rs
	cmp	x1, x2
	b.eq	.L_am_boot_core

	// Leave a core parked until started by setting the core's BOOT_PARK byte to nonzero and
	// interrupting the core.
.L_not_boot_core:
	wfe
	ADR_REL	x3, BOOT_PARK
	ldrb	w3, [x3, x1]
	cbz	w3, .L_not_boot_core
	mov	x0, x1
	ADR_ABS	x3, __phys_boot_core_stack_end_exclusive
	mov	sp, x3
	b	_start_core

.L_am_boot_core:
	// If execution reaches here, it is the boot core.

	// Establish a physical stack before calling the Rust boot-tracing helper.
	ADR_ABS	x0, __phys_boot_core_stack_end_exclusive
	mov	sp, x0

	BLINK_CODE #1

	// Only proceed if the core executes in EL2. Report a fatal boot error otherwise.
	mrs	x0, CurrentEL
	cmp	x0, {CONST_CURRENTEL_EL2}
	b.eq	.L_in_el2
	PANIC	#0x11
.L_in_el2:
	msr	DAIFSet, #0xf

	// Initialize DRAM.
	ADR_ABS	x0, __phys_bss_start
	ADR_ABS	x1, __phys_bss_end_exclusive
.L_bss_init_loop:
	cmp	x0, x1
	b.eq	.L_prepare_rust
	stp	xzr, xzr, [x0], #16
	b	.L_bss_init_loop

.L_prepare_rust:
	// QEMU passes an ATAG_CORE header, while firmware and the UART
	// chainloader pass an FDT. Work out how many bytes must survive.
	ldr	w4, [x19, #4]
	movz	w5, #0x0001
	movk	w5, #0x5441, lsl #16
	cmp	w4, w5
	b.ne	.L_device_tree_is_fdt
	mov	x4, #8
	b	.L_device_tree_size_ready

.L_device_tree_is_fdt:
	ldr	w5, [x19]
	rev	w5, w5
	movz	w6, #0xfeed
	movk	w6, #0xd00d, lsl #16
	cmp	w5, w6
	b.ne	.L_bad_device_tree
	rev	w4, w4

.L_device_tree_size_ready:
	cbz	x4, .L_bad_device_tree
	mov	x5, {CONST_DEVICE_TREE_BUFFER_SIZE}
	cmp	x4, x5
	b.hi	.L_device_tree_too_large

	// Copy the handoff into the kernel's BSS for use after MMU-on.
	ADR_ABS	x5, __phys_device_tree_buffer
	add	x6, x5, x4
.L_device_tree_copy_loop:
	ldrb	w7, [x19], #1
	strb	w7, [x5], #1
	cmp	x5, x6
	b.lo	.L_device_tree_copy_loop

	BLINK_CODE #2

	// The loader supplies the ELF byte count at the reserved physical base.
	ADR_ABS	x0, __boot_elf_phys_start
	cmp	x20, x0
	csel	x4, x21, xzr, eq

	// Pass physical table storage and layout, alongside the linked EL1 addresses.
	ADR_ABS	x0, __phys_kernel_tables
	ADR_ABS	x1, __boot_core_stack_end_exclusive
	ADR_ABS	x2, kernel_init
	ADR_ABS	x3, DEVICE_TREE_BUFFER
	ADR_REL	x5, __boot_memory_layout
	ADR_ABS	x6, __phys_boot_elf_size

	// Jump to the physical Rust bootstrap.
	b	_start_rust

.L_bad_device_tree:
	PANIC	#0x12

.L_device_tree_too_large:
	PANIC	#0x13

	// Infinitely wait for events (aka "park the core").
.L_parking_loop:
	wfe
	b	.L_parking_loop

.size	_start, . - _start
.type	_start, function
.global	_start
