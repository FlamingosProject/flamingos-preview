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
	// Preserve the device tree pointer supplied in x0 by the firmware or chainloader.
	mov	x19, x0

	// Only proceed on the boot core. Park it otherwise.
	mrs	x1, MPIDR_EL1
	and	x1, x1, {CONST_CORE_ID_MASK}
	ldr	x2, BOOT_CORE_ID      // provided by bsp/__board_name__/cpu.rs
	cmp	x1, x2
	b.ne	.L_parking_loop

	// If execution reaches here, it is the boot core.

	// Establish a stack before calling the Rust boot-tracing helper. The stack occupies the
	// physical address range below the kernel image and is not part of BSS.
	ADR_REL	x0, __boot_core_stack_end_exclusive
	mov	sp, x0

	BLINK_CODE #1

	// Only proceed if the core executes in EL2. Report a fatal boot error otherwise.
	mrs	x0, CurrentEL
	cmp	x0, {CONST_CURRENTEL_EL2}
	b.eq	.L_in_el2
	PANIC	#0x11
.L_in_el2:

	// Initialize DRAM.
	ADR_REL	x0, __bss_start
	ADR_REL x1, __bss_end_exclusive

.L_bss_init_loop:
	cmp	x0, x1
	b.eq	.L_prepare_rust
	stp	xzr, xzr, [x0], #16
	b	.L_bss_init_loop

	// Prepare the jump to Rust code.
.L_prepare_rust:
	BLINK_CODE #2

	// Zero the frame pointer and link register so that the unwinder knows this is the
	// bottom of the call stack.
	mov	x29, xzr
	mov	x30, xzr

	// Pass the boot stack and preserved device tree pointer to Rust.
	ADR_REL	x0, __boot_core_stack_end_exclusive
	mov	x1, x19
	b	_start_rust

	// Infinitely wait for events (aka "park the core").
.L_parking_loop:
	wfe
	b	.L_parking_loop

.size	_start, . - _start
.type	_start, function
.global	_start
