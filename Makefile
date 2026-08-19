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
DEV_SERIAL ?= /dev/ttyUSB0
SCIP ?= scip
CHAINBOOT_PAYLOAD ?= kernel8.img



##--------------------------------------------------------------------------------------------------
## BSP-specific configuration values
##--------------------------------------------------------------------------------------------------
QEMU_MISSING_STRING = "This board is not yet supported for QEMU."

ifeq ($(BSP),rpi3)
    TARGET            = aarch64-unknown-none-softfloat
    NORMAL_KERNEL_BIN = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE = raspi3b
    QEMU_RELEASE_ARGS = -serial stdio -display none
    QEMU_TEST_ARGS    = $(QEMU_RELEASE_ARGS) -semihosting
    OBJDUMP_BINARY    = rust-objdump
    NM_BINARY         = rust-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    LD_SCRIPT_PATH    = $(shell pwd)/src/bsp/raspberrypi
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a53
else ifeq ($(BSP),rpi4)
    TARGET            = aarch64-unknown-none-softfloat
    NORMAL_KERNEL_BIN = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE =
    QEMU_RELEASE_ARGS = -serial stdio -display none
    OBJDUMP_BINARY    = rust-objdump
    NM_BINARY         = rust-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    LD_SCRIPT_PATH    = $(shell pwd)/src/bsp/raspberrypi
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a72
endif

##--------------------------------------------------------------------------------------------------
## Targets and Prerequisites
##--------------------------------------------------------------------------------------------------
KERNEL_MANIFEST      = Cargo.toml
CHAINLOADER_BIN      = chainloader8.img
KERNEL_BIN           = $(if $(CHAINLOADER),$(CHAINLOADER_BIN),$(NORMAL_KERNEL_BIN))
BUILD_MODE           = $(if $(CHAINLOADER),chainloader,kernel)
LAST_BUILD_CONFIG    = target/$(BSP).$(BUILD_MODE).build_config

KERNEL_ELF      = target/$(TARGET)/release/kernel
TEST_BUILD_DIR  = target/test_build/$(BSP)
TEST_KERNEL_ELF = $(TEST_BUILD_DIR)/$(TARGET)/release/kernel
TEST_KERNEL_BIN = $(TEST_BUILD_DIR)/kernel8.img
# This parses cargo's dep-info file.
# https://doc.rust-lang.org/cargo/guide/build-cache.html#dep-info-files
KERNEL_ELF_DEPS = $(filter-out %: ,$(file < $(KERNEL_ELF).d)) $(KERNEL_MANIFEST) $(LAST_BUILD_CONFIG)



##--------------------------------------------------------------------------------------------------
## Command building blocks
##--------------------------------------------------------------------------------------------------
RUSTFLAGS = $(RUSTC_MISC_ARGS)

RUSTFLAGS_PEDANTIC = $(RUSTFLAGS) \
    -D missing_docs

KERNEL_FEATURES = bsp_$(BSP)
ifdef CHAINLOADER
    KERNEL_FEATURES := $(KERNEL_FEATURES),chainloader
endif
FEATURES      = --features $(KERNEL_FEATURES)
TEST_FEATURES = --no-default-features --features bsp_$(BSP),test_build
COMPILER_ARGS = --target=$(TARGET) \
    $(FEATURES)                    \
    --release

RUSTC_CMD   = cargo rustc $(COMPILER_ARGS)
TEST_RUSTC_CMD = cargo rustc                     \
    --target=$(TARGET)                           \
    $(TEST_FEATURES)                             \
    --release                                    \
    --target-dir=$(TEST_BUILD_DIR)
DOC_CMD     = cargo doc $(COMPILER_ARGS)
CLIPPY_CMD  = cargo clippy $(COMPILER_ARGS)
OBJCOPY_CMD = rust-objcopy \
    --strip-all            \
    -O binary

EXEC_QEMU = $(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE)




##--------------------------------------------------------------------------------------------------
## Targets
##--------------------------------------------------------------------------------------------------
.PHONY: all chainboot doc qemu test_boot clippy clean readelf objdump nm check

all: $(KERNEL_BIN)

##------------------------------------------------------------------------------
## Send a payload to a chainloader already running on the target
##------------------------------------------------------------------------------
chainboot: $(NORMAL_KERNEL_BIN)
	@test -f "$(CHAINBOOT_PAYLOAD)" || { \
		echo "Missing payload: $(CHAINBOOT_PAYLOAD)"; \
		exit 1; \
	}
	$(SCIP) --binfile "$(CHAINBOOT_PAYLOAD)" "$(DEV_SERIAL)" 921600 8 N 1 N

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
$(KERNEL_ELF): $(KERNEL_ELF_DEPS)
	$(call color_header, "Compiling kernel ELF - $(BSP)")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(RUSTC_CMD)

##------------------------------------------------------------------------------
## Generate the stripped kernel binary
##------------------------------------------------------------------------------
$(KERNEL_BIN): $(KERNEL_ELF)
	$(call color_header, "Generating stripped binary")
	@$(OBJCOPY_CMD) $(KERNEL_ELF) $(KERNEL_BIN)
	$(call color_progress_prefix, "Name")
	@echo $(KERNEL_BIN)
	$(call color_progress_prefix, "Size")
	$(call disk_usage_KiB, $(KERNEL_BIN))

##------------------------------------------------------------------------------
## Generate the documentation
##------------------------------------------------------------------------------
doc:
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
## Run a deterministic boot smoke test in QEMU
##------------------------------------------------------------------------------
ifeq ($(QEMU_MACHINE_TYPE),) # QEMU is not supported for the board.

test_boot:
	$(call color_header, "$(QEMU_MISSING_STRING)")

else # QEMU is supported.

test_boot:
	$(call color_header, "Building QEMU boot test")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(TEST_RUSTC_CMD)
	@$(OBJCOPY_CMD) $(TEST_KERNEL_ELF) $(TEST_KERNEL_BIN)
	$(call color_header, "Running QEMU boot test")
	$(EXEC_QEMU) $(QEMU_TEST_ARGS) -kernel $(TEST_KERNEL_BIN)
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
	rm -rf target $(NORMAL_KERNEL_BIN) $(CHAINLOADER_BIN)

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
                $(KERNEL_ELF)

##------------------------------------------------------------------------------
## Run nm
##------------------------------------------------------------------------------
nm: $(KERNEL_ELF)
	$(call color_header, "Launching nm")
	$(NM_BINARY) --demangle --print-size $(KERNEL_ELF) | sort
