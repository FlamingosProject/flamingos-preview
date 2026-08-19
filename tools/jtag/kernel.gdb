set pagination off
target remote :3333
monitor halt
load
set $pc = 0x80000
echo ELF bootstrap loaded and stopped at its physical entry.\n
