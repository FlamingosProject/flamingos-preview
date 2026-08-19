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
CHAINLOADER_BIN      = chainloader8.img
KERNEL_BIN           = $(if $(CHAINLOADER),$(CHAINLOADER_BIN),$(NORMAL_KERNEL_BIN))
BUILD_MODE           = $(if $(CHAINLOADER),chainloader,kernel)
LAST_BUILD_CONFIG    = target/$(BSP).$(BUILD_MODE).$(DEBUG_PRINTS).build_config

KERNEL_ELF_RAW = target/$(TARGET)/release/kernel
JTAG_TARGET_DIR = target/jtag/$(BSP)
JTAG_OPT0_TARGET_DIR = target/jtag-opt0/$(BSP)
JTAG_ELF = $(JTAG_TARGET_DIR)/$(TARGET)/release/kernel
JTAG_OPT0_ELF = $(JTAG_OPT0_TARGET_DIR)/$(TARGET)/release/kernel
GDB_INIT = tools/jtag/kernel.gdb
HOST_TARGET     = $(shell rustc -vV | sed -n 's/^host: //p')
TEST_RUNNER     = $(shell pwd)/target/$(HOST_TARGET)/release/kernel_test_runner
# This parses cargo's dep-info file.
# https://doc.rust-lang.org/cargo/guide/build-cache.html#dep-info-files
KERNEL_ELF_RAW_DEPS = $(filter-out %: ,$(file < $(KERNEL_ELF_RAW).d)) $(KERNEL_MANIFEST) $(LAST_BUILD_CONFIG)

##------------------------------------------------------------------------------
## Translation tables
##------------------------------------------------------------------------------
TT_TOOL_PATH = tools/bin

KERNEL_ELF_TTABLES      = target/$(TARGET)/release/kernel+ttables
KERNEL_ELF_TTABLES_DEPS = $(KERNEL_ELF_RAW) $(EXEC_TT_TOOL)

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
ifdef CHAINLOADER
KERNEL_ELF = $(KERNEL_ELF_RAW)
endif
TEST_BUILD_DIR = target/test_build/$(BSP)
TEST_KERNEL_BIN = $(TEST_BUILD_DIR)/kernel8.img
HOST_TARGET = $(shell rustc -vV | sed -n 's/^host: //p')
TEST_RUNNER = $(shell pwd)/target/$(HOST_TARGET)/release/kernel_test_runner
TEST_TT_TOOL = $(shell pwd)/target/$(HOST_TARGET)/release/translation_table_tool
TEST_SYMBOLS_TOOL = $(shell pwd)/target/$(HOST_TARGET)/release/kernel-elf-symbol



##--------------------------------------------------------------------------------------------------
## Command building blocks
##--------------------------------------------------------------------------------------------------
RUSTFLAGS = $(RUSTC_MISC_ARGS)
RUSTFLAGS_PEDANTIC = $(RUSTFLAGS) \
    -D missing_docs

KERNEL_FEATURES = bsp_$(BSP_FAMILY)
TEST_KERNEL_FEATURES = bsp_$(BSP_FAMILY),test_build
ifdef DEBUG_PRINTS
    KERNEL_FEATURES := $(KERNEL_FEATURES),debug_prints
    TEST_KERNEL_FEATURES := $(TEST_KERNEL_FEATURES),debug_prints
endif
ifdef CHAINLOADER
    KERNEL_FEATURES := $(KERNEL_FEATURES),chainloader
endif
FEATURES      = --features $(KERNEL_FEATURES)
TEST_FEATURES = --no-default-features --features $(TEST_KERNEL_FEATURES)
COMPILER_ARGS = --target=$(TARGET) \
    $(FEATURES)                    \
    --release

# build-std can be skipped for helper commands that do not rely on correct stack frames and other
# custom compiler options. This results in a huge speedup.
RUSTC_CMD   = cargo rustc $(COMPILER_ARGS) --manifest-path $(KERNEL_MANIFEST)
TEST_SELECTION = $(if $(TEST),--test $(TEST),--tests)
TEST_CMD = cargo test                            \
    --target=$(TARGET)                           \
    $(TEST_FEATURES)                             \
    --release                                    \
    --manifest-path $(KERNEL_MANIFEST)           \
    $(TEST_SELECTION)
DOC_CMD     = cargo doc $(COMPILER_ARGS)
HOST_CLIPPY_PACKAGES = kernel_test_runner translation_table_tool kernel_symbols_tool xtask
CLIPPY_KERNEL_CMD = cargo clippy $(COMPILER_ARGS) --manifest-path $(KERNEL_MANIFEST)
CLIPPY_HOST_CMD = cargo clippy --release --target=$(HOST_TARGET) \
    $(addprefix --package=,$(HOST_CLIPPY_PACKAGES))
OBJCOPY_CMD = rust-objcopy \
    --strip-all            \
    -O binary

EXEC_QEMU              = $(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE)
EXEC_TT_TOOL           = $(TT_TOOL_PATH)/translation_table_tool
EXEC_KERNEL_SYMBOLS_TOOL = target/release/kernel-elf-symbol



