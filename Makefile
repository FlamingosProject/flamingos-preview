## SPDX-License-Identifier: MIT OR Apache-2.0
##
## Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

include common/format.mk
include common/operating_system.mk

##--------------------------------------------------------------------------------------------------
## Optional, user-provided configuration values
##--------------------------------------------------------------------------------------------------

# Default to the RPi3.
BSP ?= rpi3



##--------------------------------------------------------------------------------------------------
## BSP-specific configuration values
##--------------------------------------------------------------------------------------------------
QEMU_MISSING_STRING = "This board is not yet supported for QEMU."

ifeq ($(BSP),rpi3)
    TARGET            = aarch64-unknown-none-softfloat
    KERNEL_BIN        = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE = raspi3b
    QEMU_RELEASE_ARGS = -serial stdio -display none
    QEMU_TEST_ARGS    = $(QEMU_RELEASE_ARGS) -semihosting
    OBJDUMP_BINARY    = aarch64-none-elf-objdump
    NM_BINARY         = aarch64-none-elf-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    OPENOCD_ARG       = -f /openocd/tcl/interface/ftdi/olimex-arm-usb-tiny-h.cfg -f /openocd/rpi3.cfg
    JTAG_BOOT_IMAGE   = ../X1_JTAG_boot/jtag_boot_rpi3.img
    LD_SCRIPT_PATH    = $(shell pwd)/kernel/src/bsp/raspberrypi
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a53
else ifeq ($(BSP),rpi4)
    TARGET            = aarch64-unknown-none-softfloat
    KERNEL_BIN        = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE =
    QEMU_RELEASE_ARGS = -serial stdio -display none
    QEMU_TEST_ARGS    = $(QEMU_RELEASE_ARGS) -semihosting
    OBJDUMP_BINARY    = aarch64-none-elf-objdump
    NM_BINARY         = aarch64-none-elf-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    OPENOCD_ARG       = -f /openocd/tcl/interface/ftdi/olimex-arm-usb-tiny-h.cfg -f /openocd/rpi4.cfg
    JTAG_BOOT_IMAGE   = ../X1_JTAG_boot/jtag_boot_rpi4.img
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a72
endif


##--------------------------------------------------------------------------------------------------
## Targets and Prerequisites
##--------------------------------------------------------------------------------------------------
KERNEL_MANIFEST      = kernel/Cargo.toml
KERNEL_LINKER_SCRIPT = kernel.ld
LAST_BUILD_CONFIG    = target/$(BSP).build_config

KERNEL_ELF_RAW      = target/$(TARGET)/release/kernel
# This parses cargo's dep-info file.
# https://doc.rust-lang.org/cargo/guide/build-cache.html#dep-info-files
KERNEL_ELF_RAW_DEPS = $(filter-out %: ,$(file < $(KERNEL_ELF_RAW).d)) $(KERNEL_MANIFEST) $(LAST_BUILD_CONFIG)

##------------------------------------------------------------------------------
## Translation tables
##------------------------------------------------------------------------------
TT_TOOL_PATH = tools/bin

