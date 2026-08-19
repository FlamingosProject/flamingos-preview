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
	// Keep the firmware pointer while the relocated loader is initialized.
	mov x19, x0

	// Park other cores before any shared memory, stack or GPIO accesses.
	mrs x0, MPIDR_EL1
	and x0, x0, {CONST_CORE_ID_MASK}
	ldr x1, BOOT_CORE_ID
	cmp x0, x1
	b.ne .L_parking_loop
	msr daifset, #0xf

	// This bootstrap's physical-address contract requires EL2, MMU/data cache off.
	mrs x0, CurrentEL
	cmp x0, #8
	b.ne .L_parking_loop
	mrs x0, SCTLR_EL2
	tst x0, #1
	b.ne .L_parking_loop
	tst x0, #4
	b.ne .L_parking_loop

	// Preserve a bounded FDT, or QEMU's legacy ATAG_CORE header. The buffer
	// is separate from BSS and relocation destinations; preserve it before
	// either operation can overwrite firmware-owned source memory.
	ldr w1, [x19, #4]
	movz w2, #0x0001
	movk w2, #0x5441, lsl #16
	cmp w1, w2
	b.ne .L_chain_fdt
	mov x2, #8
	b .L_chain_tree_size
.L_chain_fdt:
	ldr w2, [x19]
	rev w2, w2
	movz w3, #0xfeed
	movk w3, #0xd00d, lsl #16
	cmp w2, w3
	b.ne .L_parking_loop
	rev w2, w1
	cmp x2, #40
	b.lo .L_parking_loop
.L_chain_tree_size:
	mov x3, #0x20000
	cmp x2, x3
	b.hi .L_parking_loop
	ADR_ABS x1, __device_tree_start
	cmp x1, x19
	b.ls .L_device_tree_copy_loop
	add x3, x19, x2
	cmp x1, x3
	b.hs .L_device_tree_copy_loop
	add x19, x19, x2
	add x1, x1, x2
.L_device_tree_copy_backwards:
	ldrb w3, [x19, #-1]!
	strb w3, [x1, #-1]!
	subs x2, x2, #1
	b.ne .L_device_tree_copy_backwards
	b .L_device_tree_preserved
.L_device_tree_copy_loop:
	ldrb w3, [x19], #1
	strb w3, [x1], #1
	subs x2, x2, #1
	b.ne .L_device_tree_copy_loop
.L_device_tree_preserved:


	ADR_REL x0, __boot_core_stack_end_exclusive
	mov sp, x0
	BLINK_CODE #1

	ADR_ABS x0, __bss_start
	ADR_ABS x1, __bss_end_exclusive
.L_bss_init_loop:
	cmp x0, x1
	b.eq .L_relocate_binary
	stp xzr, xzr, [x0], #16
	b .L_bss_init_loop

.L_relocate_binary:
	ADR_REL x0, __binary_nonzero_start
	ADR_ABS x1, __binary_nonzero_start
	ADR_ABS x2, __binary_nonzero_end_exclusive
	// The embedded source can approach the relocation gap in size. Copy from
	// the end so relocation is safe even if source and destination overlap.
	sub x3, x2, x1
	add x0, x0, x3
	mov x1, x2
	ADR_ABS x2, __binary_nonzero_start
.L_binary_relocate_copy_loop:
	cmp x1, x2
	b.eq .L_relocated
	ldrb w3, [x0, #-1]!
	strb w3, [x1, #-1]!
	b .L_binary_relocate_copy_loop

.L_relocated:
	// Instruction caching may already be enabled even though the data cache
	// is off. Publish copied instructions before calling relocated Rust code.
	dsb sy
	ic iallu
	dsb sy
	isb
	ADR_ABS x0, __boot_core_stack_end_exclusive
	mov sp, x0


	BLINK_CODE #2
	ADR_ABS x0, __device_tree_start
	ADR_ABS x1, _start_rust
	br x1

.L_parking_loop:
	wfe
	b	.L_parking_loop

.size	_start, . - _start
.type	_start, function
.global	_start
