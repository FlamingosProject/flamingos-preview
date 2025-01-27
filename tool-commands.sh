cargo rustc --release --target=aarch64-unknown-none-softfloat --features=bsp_rpi3 --bin kernel -- -C link-arg=--library-path=src/bsp/raspberrypi/ -C link-arg=--script=kernel.ld
/bin/qemu-system-aarch64 -M raspi3b -serial stdio -display none -kernel kernel8.img
cargo objcopy --target=aarch64-unknown-none-softfloat --features=bsp_rpi3 -C link-arg=--library-path=src/bsp/raspberrypi/ -C link-arg=--script=kernel.ld --bin kernel -- -O binary kernel8.img
rust-objcopy --strip-all -O binary target/aarch64-unknown-none-softfloat/release/kernel kernel8.img