KERNEL_ELF_TTABLES      = target/$(TARGET)/release/kernel+ttables
KERNEL_ELF_TTABLES_DEPS = $(KERNEL_ELF_RAW) $(wildcard $(TT_TOOL_PATH)/*)

##------------------------------------------------------------------------------
## Kernel symbols
##------------------------------------------------------------------------------
export KERNEL_SYMBOLS_TOOL_PATH = tools/kernel_symbols_tool

KERNEL_ELF_TTABLES_SYMS = target/$(TARGET)/release/kernel+ttables+symbols

# Unlike with KERNEL_ELF_RAW, we are not relying on dep-info here. One of the reasons being that the
# name of the generated symbols file varies between runs, which can cause confusion.
KERNEL_ELF_TTABLES_SYMS_DEPS = $(KERNEL_ELF_TTABLES) \
    $(wildcard kernel_symbols/*)                     \
    $(EXEC_KERNEL_SYMBOLS_TOOL)

# This overrides the two ENV variables. The other ENV variables that are required as input for
# the .mk file are set already because they are exported by this Makefile and this script is
# started by the same.
KERNEL_SYMBOLS_INPUT_ELF=$$TEST_ELF           \
    KERNEL_SYMBOLS_OUTPUT_ELF=$$TEST_ELF_SYMS \
    $(MAKE) --no-print-directory -f kernel_symbols.mk > /dev/null 2>&1


export TARGET
export KERNEL_SYMBOLS_INPUT_ELF  = $(KERNEL_ELF_TTABLES)
export KERNEL_SYMBOLS_OUTPUT_ELF = $(KERNEL_ELF_TTABLES_SYMS)

KERNEL_ELF = $(KERNEL_ELF_TTABLES_SYMS)



##--------------------------------------------------------------------------------------------------
## Command building blocks
##--------------------------------------------------------------------------------------------------
RUSTFLAGS_PEDANTIC = $(RUSTFLAGS) \
    -D missing_docs

FEATURES      = --features bsp_$(BSP)
COMPILER_ARGS = --target=$(TARGET) \
    $(FEATURES)                    \
    --release

RUSTC_CMD   = cargo rustc $(COMPILER_ARGS) --manifest-path $(KERNEL_MANIFEST)
DOC_CMD     = cargo doc $(COMPILER_ARGS)
CLIPPY_CMD  = cargo clippy $(COMPILER_ARGS)
TEST_CMD    = cargo test $(COMPILER_ARGS) --manifest-path $(KERNEL_MANIFEST)
OBJCOPY_CMD = rust-objcopy \
    --strip-all            \
    -O binary

EXEC_QEMU              = $(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE)
EXEC_TT_TOOL           = $(TT_TOOL_PATH)/translation_table_tool
EXEC_KERNEL_SYMBOLS_TOOL = target/release/kernel-elf-symbol



##--------------------------------------------------------------------------------------------------
## Targets
##--------------------------------------------------------------------------------------------------
.PHONY: all doc qemu clippy clean readelf objdump nm check

all: $(KERNEL_BIN)

##------------------------------------------------------------------------------
## Save the configuration as a file, so make understands if it changed.
##------------------------------------------------------------------------------
$(LAST_BUILD_CONFIG):
	@rm -f target/*.build_config
	@mkdir -p target
	@touch $(LAST_BUILD_CONFIG)

##------------------------------------------------------------------------------
## Compile the kernel ELF
##------------------------------------------------------------------------------
$(KERNEL_ELF_RAW): $(KERNEL_ELF_RAW_DEPS)
	$(call color_header, "Compiling kernel ELF - $(BSP)")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(RUSTC_CMD)

##------------------------------------------------------------------------------
## Precompute the kernel translation tables and patch them into the kernel ELF
##------------------------------------------------------------------------------
$(KERNEL_ELF_TTABLES): $(KERNEL_ELF_TTABLES_DEPS)
	$(call color_header, "Precomputing kernel translation tables and patching kernel ELF")
	TMP=/tmp/kernel-elf-raw.$$$$ && \
	cp $(KERNEL_ELF_RAW) $$TMP && \
	$(EXEC_TT_TOOL) $(BSP) $$TMP && \
	cp $$TMP $(KERNEL_ELF_TTABLES) && \
	rm $$TMP

##------------------------------------------------------------------------------
## Build kernel symbols tool
##------------------------------------------------------------------------------
$(EXEC_KERNEL_SYMBOLS_TOOL): $(wildcard $(KERNEL_SYMBOLS_TOOL_PATH)/src/*.rs) $(KERNEL_SYMBOLS_TOOL_PATH)/Cargo.toml
	$(call color_header, "Building kernel symbols tool")
	@cargo build --package kernel_symbols_tool --release --quiet

##------------------------------------------------------------------------------
## Generate kernel symbols and patch them into the kernel ELF
##------------------------------------------------------------------------------
$(KERNEL_ELF_TTABLES_SYMS): $(KERNEL_ELF_TTABLES_SYMS_DEPS)
	$(call color_header, "Generating kernel symbols and patching kernel ELF")
	@$(MAKE) --no-print-directory -f kernel_symbols.mk

##------------------------------------------------------------------------------
## Generate the stripped kernel binary
##------------------------------------------------------------------------------
$(KERNEL_BIN): $(KERNEL_ELF_TTABLES_SYMS)
	$(call color_header, "Generating stripped binary")
	@$(OBJCOPY_CMD) $(KERNEL_ELF_TTABLES_SYMS) $(KERNEL_BIN)
	$(call color_progress_prefix, "Name")
	@echo $(KERNEL_BIN)
	$(call color_progress_prefix, "Size")
	$(call disk_usage_KiB, $(KERNEL_BIN))

##------------------------------------------------------------------------------
## Generate the documentation
##------------------------------------------------------------------------------
doc: clean
	$(call color_header, "Generating docs")
	@$(DOC_CMD) --document-private-items --open

##------------------------------------------------------------------------------
## Run the kernel in QEMU
##------------------------------------------------------------------------------
ifeq ($(QEMU_MACHINE_TYPE),) # QEMU is not supported for the board.

qemu:
	$(call color_header, "$(QEMU_MISSING_STRING)")

else # QEMU is supported.

qemu: $(KERNEL_BIN)
	$(call color_header, "Launching QEMU")
	$(EXEC_QEMU) $(QEMU_RELEASE_ARGS) -kernel $(KERNEL_BIN)
endif

##------------------------------------------------------------------------------
## Run clippy
##------------------------------------------------------------------------------
clippy:
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(CLIPPY_CMD)

##------------------------------------------------------------------------------
## Clean
##------------------------------------------------------------------------------
clean:
	rm -rf target $(KERNEL_BIN)

##------------------------------------------------------------------------------
## Run readelf
##------------------------------------------------------------------------------
readelf: $(KERNEL_ELF)
	$(call color_header, "Launching readelf")
	$(READELF_BINARY) --headers $(KERNEL_ELF)

##------------------------------------------------------------------------------
## Run objdump
##------------------------------------------------------------------------------
objdump: $(KERNEL_ELF)
	$(call color_header, "Launching objdump")
	$(OBJDUMP_BINARY) --disassemble --demangle \
                --section .text   \
                --section .rodata \
                $(KERNEL_ELF) | rustfilt

##------------------------------------------------------------------------------
## Run nm
##------------------------------------------------------------------------------
nm: $(KERNEL_ELF)
	$(call color_header, "Launching nm")
	$(NM_BINARY) --demangle --print-size $(KERNEL_ELF) | sort | rustfilt

