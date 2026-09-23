## Goal

Monokernel with

  * multiple userspace processes
  * process loading from disk
  * processes running concurrently with preemption, 
  * process access to system resources

## Steps

  * add SD card support
  * add exfat filesystem crate
  * add ELF loader crate
  * add process creation
  * add preemption
  * add scheduler
  * split privilege to spawn in userspace
  * add syscalls
