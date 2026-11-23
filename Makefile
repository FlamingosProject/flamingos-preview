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
EMBEDDED_FS ?= $(if $(CHAINLOADER),0,1)
CHAINBOOT_PAYLOAD ?= $(NORMAL_KERNEL_ELF)
OPENOCD ?= openocd
GDB ?= gdb-multiarch
OPENOCD_INTERFACE ?= interface/ftdi/olimex-arm-usb-tiny-h.cfg



SUPPORTED_BSPS := rpiz2 rpi3 rpi4
ifeq ($(filter $(BSP),$(SUPPORTED_BSPS)),)
$(error BSP must be one of: $(SUPPORTED_BSPS))
endif

# The Raspberry Pi Zero 2 W uses the RPi3 kernel configuration.
BSP_FAMILY := $(if $(filter rpiz2,$(BSP)),rpi3,$(BSP))

##--------------------------------------------------------------------------------------------------
## BSP-specific configuration values
##--------------------------------------------------------------------------------------------------
QEMU_MISSING_STRING = "This board is not yet supported for QEMU."

ifeq ($(BSP_FAMILY),rpi3)
    TARGET            = aarch64-unknown-none-softfloat
    NORMAL_KERNEL_BIN = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE = raspi3b
    QEMU_RELEASE_ARGS = -serial stdio -display none
    QEMU_TEST_ARGS    = $(QEMU_RELEASE_ARGS) -semihosting
    OBJDUMP_BINARY    = rust-objdump
    NM_BINARY         = rust-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    OPENOCD_TARGET_CONFIG = tools/jtag/rpi3.cfg
    JTAG_BOOT_IMAGE   = tools/jtag/jtag_boot_rpi3.img
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a53
else ifeq ($(BSP_FAMILY),rpi4)
    TARGET            = aarch64-unknown-none-softfloat
    NORMAL_KERNEL_BIN = kernel8.img
    QEMU_BINARY       = qemu-system-aarch64
    QEMU_MACHINE_TYPE =
    QEMU_RELEASE_ARGS = -serial stdio -display none
    QEMU_TEST_ARGS    = $(QEMU_RELEASE_ARGS) -semihosting
    OBJDUMP_BINARY    = rust-objdump
    NM_BINARY         = rust-nm
    READELF_BINARY    = aarch64-none-elf-readelf
    OPENOCD_TARGET_CONFIG = tools/jtag/rpi4.cfg
    JTAG_BOOT_IMAGE   = tools/jtag/jtag_boot_rpi4.img
    RUSTC_MISC_ARGS   = -C target-cpu=cortex-a72
endif

##--------------------------------------------------------------------------------------------------
## Targets and Prerequisites
##--------------------------------------------------------------------------------------------------
KERNEL_MANIFEST      = kernel/Cargo.toml
NORMAL_KERNEL_ELF    = kernel.elf
CHAINLOADER_BIN      = chainloader8.img
KERNEL_BIN           = $(if $(CHAINLOADER),$(CHAINLOADER_BIN),$(NORMAL_KERNEL_BIN))
KERNEL_ELF           = $(if $(CHAINLOADER),target/$(TARGET)/release/kernel,$(NORMAL_KERNEL_ELF))
JTAG_TARGET_DIR = target/jtag/$(BSP)
JTAG_OPT0_TARGET_DIR = target/jtag-opt0/$(BSP)
JTAG_BOOT_ELF = target/jtag-bootstrap/$(BSP)/jtag_boot.elf
GDB_INIT = tools/jtag/kernel.gdb
TEST_BUILD_DIR = target/test_build/$(BSP)
TEST_KERNEL_BIN = $(TEST_BUILD_DIR)/kernel8.img
HOST_TARGET = $(shell rustc -vV | sed -n 's/^host: //p')
HOST_TOOL_DIR = $(CURDIR)/target/$(HOST_TARGET)/release
TEST_RUNNER = $(HOST_TOOL_DIR)/kernel_test_runner
TEST_XTASK = $(HOST_TOOL_DIR)/xtask



##--------------------------------------------------------------------------------------------------
## Command building blocks
##--------------------------------------------------------------------------------------------------
RUSTFLAGS = $(RUSTC_MISC_ARGS)
RUSTFLAGS_PEDANTIC = $(RUSTFLAGS) \
    -D missing_docs
XTASK_RUSTFLAGS = FLAMINGOS_KERNEL_RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)"

KERNEL_FEATURES = bsp_$(BSP_FAMILY)
TEST_KERNEL_FEATURES = bsp_$(BSP_FAMILY),test_build
ifdef DEBUG_PRINTS
    KERNEL_FEATURES := $(KERNEL_FEATURES),debug_prints
    TEST_KERNEL_FEATURES := $(TEST_KERNEL_FEATURES),debug_prints