##--------------------------------------------------------------------------------------------------
## Targets
##--------------------------------------------------------------------------------------------------
.PHONY: all chainboot jtagboot openocd gdb gdb-opt0 doc qemu test test_boot test_integration clippy clippy_kernel clippy_host clean readelf objdump nm check FORCE

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
## Real-hardware JTAG workflow
##------------------------------------------------------------------------------
jtagboot:
	@test -f "$(JTAG_BOOT_IMAGE)"
	$(SCIP) --binfile "$(JTAG_BOOT_IMAGE)" "$(DEV_SERIAL)" 921600 8 N 1 N

openocd:
	$(OPENOCD) -f "$(OPENOCD_INTERFACE)" -f "$(OPENOCD_TARGET_CONFIG)"

define build_jtag_elf
	@cargo build --package translation_table_tool --release
	@cargo build --package kernel_symbols_tool --release --bin kernel-elf-symbol
	@CARGO_PROFILE_RELEASE_STRIP=none RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" cargo rustc \
		--target=$(TARGET)                           \
		--no-default-features                       \
		--features bsp_$(BSP_FAMILY)                       \
		--release                                    \
		--target-dir=$(1)                            \
		--manifest-path $(KERNEL_MANIFEST)            \
		--bin kernel                                  \
		-- -C debuginfo=2 $(2)
	@target/release/translation_table_tool $(BSP_FAMILY) $(if $(filter $(1),$(JTAG_TARGET_DIR)),$(JTAG_ELF),$(JTAG_OPT0_ELF))
	@KERNEL_SYMBOLS_INPUT_ELF=$(if $(filter $(1),$(JTAG_TARGET_DIR)),$(JTAG_ELF),$(JTAG_OPT0_ELF)) \
	KERNEL_SYMBOLS_OUTPUT_ELF=$(if $(filter $(1),$(JTAG_TARGET_DIR)),$(JTAG_ELF),$(JTAG_OPT0_ELF)).symbols \
	$(MAKE) --no-print-directory -f kernel_symbols.mk
	@mv $(if $(filter $(1),$(JTAG_TARGET_DIR)),$(JTAG_ELF),$(JTAG_OPT0_ELF)).symbols \
		$(if $(filter $(1),$(JTAG_TARGET_DIR)),$(JTAG_ELF),$(JTAG_OPT0_ELF))
	@rm -f $(if $(filter $(1),$(JTAG_TARGET_DIR)),$(JTAG_ELF),$(JTAG_OPT0_ELF))_symbols.rs
endef

gdb:
	$(call build_jtag_elf,$(JTAG_TARGET_DIR),)
	$(GDB) -q -x "$(GDB_INIT)" "$(JTAG_ELF)"

gdb-opt0:
	$(call build_jtag_elf,$(JTAG_OPT0_TARGET_DIR),-C opt-level=0)
	$(GDB) -q -x "$(GDB_INIT)" "$(JTAG_OPT0_ELF)"

##------------------------------------------------------------------------------
## Save the configuration as a file, so make understands if it changed.
##------------------------------------------------------------------------------
$(LAST_BUILD_CONFIG):
	@rm -f target/*.build_config
	@mkdir -p target
	@touch $(LAST_BUILD_CONFIG)

##------------------------------------------------------------------------------
## Build the host-side translation-table patcher.
##------------------------------------------------------------------------------
$(EXEC_TT_TOOL): FORCE
	@cargo objcopy -p translation_table_tool --release -- $(EXEC_TT_TOOL)

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
	$(EXEC_TT_TOOL) $(BSP_FAMILY) $$TMP && \
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
## Run a deterministic boot smoke test in QEMU
##------------------------------------------------------------------------------
ifeq ($(QEMU_MACHINE_TYPE),) # QEMU is not supported for the board.

test_boot:
	$(call color_header, "$(QEMU_MISSING_STRING)")

else # QEMU is supported.

test_boot:
	$(call color_header, "Building QEMU boot test")
	@mkdir -p $(TEST_BUILD_DIR)
	@CARGO_TARGET_DIR=$(TEST_BUILD_DIR) cargo xtask build $(BSP) --features=test_build
	@mv $(KERNEL_BIN) $(TEST_KERNEL_BIN)
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

test_integration:
	$(call color_header, "Building kernel test tools")
	@cargo build --package kernel_test_runner --release --target $(HOST_TARGET)
	@cargo build --package translation_table_tool --release --target $(HOST_TARGET)
	@cargo build --package kernel_symbols_tool --release --target $(HOST_TARGET)
	$(call color_header, "Running QEMU integration tests")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)"                                             \
	CARGO_TARGET_AARCH64_UNKNOWN_NONE_SOFTFLOAT_RUNNER="$(TEST_RUNNER)"            \
	KERNEL_TEST_QEMU="$(QEMU_BINARY)"                                               \
	KERNEL_TEST_QEMU_ARGS="-M $(QEMU_MACHINE_TYPE) $(QEMU_TEST_ARGS)"               \
	KERNEL_TEST_OBJCOPY="rust-objcopy"                                              \
	KERNEL_TEST_TT_TOOL="$(TEST_TT_TOOL)"                                           \
	KERNEL_TEST_SYMBOLS_TOOL="$(TEST_SYMBOLS_TOOL)"                                 \
	KERNEL_TEST_TARGET="$(TARGET)"                                                  \
	KERNEL_TEST_REPO_ROOT="$(shell pwd)"                                             \
	KERNEL_TEST_BSP="$(BSP_FAMILY)"                                                        \
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
