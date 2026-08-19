// SPDX-License-Identifier: MIT OR Apache-2.0

.macro ADR_REL register, symbol
	adrp	\register, \symbol
	add	\register, \register, #:lo12:\symbol
.endm

.macro ADR_ABS register, symbol
	movz	\register, #:abs_g2:\symbol
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

.section .text._start

_start:
	// Preserve the firmware device tree by copying it into the relocated loader's BSS.
	ldr	w1, [x0, #4]
	rev	w2, w1
	ADR_ABS	x1, __device_tree_start
	add	x2, x2, x1

.L_device_tree_copy_loop:
	ldr	x3, [x0], #8
	str	x3, [x1], #8
	cmp	x1, x2
	b.lo	.L_device_tree_copy_loop

	// Only the firmware-selected boot core may relocate and run the loader.
	mrs	x0, MPIDR_EL1
	and	x0, x0, {CONST_CORE_ID_MASK}
	ldr	x1, BOOT_CORE_ID
	cmp	x0, x1
	b.ne	.L_parking_loop

	ADR_REL	x0, __boot_core_stack_end_exclusive
	mov	sp, x0
	BLINK_CODE #1

	ADR_ABS	x0, __bss_start
	ADR_ABS	x1, __bss_end_exclusive

.L_bss_init_loop:
	cmp	x0, x1
	b.eq	.L_relocate_binary
	stp	xzr, xzr, [x0], #16
	b	.L_bss_init_loop

.L_relocate_binary:
	ADR_REL	x0, __binary_nonzero_start
	ADR_ABS	x1, __binary_nonzero_start
	ADR_ABS	x2, __binary_nonzero_end_exclusive

.L_binary_relocate_copy_loop:
	ldr	x3, [x0], #8
	str	x3, [x1], #8
	cmp	x1, x2
	b.lo	.L_binary_relocate_copy_loop

	ADR_ABS	x0, __boot_core_stack_end_exclusive
	mov	sp, x0
	BLINK_CODE #2

	ADR_ABS	x0, __device_tree_start
	ADR_ABS	x1, _start_rust
	br	x1

.L_parking_loop:
	wfe
	b	.L_parking_loop

.size	_start, . - _start
.type	_start, function
.global	_start
