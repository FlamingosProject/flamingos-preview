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

.macro BLINK_CODE code, word_space
	mov	x0, {CONST_BOOT_TRACE}
	cmp	x0, #0
	b.eq	noblink_\@
	mov	x0, \code
	mov	x1, \word_space
	bl	_blink_code
noblink_\@:
	nop
.endm
        
.macro BLINK_CODE_WS code
	BLINK_CODE \code, #1
.endm

.macro BLINK_CODE_NWS code
	BLINK_CODE \code, #0
.endm

.macro PANIC code
	mov x0, \code
	bl _panic_code
.endm

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------
.section .text._start

//------------------------------------------------------------------------------
// fn _start()
//------------------------------------------------------------------------------
_start:
        // Save the device table pointer in a callee-saves register for later.
	mov	x19, x0

	// Load the PC-relative address of the stack and set the stack pointer.
	//
	// Since this is the first code that runs after the firmware has loaded the kernel
	// into memory, retrieving this symbol PC-relative returns the "physical" address.
	//
	// Setting the stack pointer to this value ensures that anything that still runs in EL2,
	// until the kernel returns to EL1 with the MMU enabled, works as well. After the return to
	// EL1, the virtual address of the stack retrieved above will be used.
	ADR_REL	x0, __boot_core_stack_end_exclusive
	mov	sp, x0

        // Assure working blinking.
        BLINK_CODE_NWS #9
        BLINK_CODE_WS #9

        ic      iallu           // Invalidate instruction cache
        // dc      civac, x0       // Clean & invalidate data cache
        dsb     sy              // Data synchronization barrier
        isb                     // Instruction synchronization barrier

	// Stage 1
        BLINK_CODE_WS #1

	// Only proceed if the core executes in EL2. Park it otherwise.
	mrs	x1, CurrentEL
	cmp	x1, {CONST_CURRENTEL_EL2}
	b.eq	.L_have_permissions
        PANIC #0x11
.L_have_permissions:

	// Stage 2
        BLINK_CODE_WS #2

	// Only proceed on the boot core. Park it otherwise.
	mrs	x1, MPIDR_EL1
	and	x1, x1, {CONST_CORE_ID_MASK}
	ldr	x2, BOOT_CORE_ID      // provided by bsp/__board_name__/cpu.rs
	cmp	x1, x2
	b.eq	.L_am_boot_core
.L_not_boot_core:
        PANIC #0x12
        b       .L_not_boot_core
.L_am_boot_core:

	// If execution reaches here, it is the boot core.

	// Stage 3
        BLINK_CODE_WS #3

	// Initialize DRAM.
	ADR_REL	x0, __bss_start
	ADR_REL x1, __bss_end_exclusive
.L_bss_init_loop:
	cmp	x0, x1
	b.eq	.L_prepare_rust
	stp	xzr, xzr, [x0], #16
	b	.L_bss_init_loop
.L_prepare_rust:

	// Stage 4
        BLINK_CODE_WS #4

	// Prepare the jump to Rust code.
	// Load the base address of the kernel's translation tables.
	ldr	x0, PHYS_KERNEL_TABLES_BASE_ADDR // provided by bsp/__board_name__/memory/mmu.rs

	// Load the _absolute_ addresses of the following symbols. Since the kernel is linked at
	// the top of the 64 bit address space, these are effectively virtual addresses.
	ADR_ABS	x1, __boot_core_stack_end_exclusive
	ADR_ABS	x2, kernel_init
	mov	x3, x19

	// Jump to Rust code. x0, x1, x2 hold the function arguments provided to _start_rust().
	b	_start_rust

.size	_start, . - _start
.type	_start, function
.global	_start