endif
ifdef CHAINLOADER
    KERNEL_FEATURES := $(KERNEL_FEATURES),chainloader
endif
ifeq ($(EMBEDDED_FS),1)
ifdef CHAINLOADER
$(error EMBEDDED_FS belongs to the kernel, not the chainloader)
endif
    KERNEL_FEATURES := $(KERNEL_FEATURES),embedded-fs
    TEST_KERNEL_FEATURES := $(TEST_KERNEL_FEATURES),embedded-fs
EMBEDDED_IMAGE = target/bootfs/$(BSP_FAMILY)/init.sparse
endif
FEATURES      = --features $(KERNEL_FEATURES)
TEST_FEATURES = --no-default-features --features $(TEST_KERNEL_FEATURES)
COMPILER_ARGS = --target=$(TARGET) \
    $(FEATURES)                    \
    --release

TEST_SELECTION = $(if $(TEST),--test $(TEST),--tests)
TEST_CMD = cargo test                            \
    --target=$(TARGET)                           \
    $(TEST_FEATURES)                             \
    --release                                    \
    --manifest-path $(KERNEL_MANIFEST)           \
    $(TEST_SELECTION)
XTASK_SOURCE = $(if $(filter 0,$(EMBEDDED_FS)),--sd-card,)
DOC_CMD     = cargo xtask doc $(BSP) $(XTASK_SOURCE) $(FEATURES)
HOST_CLIPPY_PACKAGES = kernel_test_runner xtask
CLIPPY_KERNEL_CMD = cargo clippy $(COMPILER_ARGS) --manifest-path $(KERNEL_MANIFEST)
CLIPPY_HOST_CMD = cargo clippy --release --target=$(HOST_TARGET) \
    $(addprefix --package=,$(HOST_CLIPPY_PACKAGES))
EXEC_QEMU              = $(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE)



##--------------------------------------------------------------------------------------------------
## Targets
##--------------------------------------------------------------------------------------------------
.PHONY: all chainboot jtagboot openocd gdb gdb-opt0 doc qemu test test_boot test_integration clippy clippy_kernel clippy_host clean readelf objdump nm check FORCE

all: $(KERNEL_BIN)

ifeq ($(EMBEDDED_FS),1)
$(EMBEDDED_IMAGE): FORCE
	@cargo xtask boot-image $(BSP)
clippy_kernel: $(EMBEDDED_IMAGE)
doc: $(EMBEDDED_IMAGE)
endif

ifndef CHAINLOADER
$(NORMAL_KERNEL_ELF): $(NORMAL_KERNEL_BIN)
endif

##------------------------------------------------------------------------------
## Send a payload to a chainloader already running on the target
##------------------------------------------------------------------------------
chainboot: $(if $(filter $(NORMAL_KERNEL_ELF),$(CHAINBOOT_PAYLOAD)),$(NORMAL_KERNEL_ELF))
	@test -f "$(CHAINBOOT_PAYLOAD)" || { \
		echo "Missing payload: $(CHAINBOOT_PAYLOAD)"; \
		exit 1; \
	}
	$(SCIP) --binfile "$(CHAINBOOT_PAYLOAD)" "$(DEV_SERIAL)" 921600 8 N 1 N

##------------------------------------------------------------------------------
## Real-hardware JTAG workflow
##------------------------------------------------------------------------------
jtagboot:
	@test -f "$(JTAG_BOOT_IMAGE)"
	@cargo xtask wrap-flat "$(JTAG_BOOT_IMAGE)" "$(JTAG_BOOT_ELF)"
	$(SCIP) --binfile "$(JTAG_BOOT_ELF)" "$(DEV_SERIAL)" 921600 8 N 1 N

openocd:
	$(OPENOCD) -f "$(OPENOCD_INTERFACE)" -f "$(OPENOCD_TARGET_CONFIG)"

# GDB loads the bootstrap at its physical entry and reads stage-2 symbols at
# their linked virtual addresses.
define debug_kernel
	@CARGO_PROFILE_RELEASE_STRIP=none RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" cargo rustc \
		--target=$(TARGET)                           \
		--no-default-features                       \
		--features bsp_$(BSP_FAMILY)                       \
		--release                                    \
		--target-dir=$(1)                            \
		--manifest-path $(KERNEL_MANIFEST)            \
		--bin kernel                                  \
		-- -C debuginfo=2 $(2)
	@rust-objcopy --strip-debug "$(1)/$(TARGET)/release/kernel" "$(1)/kernel.elf"
	@$(XTASK_RUSTFLAGS) cargo xtask package $(BSP) "$(1)/kernel.elf" "$(1)/kernel8.img" \
		--bootstrap-elf "$(1)/bootstrap.elf"
	$(GDB) -q -x "$(GDB_INIT)" -ex 'add-symbol-file "$(1)/$(TARGET)/release/kernel"' "$(1)/bootstrap.elf"
