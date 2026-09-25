# Builds the `lr2021` native .mpy: the Rust driver core plus a thin shim.
#
# Xtensa, not Thumb: this driver's board is the Meshnology W12 (ESP32-S3R8), so
# the toolchain differs from the LR1121's in every one of the four knobs below.
# Build with:
#   . ~/export-esp.sh && make PYTHON=$(realpath -s ../../venv/bin/python)
# Both are mandatory — without export-esp.sh the compiler is not on PATH, and
# without PYTHON the system python3 has no pyelftools for mpy_ld.py.

MPY_DIR ?= ../../../circuitpython

MOD = lr2021
SRC = lr2021.c
ARCH = xtensawin

# Pulls in libgcc for the 64-bit division helper goodput_bps needs.
LINK_RUNTIME = 1

# dynruntime.mk hardcodes the original ESP32 prefix and resolves libgcc/libm at
# include time, so this has to win before then. The generic `xtensa-esp-elf-`
# driver must not be used: with no chip dynconfig it builds big-endian Xtensa.
override CROSS = xtensa-esp32s3-elf-

RUST_CRATE_DIR = ..
RUST_TARGET ?= xtensa-esp32s3-none-elf
# The `esp` fork, not plain nightly: upstream rustc has no Xtensa backend. It is
# an older cargo, so nothing here may use core features newer than it.
RUST_TOOLCHAIN ?= esp
RUST_LIB = $(RUST_CRATE_DIR)/target/$(RUST_TARGET)/release/liblr2021.a
RUST_OBJ = $(BUILD)/rust_lr2021.o

SRC_O += $(RUST_OBJ)

include $(MPY_DIR)/py/dynruntime.mk

# Extracted into its own directory because the crate's member name and the
# shim's own build/lr2021.o both match lr2021*.o. Both this crate and the shared
# lrxxxx-hal crate contribute members, so they are combined with `ld -r` rather
# than moved: SRC_O wants one object, and the hal may contribute none at all
# once it fully inlines.
#
# The panic shim is localized because -Zbuild-std gives the synthetic `__rustc`
# crate the same disambiguator in every build, so two crates built this way
# define the same symbol. Harmless while each is its own .mpy; a multiple
# definition once both are linked into one firmware image.
$(RUST_OBJ): $(RUST_LIB)
	$(ECHO) "AR $@"
	$(Q)rm -rf $(BUILD)/rust-obj && mkdir -p $(BUILD)/rust-obj
	$(Q)cd $(BUILD)/rust-obj && $(CROSS)ar x $(abspath $(RUST_LIB)) \
		$$($(CROSS)ar t $(abspath $(RUST_LIB)) | grep -E '^(lr2021|lrxxxx_hal)')
	$(Q)$(CROSS)ld -r -o $@ $(BUILD)/rust-obj/*.o
	$(Q)$(CROSS)objcopy --wildcard --localize-symbol='*rust_begin_unwind*' $@

$(RUST_LIB): $(wildcard $(RUST_CRATE_DIR)/src/*.rs) $(RUST_CRATE_DIR)/Cargo.toml \
             $(wildcard $(RUST_CRATE_DIR)/../lrxxxx-hal/src/*.rs)
	$(ECHO) "CARGO $(RUST_TARGET)"
	$(Q)cd $(RUST_CRATE_DIR) && cargo +$(RUST_TOOLCHAIN) build --release \
		-Zunstable-options \
		-Zbuild-std=core,compiler_builtins \
		-Zbuild-std-features=compiler-builtins-mem \
		--target $(RUST_TARGET)
