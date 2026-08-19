// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//--------------------------------------------------------------------------------------------------
// Definitions
//--------------------------------------------------------------------------------------------------

/// Call the function provided by parameter `\handler` after saving the exception context. Provide
/// the context as the first parameter to '\handler'.
/// XXX There are 28 instructions here currently. We have room
/// for 32 in the instruction vector.
.macro CALL_WITH_CONTEXT handler
__vector_\handler:
	.cfi_startproc simple
	.cfi_signal_frame

	// Make room on the stack for the exception context.
	sub	sp,  sp,  #16 * 17

	// CFA = pre-exception SP = current sp + 272
	.cfi_def_cfa sp, 16 * 17

	// Store all general purpose registers on the stack.
	stp	x0,  x1,  [sp, #16 * 0]
	stp	x2,  x3,  [sp, #16 * 1]
	stp	x4,  x5,  [sp, #16 * 2]
	stp	x6,  x7,  [sp, #16 * 3]
	stp	x8,  x9,  [sp, #16 * 4]
	stp	x10, x11, [sp, #16 * 5]
	stp	x12, x13, [sp, #16 * 6]
	stp	x14, x15, [sp, #16 * 7]
	stp	x16, x17, [sp, #16 * 8]
	stp	x18, x19, [sp, #16 * 9]
	stp	x20, x21, [sp, #16 * 10]
	stp	x22, x23, [sp, #16 * 11]
	stp	x24, x25, [sp, #16 * 12]
	stp	x26, x27, [sp, #16 * 13]
	stp	x28, x29, [sp, #16 * 14]

	// Add the exception link register (ELR_EL1), saved program status (SPSR_EL1) and exception
	// syndrome register (ESR_EL1).
	mrs	x1,  ELR_EL1
	mrs	x2,  SPSR_EL1
	mrs	x3,  ESR_EL1

	stp	lr,  x1,  [sp, #16 * 15]
	stp	x2,  x3,  [sp, #16 * 16]

	// Tell the unwinder where each register was saved (offsets from CFA).
	// x0..x29 are at their natural positions.
	.cfi_offset x0,  -(16 * 17)
	.cfi_offset x1,  -(16 * 17 - 8)
	.cfi_offset x2,  -(16 * 17 - 16)
	.cfi_offset x3,  -(16 * 17 - 24)
	.cfi_offset x4,  -(16 * 17 - 32)
	.cfi_offset x5,  -(16 * 17 - 40)
	.cfi_offset x6,  -(16 * 17 - 48)
	.cfi_offset x7,  -(16 * 17 - 56)
	.cfi_offset x8,  -(16 * 17 - 64)
	.cfi_offset x9,  -(16 * 17 - 72)
	.cfi_offset x10, -(16 * 17 - 80)
	.cfi_offset x11, -(16 * 17 - 88)
	.cfi_offset x12, -(16 * 17 - 96)
	.cfi_offset x13, -(16 * 17 - 104)
	.cfi_offset x14, -(16 * 17 - 112)
	.cfi_offset x15, -(16 * 17 - 120)
	.cfi_offset x16, -(16 * 17 - 128)
	.cfi_offset x17, -(16 * 17 - 136)
	.cfi_offset x18, -(16 * 17 - 144)
	.cfi_offset x19, -(16 * 17 - 152)
	.cfi_offset x20, -(16 * 17 - 160)
	.cfi_offset x21, -(16 * 17 - 168)
	.cfi_offset x22, -(16 * 17 - 176)
	.cfi_offset x23, -(16 * 17 - 184)
	.cfi_offset x24, -(16 * 17 - 192)
	.cfi_offset x25, -(16 * 17 - 200)
	.cfi_offset x26, -(16 * 17 - 208)
	.cfi_offset x27, -(16 * 17 - 216)
	.cfi_offset x28, -(16 * 17 - 224)
	.cfi_offset x29, -(16 * 17 - 232)
	// x30 (return-address register on aarch64) -> ELR_EL1 at sp+248, so the
	// unwinder uses the faulting PC to continue into the interrupted frame.
	.cfi_offset x30, -(16 * 17 - 248)

	// x0 is the first argument for the function called through `\handler`.
	mov	x0,  sp

	// Call `\handler`.
	bl	\handler

	// After returning from exception handling code, replay the saved context and return via
	// `eret`.
	b	__exception_restore_context

	.cfi_endproc
.size	__vector_\handler, . - __vector_\handler
.type	__vector_\handler, function
.endm

.macro FIQ_SUSPEND
1:	wfe
	b	1b
.endm

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------
.section .text

//------------------------------------------------------------------------------
// The exception vector table.
//------------------------------------------------------------------------------

// Align by 2^11 bytes, as demanded by ARMv8-A. Same as ALIGN(2048) in an ld script.
.align 11

// Export a symbol for the Rust code to use.
__exception_vector_start:

// Current exception level with SP_EL0.
//
// .org sets the offset relative to section start.
//
// # Safety
//
// - It must be ensured that `CALL_WITH_CONTEXT` <= 0x80 bytes.
.org 0x000
	CALL_WITH_CONTEXT current_el0_synchronous
.org 0x080
	CALL_WITH_CONTEXT current_el0_irq
.org 0x100
	FIQ_SUSPEND
.org 0x180
	CALL_WITH_CONTEXT current_el0_serror

// Current exception level with SP_ELx, x > 0.
.org 0x200
	CALL_WITH_CONTEXT current_elx_synchronous
.org 0x280
	CALL_WITH_CONTEXT current_elx_irq
.org 0x300
	FIQ_SUSPEND
.org 0x380
	CALL_WITH_CONTEXT current_elx_serror

// Lower exception level, AArch64
.org 0x400
	CALL_WITH_CONTEXT lower_aarch64_synchronous
.org 0x480
	CALL_WITH_CONTEXT lower_aarch64_irq
.org 0x500
	FIQ_SUSPEND
.org 0x580
	CALL_WITH_CONTEXT lower_aarch64_serror

// Lower exception level, AArch32
.org 0x600
	CALL_WITH_CONTEXT lower_aarch32_synchronous
.org 0x680
	CALL_WITH_CONTEXT lower_aarch32_irq
.org 0x700
	FIQ_SUSPEND
.org 0x780
	CALL_WITH_CONTEXT lower_aarch32_serror
.org 0x800

//------------------------------------------------------------------------------
// fn __exception_restore_context()
//------------------------------------------------------------------------------
__exception_restore_context:
	ldr	w19,      [sp, #16 * 16]
	ldp	lr,  x20, [sp, #16 * 15]

	msr	SPSR_EL1, x19
	msr	ELR_EL1,  x20

	ldp	x0,  x1,  [sp, #16 * 0]
	ldp	x2,  x3,  [sp, #16 * 1]
	ldp	x4,  x5,  [sp, #16 * 2]
	ldp	x6,  x7,  [sp, #16 * 3]
	ldp	x8,  x9,  [sp, #16 * 4]
	ldp	x10, x11, [sp, #16 * 5]
	ldp	x12, x13, [sp, #16 * 6]
	ldp	x14, x15, [sp, #16 * 7]
	ldp	x16, x17, [sp, #16 * 8]
	ldp	x18, x19, [sp, #16 * 9]
	ldp	x20, x21, [sp, #16 * 10]
	ldp	x22, x23, [sp, #16 * 11]
	ldp	x24, x25, [sp, #16 * 12]
	ldp	x26, x27, [sp, #16 * 13]
	ldp	x28, x29, [sp, #16 * 14]

	add	sp,  sp,  #16 * 17

	eret

.size	__exception_restore_context, . - __exception_restore_context
.type	__exception_restore_context, function