endef

gdb:
	$(call debug_kernel,$(JTAG_TARGET_DIR),)

gdb-opt0:
	$(call debug_kernel,$(JTAG_OPT0_TARGET_DIR),-C opt-level=0)

##------------------------------------------------------------------------------
## Build the kernel ELF and firmware image, or the serial chainloader
##------------------------------------------------------------------------------
$(KERNEL_BIN): FORCE
	$(call color_header, "Building boot artifacts - $(BSP)")
	@$(XTASK_RUSTFLAGS) cargo xtask build $(BSP) $(XTASK_SOURCE) $(FEATURES)
	$(call color_progress_prefix, "Name")
	@echo $(KERNEL_BIN)
	$(call color_progress_prefix, "Size")
	$(call disk_usage_KiB, $(KERNEL_BIN))

##------------------------------------------------------------------------------
## Generate the documentation
##------------------------------------------------------------------------------
doc:
	$(call color_header, "Generating docs")
	@$(DOC_CMD) --open

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
	@$(XTASK_RUSTFLAGS) CARGO_TARGET_DIR=$(TEST_BUILD_DIR) \
		cargo xtask build $(BSP) --out-dir $(TEST_BUILD_DIR) $(XTASK_SOURCE) --features=$(TEST_KERNEL_FEATURES)
	$(call color_header, "Running QEMU boot test")
	$(EXEC_QEMU) $(QEMU_TEST_ARGS) -kernel $(TEST_KERNEL_BIN)
endif

##------------------------------------------------------------------------------
## Run the stable, harness-free integration test kernels in QEMU
##------------------------------------------------------------------------------
ifeq ($(QEMU_MACHINE_TYPE),) # QEMU is not supported for the board.

test_integration:
	$(call color_header, "$(QEMU_MISSING_STRING)")

else # QEMU is supported.

# The runner executes a prebuilt xtask while Cargo holds the test build lock.
test_integration:
	$(call color_header, "Building kernel test tools")
	@cargo xtask test-init $(BSP)
	@cargo build --package kernel_test_runner --package xtask --release --target $(HOST_TARGET)
	$(call color_header, "Running QEMU integration tests")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)"                                              \
	CARGO_TARGET_AARCH64_UNKNOWN_NONE_SOFTFLOAT_RUNNER="$(TEST_RUNNER)"             \
	KERNEL_TEST_QEMU="$(QEMU_BINARY)"                                              \
	KERNEL_TEST_QEMU_ARGS="-M $(QEMU_MACHINE_TYPE) $(QEMU_TEST_ARGS)"                \
	KERNEL_TEST_XTASK="$(TEST_XTASK)"                                              \
	KERNEL_TEST_REPO_ROOT="$(CURDIR)"                                              \
	KERNEL_TEST_BSP="$(BSP)"                                                      \
	$(TEST_CMD)

endif

test: test_boot test_integration

##------------------------------------------------------------------------------
## Run Clippy for the kernel and native host tools
##------------------------------------------------------------------------------
clippy: clippy_kernel clippy_host

clippy_kernel:
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(CLIPPY_KERNEL_CMD)

clippy_host:
	@$(CLIPPY_HOST_CMD)

##------------------------------------------------------------------------------
## Clean
##------------------------------------------------------------------------------
clean:
	rm -rf target $(NORMAL_KERNEL_BIN) $(NORMAL_KERNEL_ELF) $(CHAINLOADER_BIN)

##------------------------------------------------------------------------------
## Run readelf
##------------------------------------------------------------------------------
readelf: $(KERNEL_BIN)
	$(call color_header, "Launching readelf")
	$(READELF_BINARY) --headers $(KERNEL_ELF)

##------------------------------------------------------------------------------
## Run objdump
##------------------------------------------------------------------------------
objdump: $(KERNEL_BIN)
	$(call color_header, "Launching objdump")
	$(OBJDUMP_BINARY) --disassemble --demangle \
                --section .text   \
                --section .rodata \
                $(KERNEL_ELF)

##------------------------------------------------------------------------------
## Run nm
##------------------------------------------------------------------------------
nm: $(KERNEL_BIN)
	$(call color_header, "Launching nm")
	$(NM_BINARY) --demangle --print-size $(KERNEL_ELF) | sort
