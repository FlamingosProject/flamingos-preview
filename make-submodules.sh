cat <<'EOF' | 
02-runtime-init
03-hacky-hello-world
04-safe-globals
05-drivers-gpio-uart
06-uart-chainloader
EOF
while read b
do
    git submodule add -b $b ssh://git@github.com/RustOS2/rust-raspberrypi-OS-tutorials $b
done
