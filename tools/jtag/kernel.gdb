set pagination off
target remote :3333
monitor halt
load
set $pc = _start
echo Kernel loaded and stopped at _start.\n
