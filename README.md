# Tutorial 15 - Virtual Memory Part 3: Precomputed Translation Tables

## tl;dr

- We are making the next baby-steps towards mapping the kernel to the most significant area of the
  virtual memory space.
- Instead of dynamically computing the kernel's translation tables during runtime while booting, we
  are precomputing them in advance just after kernel compilation, and patch them into the kernel's
  binary ahead of time.
- For now, we are still `identity-mapping` the kernel binary.
  - However, after this tutorial, we have all the infrastructure in place to easily map it
    elsewhere.

## Table of Contents

- [Tutorial 15 - Virtual Memory Part 3: Precomputed Translation Tables](#tutorial-15---virtual-memory-part-3-precomputed-translation-tables)
  - [tl;dr](#tldr)
  - [Table of Contents](#table-of-contents)
  - [Introduction](#introduction)
  - [When Load Address != Link Address, Funny Things Can Happen](#when-load-address--link-address-funny-things-can-happen)
    - [Interim Conclusion](#interim-conclusion)
    - [Doing the Same Thing - Expecting Different Results](#doing-the-same-thing---expecting-different-results)
  - [Position-Independent Code (PIC)](#position-independent-code-pic)
    - [Using PIC during kernel startup](#using-pic-during-kernel-startup)
  - [Precomputed Translation Tables](#precomputed-translation-tables)
  - [Implementation](#implementation)
    - [Preparing the Kernel Tables](#preparing-the-kernel-tables)
    - [Turning on the MMU Before Switching to EL1](#turning-on-the-mmu-before-switching-to-el1)
    - [The Translation Table Tool](#the-translation-table-tool)
    - [Other changes](#other-changes)
  - [Discussion](#discussion)
  - [Test it](#test-it)

## Introduction

This tutorial is another preparatory step for our overall goal of mapping the kernel to the most
significant area of the virtual memory space.

The reasoning of why we want to do this was given in the previous tutorial's introduction. But lets
for a quick moment think about what it actually means in practice: Currently, the kernel's binary is
loaded by the Raspberry's firmware at address `0x8_0000`.

In decimal, this address is at `512 KiB`, and therefore well within the _least significant part_ of
the address space. Let's have a look at the picture from the [ARM Cortex-A Series Programmer’s Guide
for ARMv8-A] again to understand in which virtual address space region the kernel would ideally be
mapped to:

<p align="center">
    <img src="../doc/15_kernel_user_address_space_partitioning.png" height="500" align="center">
</p>

As we can see, the architecture proposes somewhere between addresses `0xffff_0000_0000_0000` and
`0xffff_ffff_ffff_ffff`. Once we succeed in mapping the kernel there, the whole lower range between
`0x0` and `0xffff_ffff_ffff` would be free for future applications to use.

[ARM Cortex-A Series Programmer’s Guide for ARMv8-A]: https://developer.arm.com/documentation/den0024/latest/

Now, how can we get there?

## When Load Address != Link Address, Funny Things Can Happen

Imagine that, using the linker script, we link the kernel so that its `_start()` function is located
at address `0xffff_0000_0000_0000`. What hasn't changed is that the Raspberry's firmware will still
load the kernel binary at address `0x8_0000`, and the kernel will still start executing from there
with the `MMU` disabled.

So one of the very first things the kernel must achieve during its boot to function correctly, is to
somehow enable the `MMU` together with `translation tables` that account for the address offset
(`0xffff_0000_0000_0000 -> 0x8_0000`). In previous tutorials, we already generated translation
tables during the kernel's boot, so lets quickly remember how we did that:

In `src/bsp/__board_name__/memory/mmu.rs` we have a static (or "global" in non-Rust speak) instance
of `struct KernelTranslationTable`:

```rust
static KERNEL_TABLES: InitStateLock<KernelTranslationTable> =
    InitStateLock::new(KernelTranslationTable::new());
```

In other parts of the kernel code, this instance would be referenced one way or the other, and its
member functions would be called, for example, when mapping a range of pages. At the end of the day,
after multiple layers of indirection, what happens at the most basic level is that a `piece of code`
manipulates some `global data`. So part of the job of the code is to retrieve the data's constant
address before it can manipulate it.

Let's simplify the address-retrieval to the most basic code example possible. The example will be
presented as `C` code. Don't ask yet why `C` is chosen. It will get clear as the tutorial develops.

```c
#include <stdint.h>

uint64_t global_data_word = 0x11223344;

uint64_t* get_address_of_global(void) {
     return &global_data_word;
}
```

Let's compile and link this using the following linker script:

```ld.s
SECTIONS
{
    . =  0x80000;

    .text : {
        QUAD(0); /* Intentional fill word */
        QUAD(0); /* Intentional fill word */
        KEEP(*(.text*))
    }
    .got    : ALIGN(8)   { *(.got) }
    .data   : ALIGN(64K) {
        QUAD(0); /* Intentional fill word */
        *(.data*)
    }
}
```

Here are the compilation steps and the corresponding `objdump` for `AArch64`:

```console
$ clang --target=aarch64-none-elf -Iinclude -Wall -c start.c -o start.o
$ ld.lld start.o -T kernel.ld -o example.elf
```

```c-objdump
Disassembly of section .text:

0000000000080010 get_address_of_global:
   80010: 80 00 00 90                  	adrp	x0, #0x10000
   80014: 00 20 00 91                  	add	x0, x0, #0x8
   80018: c0 03 5f d6                  	ret

Disassembly of section .data:

0000000000090008 global_data_word:
   90008: 44 33 22 11
   9000c: 00 00 00 00
```

As you can see, the address of function `get_address_of_global()` is `0x8_0010` and
`global_data_word` got address `0x9_0008`. In the function body, the compiler emitted an [`ADRP`]
and `ADD` instruction pair, which means that the global's address is calculated as a `PC-relative
offset`. `PC` means program counter, aka the current position of where the CPU core is currently
executing from.

Without going in too much detail, what the instruction basically does is: It retrieves the `4 KiB`
page address that belongs to the program counter's (PC) current position (PC is at `0x8_0010`, so
the page address is `0x8_0000`), and adds `0x1_0000`. So after the `ADRP` instruction, register `x0`
holds the value `0x9_0000`. To this value, `8` is added in the next instruction, resulting in the
overall address of `0x9_0008`, which is exactly where `global_data_word` is located. This works,
because after linking a `static executable binary` like we do since `tutorial 01`, relative
positions of code and data are fixed, and not supposed to change during runtime.

[`ADRP`]: https://developer.arm.com/documentation/dui0802/b/A64-General-Instructions/ADRP

If the Raspberry's firmware now loads this binary at address `0x8_0000`, as always, we can be sure
that our function returns the correct address of our global data word.

Now lets link this to the most significant area of memory:

```ld.s
SECTIONS
{
    . =  0xffff000000000000; /* <--- Only line changed in the linker script! */

    .text : {

    /* omitted for brevity */
}
```

And compile again:

```c-objdump
Disassembly of section .text:

ffff000000000010 get_address_of_global:
ffff000000000010: 80 00 00 90          	adrp	x0, #0x10000
ffff000000000014: 00 20 00 91          	add	x0, x0, #0x8
ffff000000000018: c0 03 5f d6          	ret

Disassembly of section .data:

ffff000000010008 global_data_word:
ffff000000010008: 44 33 22 11
ffff00000001000c: 00 00 00 00
```

And let the Raspberry's firmware load the binary at address `0x8_0000` again (we couldn't load it to
`0xffff_0000_0000_0000` even if we wanted to. That address is `15 Exbibyte`. A Raspberry Pi with
that much RAM won't exist for some time to come 😉).

Let's try to answer the same question again: Would `get_address_of_global()` return the value for
`global_data_word` that we expect to see (`0xffff_0000_0001_0008` as shown in the objdump)? This
time, the answer is **no**. It would again return `0x9_0008`.

Why is that? Don't let yourself be distracted by the addresses the `objdump` above is showing. When
the Raspberry's firmware loads this binary at `0x8_0000`, then the Program Counter value when
`get_address_of_global()` executes is again `0x8_0010`. So **the PC-relative calculation** will not
result in the expected value, which would be the **absolute** (alternatively: **link-time**) address
of `global_data_word`.

### Interim Conclusion

What have we learned so far? We wrote a little piece of code in a high-level language that retrieves
an address, and we naively expected to retrieve an **absolute** address.

But compiler and linker conspired against us, and machine code was emitted that uses a PC-relative
addressing scheme, so our expectation is not matched when **load address != link address**. If you
compile for `AArch64`, you'll see relative addressing schemes a lot, because it is natural to the
architecture.

If you now say: Wait a second, how is this a problem? It actually helps! After all, since the code
is loaded at address `0x8_0000`, this relative addressing scheme will ensure that the processor
accesses the global data word at the correct address!

Yes, in this particular, constrained demo case, it worked out for us. But have a look at the
following.

### Doing the Same Thing - Expecting Different Results

Let's take a quick detour and see what happens if we compile **the exactly same code** for the
`x86_64` processor architecture. First when linked to `0x8_0000`:

```c-objdump
Disassembly of section .text:

0000000000080070 get_address_of_global:
   80070: 55                            push    rbp
   80071: 48 89 e5                      mov     rbp, rsp
   80074: 48 b8 08 00 09 00 00 00 00 00 movabs  rax, 0x90008
   8007e: 5d                            pop     rbp
   8007f: c3                            ret

Disassembly of section .data:

ffff000000010008 global_data_word:
ffff000000010008: 44 33 22 11
ffff00000001000c: 00 00 00 00
```

And now linked to `0xffff_0000_0000_0000`:

```c-objdump
Disassembly of section .text:

ffff000000000070 get_address_of_global:
ffff000000000070: 55                            push    rbp
ffff000000000071: 48 89 e5                      mov     rbp, rsp
ffff000000000074: 48 b8 08 00 01 00 00 00 ff ff movabs  rax, 0xffff000000010008
ffff00000000007e: 5d                            pop     rbp
ffff00000000007f: c3                            ret

Disassembly of section .data:

ffff000000010008 global_data_word:
ffff000000010008: 44 33 22 11
ffff00000001000c: 00 00 00 00
```

Both times, the `movabs` instruction gets emitted. It means that the address is put into the target
register using hardcoded `immediate values`. PC-relative address calculation is not used here.
Hence, this code would return the `absolute` address in both cases. Which means in the second case,
even when the binary would be loaded at `0x8_0000`, the return value would be
`0xffff_0000_0001_0008`.

**In summary, we get a different result for the same piece of `C` code, depending on the target
processor architecture**. What do we learn from this little detour?

First, you cannot naively compile and run `Rust` or `C` statically linked binaries when there will
be a **load address != link address** situation. You'll run into undefined behavior very fast. It is
kinda expected and obvious, but hopefully it helped to see it fail in action.

Furthermore, it is important to understand that there are of course ways to load a symbol's absolute
address into `AArch64` registers using `immediate values` as well. Likewise, you can also do
PC-relative addressing in `x86`. We just looked at a tiny example. Maybe the next line of code would
be compiled into the opposite behavior on the two architectures, so that the `x86` code would do a
PC-relative calculation while the `AArch64` code goes for absolute.

At the end of the day, what is needed to solve our task at hand (bringup of virtual memory, while
being linked to one address and executing from another), is tight control over the machine
instructions that get emitted for **those pieces of code** that generate the `translation tables`
and enable the `MMU`.

What we need is called [position-independent code].

[position-independent code]: https://en.wikipedia.org/wiki/Position-independent_code

> Much low-level stuff in this tutorial, isn't it? This was a lot to digest already, but we're far
> from finished. So take a minute or two and clear your mind before we continue. 🧘

## Position-Independent Code (PIC)

As describend by Wikipedia, position-independent code

> is a body of machine code that, being placed somewhere in the primary memory, **executes
> properly** regardless of its absolute address.

Your safest bet is to write the pieces that need to be position-independent in `assembly`, because
this gives you full control over when relative or absolute addresses are being generated or used.
You will see this approach often in big projects like the Linux kernel, for example. The downside of
that approach is that the programmer needs good domain knowledge.

If you feel more adventurous and don't want to go completely without high-level code, you can try to
make use of suitable compiler flags such as `-fpic`, and only use `assembly` where absolutely
needed. Here is the [`-fpic` description for GCC]:

> -fpic
>
> Generate position-independent code (PIC) suitable for use in a shared library, if supported for
> the target machine. Such code accesses all constant addresses through a global offset table (GOT).
> The dynamic loader resolves the GOT entries when the program starts (the dynamic loader is not
> part of GCC; it is part of the operating system).

[`-fpic` description for GCC]: https://gcc.gnu.org/onlinedocs/gcc/Code-Gen-Options.html#Code-Gen-Options

However, it is very important to understand that this flag **is not** a ready-made solution for our
particular problem (and wasn't invented for that case either). There is a hint in the quoted text
above that gives it away: "_The dynamic loader resolves the GOT entries when the program starts (the
dynamic loader is not part of GCC; it is part of the operating system)_".

Well, we are a booting kernel, and not some (userspace) program running on top of an operating
system. Hence, there is no dynamic loader available. However, it is still possible to benefit from
`-fpic` even in our case. Lets have a look at what happens if we compile the earlier piece of `C`
code for `AArch64` using `-fpic`, still linking the output to the most signifcant part of the memory
space:

```console
$ clang --target=aarch64-none-elf -Iinclude -Wall -fpic -c start.c -o start.o
$ ld.lld start.o -T kernel.ld -o example.elf
```

```c-objdump
Disassembly of section .text:

ffff000000000010 get_address_of_global:
ffff000000000010: 00 00 00 90          	adrp	x0, #0x0
ffff000000000014: 00 28 40 f9          	ldr	x0, [x0, #0x50]
ffff000000000018: c0 03 5f d6          	ret

Disassembly of section .got:

ffff000000000050 .got:
ffff000000000050: 08 00 01 00
ffff000000000054: 00 00 ff ff

Disassembly of section .data:

ffff000000010008 global_data_word:
ffff000000010008: 44 33 22 11
ffff00000001000c: 00 00 00 00
```

What changed compared to earlier is that `get_address_of_global()` now indirects through the `Global
Offset Table`, as has been promised by the compiler's documentation. Specifically,
`get_address_of_global()` addresses the `GOT` using PC-relative addressing (distance from code to
`GOT` must always be fixed), and loads the first 64 bit word from the start of the `GOT`, which
happens to be `0xffff_0000_0001_0008`.

Okay okay okay... So when we use `-fpic`, we get the **absolute** address of `global_data_word` even
on `AArch64` now. How does this help when the code executes from `0x8_0000`?

Well, this is the part where the `dynamic loader` quoted above would come into picture if this was a
userspace program: "_The dynamic loader resolves the GOT entries when the program starts_". The
`-fpic` flag is normally used to compile _shared libraries_. Suppose we have a program that uses one
or more shared library. For various reasons, it happens that the shared library is loaded at a
different address than where the userspace program would initially expect it. In our example,
`global_data_word` could be supplied by such a shared library, and the userspace program is only
referencing it. The dynamic loader would know where the shared library was loaded into memory, and
therefore know the real address of `global_data_word`. So before the userspace program starts, the
loader would overwrite the `GOT` entry with the correct location. Et voilà, the compiled high-level
code would execute properly.

### Using PIC during kernel startup

If you think about it, our problem is a special case of what we just learned. We have a single
statically linked binary, where everything is dislocated by a fixed offset. In our case, it is
`0xffff_0000_0000_0000 - 0x8_0000 = 0x0fff_efff_ffff8_0000`. If we write some PIC-`assembly` code
which loops over the `GOT` and subtracts `0x0fff_efff_ffff8_0000` from every entry as the very first
thing when our kernel boots, any high-level code compiled with `-fpic` would work correctly
afterwards.

Moreover, this approach would be portable! Here's the output of our code compiled with `-fpic` for
`x86_64`:

```c-objdump
Disassembly of section .text:

ffff000000000070 get_address_of_global:
ffff000000000070: 55                    push    rbp
ffff000000000071: 48 89 e5              mov     rbp, rsp
ffff000000000074: 48 8b 05 2d 00 00 00  mov     rax, qword ptr [rip + 0x2d]
ffff00000000007b: 5d                    pop     rbp
ffff00000000007c: c3                    ret
ffff00000000007d: 0f 1f 00              nop     dword ptr [rax]

Disassembly of section .got:

ffff0000000000a8 .got:
ffff0000000000a8: 08 00 01 00
ffff0000000000ac: 00 00 ff ff

Disassembly of section .data:

ffff000000010008 global_data_word:
ffff000000010008: 44 33 22 11
ffff00000001000c: 00 00 00 00
```

As you can see, the `x86_64` code indirects through the `GOT` now same as the `AArch64` code.

Of course, indirecting through the `GOT` would be detrimental to performance, so you would restrict
`-fpic` compilation only to the code that is needed to enable the `MMU`. Everything else can be
compiled `non-relocatable` as always, because the translation tables naturally resolve the **load
address != link address** situation once they are live.

With `C/C++` compilers, this can be done rather easily. The compilers support compilation of
PIC-code on a per-[translation-unit] basis. Think of it as telling the compiler to compile this `.c`
file as `PIC`, but this other `.c` file not.

[translation-unit]: https://en.wikipedia.org/wiki/Translation_unit_(programming)

With `Rust`, unfortunately, the [relocation model] can only be set on a per-`crate` basis at the
moment (IINM), so that makes it difficult for us to put this approach to use.

[relocation model]: https://doc.rust-lang.org/rustc/codegen-options/index.html#relocation-model

## Precomputed Translation Tables

As we have just seen, going the `-fpic` way isn't a feasible solution at the time of writing this
text. On the other hand, writing the code to set up the initial page tables in `assembly` isn't that
attractive either, because writing larger pieces of assembly is an error-prone and delicate task.

Fortunately, there is a third way. We are writing an embedded kernel, and therefore the execution
environment is way more static and deterministic as compared to a general-purpose kernel that can be
deployed on a wide variety of targets. Specifically, for the Raspberrypi, we exactly know the **load
address** of the kernel in advance, and we know about the capabilities of the `MMU`. So there is
nothing stopping us from precomputing the kernel's translation tables ahead of time.

A disadvantage of this approach is an increased binary size, but this is not a deal breaker in our
case.

## Implementation

As stated in the initial `tl;dr`, we're not yet mapping the kernel to the most significant area of
virtual memory. This tutorial will keep the binary `identity-mapped`, and focuses only on the
infrastructure changes which enable the kernel to use `precomputed translation tables`. The actual
switch to high memory will happen in the next tutorial.

The changes needed are as follows:

1. Make preparations so that precomputed tables are supported by the kernel's memory subsystem code.
2. Change the boot code of the kernel so that the `MMU` is enabled with the precomputed tables as
   soon as possible.
3. Write a `translation table tool` that precomputes the translation tables from the generated
   `kernel.elf` file, and patches the tables back into the same.

### Preparing the Kernel Tables

The tables must be linked into the `.data` section now so that they become part of the final binary.
This is ensured using an attribute on the table's instance definition in
`bsp/__board_name__/memory/mmu.rs`:

```rust
#[link_section = ".data"]
#[no_mangle]
static KERNEL_TABLES: InitStateLock<KernelTranslationTable> =
    InitStateLock::new(KernelTranslationTable::new_for_precompute());
```

The `new_for_precompute()` is a new constructor in the the respective `_arch` code that ensures some
struct members that are not the translation table entries themselves are initialized properly for
the precompute use-case. The additional `#[no_mangle]` is added because we will need to parse the
symbol from the `translation table tool`, and this is easier with unmangled names.

In the `BSP` code, there is also a new file called `kernel_virt_addr_space_size.ld`, which contains
the kernel's virtual address space size. This file gets included in both, the `kernel.ld` linker
script and `mmu.rs`. We need this value both as a symbol in the kernel's ELF (for the `translation
table tool` to parse it later) and as a constant in the `Rust` code. This inclusion approach is just
a convenience hack that turned out working well.

One critical parameter that the kernel's boot code needs in order to enable the precomputed tables
is the `translation table base address` which must be programmed into the MMU's `TTBR` register. To
make it accessible easily, it is added to the `.text._start_arguments` section. The definition is
just below the definition of the kernel table instance in the `BSP` code:

```rust
/// This value is needed during early boot for MMU setup.
///
/// This will be patched to the correct value by the "translation table tool" after linking. This
/// given value here is just a dummy.
#[link_section = ".text._start_arguments"]
#[no_mangle]
static PHYS_KERNEL_TABLES_BASE_ADDR: u64 = 0xCCCCAAAAFFFFEEEE;
```

### Turning on the MMU Before Switching to EL1

Since the Raspberry Pi starts execution in the `EL2` privilege level, one of the first things we do
during boot since `tutorial 09` is to context-switch to the appropriate `EL1`. The `EL2` boot code
is a great place to set up virtual memory for `EL1`. It will allow execution in `EL1` to start with
virtual memory enabled since the very first instruction. The tweaks to `boot.s` are minimal:

```asm
// Load the base address of the kernel's translation tables.
ldr	x0, PHYS_KERNEL_TABLES_BASE_ADDR // provided by bsp/__board_name__/memory/mmu.rs

// Set the stack pointer. This ensures that any code in EL2 that needs the stack will work.
ADR_REL	x1, __boot_core_stack_end_exclusive
mov	sp, x1

// Jump to Rust code. x0 and x1 hold the function arguments provided to _start_rust().
b	_start_rust
```

In addition to the stack's address, we are now reading _the value_ of
`PHYS_KERNEL_TABLES_BASE_ADDR`. The `ldr` instruction addresses the value-to-be-read using a
PC-relative offset, so this is a `position-independent` operation and will therefore be future
proof. The retrieved value is supplied as an argument to function `_start_rust()`, which is defined
in `_arch/__arch_name__/cpu/boot.rs`:

```rust
#[no_mangle]
pub unsafe extern "C" fn _start_rust(
    phys_kernel_tables_base_addr: u64,
    phys_boot_core_stack_end_exclusive_addr: u64,
) -> ! {
    prepare_el2_to_el1_transition(phys_boot_core_stack_end_exclusive_addr);

    // Turn on the MMU for EL1.
    let addr = Address::new(phys_kernel_tables_base_addr as usize);
    memory::mmu::enable_mmu_and_caching(addr).unwrap();

    // Use `eret` to "return" to EL1. This results in execution of kernel_init() in EL1.
    asm::eret()
}
```

You can also see that we now turn on the `MMU` just before returning to `EL1`. That's basically it
already, the only missing piece that's left is the offline computation of the translation tables.

### The Translation Table Tool

The tool for translation table computation is located in the folder
`$ROOT/tools/translation_table_tool`. For ease of use, it is written in `Ruby` 💎. The code is
organized into `BSP` and `arch` parts just like the kernel's `Rust` code, and also has a class for
processing the kernel `ELF` file:

```console
$ tree tools/translation_table_tool

tools/translation_table_tool
├── arch.rb
├── bsp.rb
├── generic.rb
├── kernel_elf.rb
└── main.rb

0 directories, 5 files
```

Especially the `arch` part, which deals with compiling the translation table entries, will contain
some overlap with the `Rust` code present in `_arch/aarch64/memory/mmu/translation_table.rs`. It
might have been possible to write this tool in Rust as well, and borrow/share these pieces of code
with the kernel. But in the end, I found it not worth the effort for the few lines of code.

In the `Makefile`, the tool is invoked after compiling and linking the kernel, and before the
`stripped binary` is generated. It's command line arguments are the target `BSP` type and the path
to the kernel's `ELF` file:

```Makefile
TT_TOOL_PATH = tools/translation_table_tool

KERNEL_ELF_RAW      = target/$(TARGET)/release/kernel
# [...]

KERNEL_ELF_TTABLES      = target/$(TARGET)/release/kernel+ttables
# [...]

EXEC_TT_TOOL       = ruby $(TT_TOOL_PATH)/main.rb
# [...]

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
	@cp $(KERNEL_ELF_RAW) $(KERNEL_ELF_TTABLES)
	@$(DOCKER_TOOLS) $(EXEC_TT_TOOL) $(TARGET) $(BSP) $(KERNEL_ELF_TTABLES)

##------------------------------------------------------------------------------
## Generate the stripped kernel binary
##------------------------------------------------------------------------------
$(KERNEL_BIN): $(KERNEL_ELF_TTABLES)
	$(call color_header, "Generating stripped binary")
	@$(OBJCOPY_CMD) $(KERNEL_ELF_TTABLES) $(KERNEL_BIN)
```

In `main.rb`, the `KERNEL_ELF` instance for handling the `ELF` file is created first, followed by
`BSP` and `arch` parts:

```ruby
KERNEL_ELF = KernelELF.new(kernel_elf_path)

BSP = case BSP_TYPE
      when :rpi3, :rpi4
          RaspberryPi.new
      else
          raise
      end

TRANSLATION_TABLES = case KERNEL_ELF.machine
                     when :AArch64
                         Arch::ARMv8::TranslationTable.new
                     else
                         raise
                     end

kernel_map_binary
```

Finally, the function `kernel_map_binary` is called, which kicks of a sequence of interactions
between the `KERNEL_ELF`, `BSP` and `TRANSLATION_TABLES` instances:

```ruby
def kernel_map_binary
    mapping_descriptors = KERNEL_ELF.generate_mapping_descriptors

    # omitted

    mapping_descriptors.each do |i|
        print 'Generating'.rjust(12).green.bold
        print ' '
        puts i

        TRANSLATION_TABLES.map_at(i.virt_region, i.phys_region, i.attributes)
    end

    # omitted
end
```

The `generate_mapping_descriptors` method internally uses the
[rbelftools](https://github.com/david942j/rbelftools) gem to parse the kernel's `ELF`. It extracts
information about the various segments that comprise the kernel, as well as segment characteristics
like their `virtual` and `physical` addresses (aka the mapping; still identity-mapped in this case)
and the memory attributes.

Part of this information can be cross-checked using the `make readelf` target if you're curious:

```console
$ make readelf

Program Headers:
  Type           Offset             VirtAddr           PhysAddr
                 FileSiz            MemSiz              Flags  Align
  LOAD           0x0000000000010000 0x0000000000000000 0x0000000000000000
                 0x0000000000000000 0x0000000000080000  RW     0x10000
  LOAD           0x0000000000010000 0x0000000000080000 0x0000000000080000
                 0x000000000000cae8 0x000000000000cae8  R E    0x10000
  LOAD           0x0000000000020000 0x0000000000090000 0x0000000000090000
                 0x0000000000030dc0 0x0000000000030de0  RW     0x10000

 Section to Segment mapping:
  Segment Sections...
   00     .boot_core_stack
   01     .text .rodata
   02     .data .bss

```

The output of `generate_mapping_descriptors` is then fed into the `map_at()` method of the
`TRANSLATION_TABLE` instance. For it to work properly, `TRANSLATION_TABLE` needs knowledge about
**location** and **layout** of the kernel's table structure. Location will be queried from the `BSP`
code, which itself retrieves it by querying `KERNEL_ELF` for the `BSP`-specific `KERNEL_TABLES`
symbol. The layout, on the other hand, is hardcoded and as such must be kept in sync with the
structure definition in `translation_tables.rs`.

Finally, after the mappings have been created, it is time to _patch_ them back into the kernel ELF
file. This is initiated from `main.rb` again:

```ruby
kernel_patch_tables(kernel_elf_path)
kernel_patch_base_addr(kernel_elf_path)
```

The tool prints some information on the fly. Here's the console output of a successful run:

```console
$ make

Compiling kernel - rpi3
    Finished release [optimized] target(s) in 0.00s

Precomputing kernel translation tables and patching kernel ELF
             ------------------------------------------------------------------------------------
                 Sections          Virt Start Addr         Phys Start Addr       Size      Attr
             ------------------------------------------------------------------------------------
  Generating .boot_core_stack | 0x0000_0000_0000_0000 | 0x0000_0000_0000_0000 | 512 KiB | C RW XN
  Generating .text .rodata    | 0x0000_0000_0008_0000 | 0x0000_0000_0008_0000 |  64 KiB | C RO X
  Generating .data .bss       | 0x0000_0000_0009_0000 | 0x0000_0000_0009_0000 | 256 KiB | C RW XN
             ------------------------------------------------------------------------------------
    Patching Kernel table struct at ELF file offset 0x2_0000
    Patching Kernel tables physical base address start argument to value 0xb_0000 at ELF file offset 0x1_0068
    Finished in 0.16s

```

Please note how **only** the kernel binary is precomputed! Thanks to the changes made in the last
tutorial, anything else, like `MMIO-remapping`, can and will happen lazily during runtime.

### Other changes

Two more things that changed in this tutorial, but won't be explained in detail:

- Since virtual memory in `EL1` is now active from the start, any attempt to convert from a kernel
  `Address<Virtual>` to `Address<Physical>` is now done using the function
  `mmu::try_kernel_virt_addr_to_phys_addr()`, which internally uses a new function that has been
  added to the `TranslationTable` interface. If there is no valid virtual-to-physical mapping
  present in the tables, an `Err()` is returned.
- The precomputed translation table mappings won't automatically have entries in the kernel's
  `mapping info record`, which is used to print mapping info during boot. Mapping record entries are
  not computed offline in order to reduce complexity. For this reason, the `BSP` code, which in
  earlier tutorials would have called `kernel_map_at()` (which implicitly would have generated
  mapping record entries), now only calls `kernel_add_mapping_record()`, since the mappings are
  already in place.

## Discussion

It is understood that there is room for optimizations in the presented approach. For example, the
generated kernel binary contains the _complete_ array of translation tables for the whole kernel
virtual address space. However, most of the entries are empty initially, because the kernel binary
only occupies a small area in the tables. It would make sense to add some smarts so that only the
non-zero entries are packed into binary.

On the other hand, this would add complexity to the code. The increased size doesn't hurt too much
yet, so the reduced complexity in the code is a tradeoff made willingly to keep everything concise
and focused on the high-level concepts.

## Test it

```console
$ make chainboot
[...]

Precomputing kernel translation tables and patching kernel ELF
             ------------------------------------------------------------------------------------
                 Sections          Virt Start Addr         Phys Start Addr       Size      Attr
             ------------------------------------------------------------------------------------
  Generating .boot_core_stack | 0x0000_0000_0000_0000 | 0x0000_0000_0000_0000 | 512 KiB | C RW XN
  Generating .text .rodata    | 0x0000_0000_0008_0000 | 0x0000_0000_0008_0000 |  64 KiB | C RO X
  Generating .data .bss       | 0x0000_0000_0009_0000 | 0x0000_0000_0009_0000 | 256 KiB | C RW XN
             ------------------------------------------------------------------------------------
    Patching Kernel table struct at ELF file offset 0x2_0000
    Patching Kernel tables physical base address start argument to value 0xb_0000 at ELF file offset 0x1_0068
    Finished in 0.15s

Minipush 1.0

[MP] ⏳ Waiting for /dev/ttyUSB0
[MP] ✅ Serial connected
[MP] 🔌 Please power the target now

 __  __ _      _ _                 _
|  \/  (_)_ _ (_) |   ___  __ _ __| |
| |\/| | | ' \| | |__/ _ \/ _` / _` |
|_|  |_|_|_||_|_|____\___/\__,_\__,_|

           Raspberry Pi 3

[ML] Requesting binary
[MP] ⏩ Pushing 257 KiB ======================================🦀 100% 128 KiB/s Time: 00:00:02
[ML] Loaded! Executing the payload now

[    2.866917] mingo version 0.15.0
[    2.867125] Booting on: Raspberry Pi 3
[    2.867580] MMU online:
[    2.867872]       -------------------------------------------------------------------------------------------------------------------------------------------
[    2.869616]                         Virtual                                   Physical               Size       Attr                    Entity
[    2.871360]       -------------------------------------------------------------------------------------------------------------------------------------------
[    2.873105]       0x0000_0000_0000_0000..0x0000_0000_0007_ffff --> 0x00_0000_0000..0x00_0007_ffff | 512 KiB | C   RW XN | Kernel boot-core stack
[    2.874709]       0x0000_0000_0008_0000..0x0000_0000_0008_ffff --> 0x00_0008_0000..0x00_0008_ffff |  64 KiB | C   RO X  | Kernel code and RO data
[    2.876322]       0x0000_0000_0009_0000..0x0000_0000_000c_ffff --> 0x00_0009_0000..0x00_000c_ffff | 256 KiB | C   RW XN | Kernel data and bss
[    2.877893]       0x0000_0000_000d_0000..0x0000_0000_000d_ffff --> 0x00_3f20_0000..0x00_3f20_ffff |  64 KiB | Dev RW XN | BCM PL011 UART
[    2.879410]                                                                                                             | BCM GPIO
[    2.880861]       0x0000_0000_000e_0000..0x0000_0000_000e_ffff --> 0x00_3f00_0000..0x00_3f00_ffff |  64 KiB | Dev RW XN | BCM Interrupt Controller
[    2.882487]       -------------------------------------------------------------------------------------------------------------------------------------------
```

