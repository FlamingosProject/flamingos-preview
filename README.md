# Tutorial 13 - Exceptions Part 2: Peripheral IRQs

## tl;dr

- We write `device drivers` for the two interrupt controllers on the **Raspberry Pi 3** (`Broadcom`
  custom controller) and **Pi 4** (`ARM` Generic Interrupt Controller v2, `GICv2`).
- Modularity is ensured by interfacing everything through a trait named `IRQManager`.
- Handling for our first peripheral IRQs is implemented: The `UART`'s receive IRQs.

![Header](../doc/14_header.png)

## Table of Contents

- [Tutorial 13 - Exceptions Part 2: Peripheral IRQs](#tutorial-13---exceptions-part-2-peripheral-irqs)
  - [tl;dr](#tldr)
  - [Table of Contents](#table-of-contents)
  - [Introduction](#introduction)
  - [Different Controllers: A Usecase for Abstraction](#different-controllers-a-usecase-for-abstraction)
  - [New Challenges: Reentrancy](#new-challenges-reentrancy)
  - [Implementation](#implementation)
    - [The Kernel's Interfaces for Interrupt Handling](#the-kernels-interfaces-for-interrupt-handling)
      - [Uniquely Identifying an IRQ](#uniquely-identifying-an-irq)
        - [The BCM IRQ Number Scheme](#the-bcm-irq-number-scheme)
        - [The GICv2 IRQ Number Scheme](#the-gicv2-irq-number-scheme)
      - [Registering IRQ Handlers](#registering-irq-handlers)
      - [Handling Pending IRQs](#handling-pending-irqs)
    - [Reentrancy: What to protect?](#reentrancy-what-to-protect)
    - [The Interrupt Controller Device Drivers](#the-interrupt-controller-device-drivers)
      - [The BCM Driver (Pi 3)](#the-bcm-driver-pi-3)
        - [Peripheral Controller Register Access](#peripheral-controller-register-access)
        - [The IRQ Handler Table](#the-irq-handler-table)
      - [The GICv2 Driver (Pi 4)](#the-gicv2-driver-pi-4)
        - [GICC Details](#gicc-details)
        - [GICD Details](#gicd-details)
  - [Test it](#test-it)

## Introduction

In [tutorial 11], we laid the groundwork for exception handling from the processor architecture
side. Handler stubs for the different exception types were set up, and a first glimpse at exception
handling was presented by causing a `synchronous` exception by means of a `page fault`.

[tutorial 11]: ../11_exceptions_part1_groundwork

In this tutorial, we will add a first level of support for one of the three types of `asynchronous`
exceptions that are defined for `AArch64`: `IRQs`. The overall goal for this tutorial is to get rid
of the  busy-loop at the end of our current `kernel_main()` function, which actively polls the
`UART` for newly received characters. Instead, we will let the processor idle and wait for the
`UART`'s RX IRQs, which indicate that new characters were received. A respective `IRQ` service
routine, provided by the `UART` driver, will run in response to the `IRQ` and print the characters.

## Different Controllers: A Usecase for Abstraction

One very exciting aspect of this tutorial is that the `Pi 3` and the `Pi 4` feature completely
different interrupt controllers. This is also a first in all of the tutorial series. Until now, both
Raspberrys did not need differentiation with respect to their devices.

The `Pi 3` has a very simple custom controller made by Broadcom (BCM), the manufacturer of the Pi's
`System-on-Chip`. The `Pi 4` features an implementation of `ARM`'s Generic Interrupt Controller
version 2 (`GICv2`). Since ARM's GIC controllers are the prevalent interrupt controllers in ARM
application procesors, it is very beneficial to finally have it on the Raspberry Pi. It will enable
people to learn about one of the most common building blocks in ARM-based embedded computing.

This also means that we can finally make full use of all the infrastructure for abstraction that we
prepared already. We will design an `IRQManager` interface trait and implement it in both controller
drivers. The generic part of our `kernel` code will only be exposed to this trait (compare to the
diagram in the [tl;dr] section). This common idiom of *program to an interface, not an
implementation* enables a clean abstraction and makes the code modular and pluggable.

[tl;dr]: #tldr

## New Challenges: Reentrancy

Enabling interrupts also poses new challenges with respect to protecting certain code sections in
the kernel from being [re-entered]. Please read the linked article for background on that topic.

[re-entered]: https://en.wikipedia.org/wiki/Reentrancy_(computing)

Our `kernel` is still running on a single core. For this reason, we are still using our `NullLock`
pseudo-locks for `Critical Sections` or `shared resources`, instead of real `Spinlocks`. Hence,
interrupt handling at this point in time does not put us at risk of running into one of those
dreaded `deadlocks`, which is one of several side-effects that reentrancy can cause. For example, a
`deadlock` because of interrupts can happen happen when the executing CPU core has locked a
`Spinlock` at the beginning of a function, an IRQ happens, and the IRQ service routine is trying to
execute the same function. Since the lock is already locked, the core would spin forever waiting for
it to be released.

There is no straight-forward way to tell if a function is `reentrantcy`-safe or not. It usually
needs careful manual checking to conclude. Even though it might be technically safe to `re-enter` a
function, sometimes you don't want that to happen for functional reasons. For example, printing of a
string should not be interrupted by a an interrupt service routine that starts printing another
string, so that the output mixes. In the course of this tutorial, we will check and see where we
want to protect against `reentrancy`.

## Implementation

Okay, let's start. The following sections cover the the implementation in a top-down fashion,
starting with the trait that interfaces all the `kernel` components to each other.

### The Kernel's Interfaces for Interrupt Handling

First, we design the `IRQManager` trait that interrupt controller drivers must implement. The
minimal set of functionality that we need for starters is:

1. Registering an IRQ `handler` for a given IRQ `number`.
2. Enabling an IRQ (from the controller side).
3. Handling pending IRQs.
4. Printing the list of registered IRQ handlers.

The trait is defined as `exception::asynchronous::interface::IRQManager`:

```rust
pub trait IRQManager {
    /// The IRQ number type depends on the implementation.
    type IRQNumberType: Copy;

    /// Register a handler.
    fn register_handler(
        &self,
        irq_handler_descriptor: super::IRQHandlerDescriptor<Self::IRQNumberType>,
    ) -> Result<(), &'static str>;

    /// Enable an interrupt in the controller.
    fn enable(&self, irq_number: &Self::IRQNumberType);

    /// Handle pending interrupts.
    ///
    /// This function is called directly from the CPU's IRQ exception vector. On AArch64,
    /// this means that the respective CPU core has disabled exception handling.
    /// This function can therefore not be preempted and runs start to finish.
    ///
    /// Takes an IRQContext token to ensure it can only be called from IRQ context.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn handle_pending_irqs<'irq_context>(
        &'irq_context self,
        ic: &super::IRQContext<'irq_context>,
    );

    /// Print list of registered handlers.
    fn print_handler(&self) {}
}
```

#### Uniquely Identifying an IRQ

The first member of the trait is the [associated type] `IRQNumberType`. The following explains why
we make it customizable for the implementor and do not define the type as a plain integer right
away.

Interrupts can generally be characterizied with the following properties:

[associated type]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#specifying-placeholder-types-in-trait-definitions-with-associated-types

1. Software-generated vs hardware-generated.
2. Private vs shared.

Different interrupt controllers take different approaches at categorizing and numbering IRQs that
have one or the other property. Often times, this leads to situations where a plain integer does not
suffice to uniquely identify an IRQ, and makes it necessary to encode additional information in the
used type. Letting the respective interrupt controller driver define `IRQManager::IRQNumberType`
itself addresses this issue. The rest of the `BSP` must then conditionally use this type.

##### The BCM IRQ Number Scheme

The `BCM` controller of the `Raspberry Pi 3`, for example, is composed of two functional parts: A
**local** controller and a **peripheral** controller. The BCM's **local controller** handles all
`private` IRQs, which means private SW-generated IRQs and IRQs of private HW devices. An example for
the latter would be the `ARMv8` timer. Each  CPU core has its own private instance of it. The BCM's
**peripheral controller** handles all IRQs of `non-private` HW devices such as the `UART` (if those
IRQs can be declared as `shared` according to our taxonomy above is a different discussion, because
the BCM controller allows these HW interrupts to be routed to _only one CPU core at a time_).

The IRQ numbers of the BCM **local controller** range from `0..11`. The numbers of the **peripheral
controller** range from `0..63`. This demonstrates why a primitive integer type would not be
sufficient to uniquely encode the IRQs, because their ranges overlap. In the driver for the `BCM`
controller, we therefore define the associated type as follows:

```rust
pub type LocalIRQ = BoundedUsize<{ InterruptController::MAX_LOCAL_IRQ_NUMBER }>;
pub type PeripheralIRQ = BoundedUsize<{ InterruptController::MAX_PERIPHERAL_IRQ_NUMBER }>;

/// Used for the associated type of trait [`exception::asynchronous::interface::IRQManager`].
#[derive(Copy, Clone)]
#[allow(missing_docs)]
pub enum IRQNumber {
    Local(LocalIRQ),
    Peripheral(PeripheralIRQ),
}
```

The type `BoundedUsize` is a newtype around an `usize` that uses a [const generic] to ensure that
the value of the encapsulated IRQ number is in the allowed range (e.g. `0..MAX_LOCAL_IRQ_NUMBER` for
`LocalIRQ`, with `MAX_LOCAL_IRQ_NUMBER == 11`).

[const generic]: https://github.com/rust-lang/rfcs/blob/master/text/2000-const-generics.md

##### The GICv2 IRQ Number Scheme

The `GICv2` in the `Raspberry Pi 4`, on the other hand, uses a different scheme. IRQ numbers `0..31`
are for `private` IRQs. Those are further subdivided into `SW-generated` (SGIs, `0..15`) and
`HW-generated` (PPIs, Private Peripheral Interrupts, `16..31`). Numbers `32..1019` are for `shared
hardware-generated` interrupts (SPI, Shared Peripheral Interrupts).

There are no overlaps, so this scheme enables us to actually have a plain integer as a unique
identifier for the IRQs. We define the type as follows:

```rust
/// Used for the associated type of trait [`exception::asynchronous::interface::IRQManager`].
pub type IRQNumber = BoundedUsize<{ GICv2::MAX_IRQ_NUMBER }>;
```

#### Registering IRQ Handlers

To enable the controller driver to manage interrupt handling, it must know where to find respective
handlers, and it must know how to call them. For the latter, we define an `IRQHandler` trait in
`exception::asynchronous` that must be implemented by any SW entity that wants to handle IRQs:

```rust
/// Implemented by types that handle IRQs.
pub trait IRQHandler {
    /// Called when the corresponding interrupt is asserted.
    fn handle(&self) -> Result<(), &'static str>;
}
```

The `PL011Uart` driver gets the honors for being our first driver to ever implement this trait. In
this tutorial, the `RX IRQ` and the `RX Timeout IRQ` will be configured. This means that the
`PL011Uart` will assert it's interrupt line when one of following conditions is met:

1. `RX IRQ`: The RX FIFO fill level is equal or more than the configured trigger level (which will be 1/8 of
   the total FIFO size in our case).
1. `RX Timeout IRQ`: The RX FIFO fill level is greater than zero, but less than the configured fill
   level, and the characters have not been pulled for a certain amount of time. The exact time is
   not documented in the respective `PL011Uart` datasheet. Usually, it is a single-digit multiple of
   the time it takes to receive or transmit one character on the serial line.

 In the handler, our standard scheme of echoing any received characters back to the host is used:

```rust
impl exception::asynchronous::interface::IRQHandler for PL011Uart {
    fn handle(&self) -> Result<(), &'static str> {
        self.inner.lock(|inner| {
            let pending = inner.registers.MIS.extract();

            // Clear all pending IRQs.
            inner.registers.ICR.write(ICR::ALL::CLEAR);

            // Check for any kind of RX interrupt.
            if pending.matches_any(MIS::RXMIS::SET + MIS::RTMIS::SET) {
                // Echo any received characters.
                while let Some(c) = inner.read_char_converting(BlockingMode::NonBlocking) {
                    inner.write_char(c)
                }
            }
        });

        Ok(())
    }
}
```

Registering and enabling handlers in the interrupt controller is supposed to be done by the
respective drivers themselves. Therefore, we added a new function to the standard device driver
trait in `driver::interface::DeviceDriver` that must be implemented if IRQ handling is supported:

```rust
/// Called by the kernel to register and enable the device's IRQ handler.
///
/// Rust's type system will prevent a call to this function unless the calling instance
/// itself has static lifetime.
fn register_and_enable_irq_handler(
    &'static self,
    irq_number: &Self::IRQNumberType,
) -> Result<(), &'static str> {
    panic!(
        "Attempt to enable IRQ {} for device {}, but driver does not support this",
        irq_number,
        self.compatible()
    )
}
```

Here is the implementation for the `PL011Uart`:

```rust
fn register_and_enable_irq_handler(
    &'static self,
    irq_number: &Self::IRQNumberType,
) -> Result<(), &'static str> {
    use exception::asynchronous::{irq_manager, IRQHandlerDescriptor};

    let descriptor = IRQHandlerDescriptor::new(*irq_number, Self::COMPATIBLE, self);

    irq_manager().register_handler(descriptor)?;
    irq_manager().enable(irq_number);

    Ok(())
}
```

The `exception::asynchronous::irq_manager()` function used here returns a reference to an
implementor of the `IRQManager` trait. Since the implementation is supposed to be done by the
platform's interrupt controller, this call will redirect to the `kernel`'s instance of either the
driver for the `BCM` controller (`Raspberry Pi 3`) or the driver for the `GICv2` (`Pi 4`). We will
look into the  implementation of the `register_handler()` function from the driver's perspective
later. The gist here is that the calls on `irq_manager()` will make the platform's interrupt
controller aware that the `UART` driver (i) wants to handle its interrupt and (ii) which function it
provides to do so.

Also note how `irq_number` is supplied as a function argument and not hardcoded. The reason is that
the `UART` driver code is agnostic about the **IRQ numbers** that are associated to it. This is
vendor-supplied information and as such typically part of the Board Support Package (`BSP`). It can
vary from `BSP` to `BSP`, same like the board's memory map, which provides the `UART`'s MMIO
register addresses.

With all this in place, we can finally let drivers register and enable their IRQ handlers with the
interrupt controller, and unmask IRQ reception on the boot CPU core during the kernel init phase.
The global `driver_manager` takes care of this in the function `init_drivers_and_irqs()` (before
this tutorial, the function's name was `init_drivers()`), where this happens as the third and last
step of initializing all registered device drivers:

```rust
pub unsafe fn init_drivers_and_irqs(&self) {
    self.for_each_descriptor(|descriptor| {
        // 1. Initialize driver.
        if let Err(x) = descriptor.device_driver.init() {
            // omitted for brevity
        }

        // 2. Call corresponding post init callback.
        if let Some(callback) = &descriptor.post_init_callback {
            // omitted for brevity
        }
    });

    // 3. After all post-init callbacks were done, the interrupt controller should be
    //    registered and functional. So let drivers register with it now.
    self.for_each_descriptor(|descriptor| {
        if let Some(irq_number) = &descriptor.irq_number {
            if let Err(x) = descriptor
                .device_driver
                .register_and_enable_irq_handler(irq_number)
            {
                panic!(
                    "Error during driver interrupt handler registration: {}: {}",
                    descriptor.device_driver.compatible(),
                    x
                );
            }
        }
    });
}
```


In `main.rs`, IRQs are unmasked right afterwards, after which point IRQ handling is live:

```rust
// Initialize all device drivers.
driver::driver_manager().init_drivers_and_irqs();

// Unmask interrupts on the boot CPU core.
exception::asynchronous::local_irq_unmask();
```

#### Handling Pending IRQs

Now that interrupts can happen, the `kernel` needs a way of requesting the interrupt controller
driver to handle pending interrupts. Therefore, implementors of the trait `IRQManager` must also
supply the following function:

```rust
fn handle_pending_irqs<'irq_context>(
    &'irq_context self,
    ic: &super::IRQContext<'irq_context>,
);
```

An important aspect of this function signature is that we want to ensure that IRQ handling is only
possible from IRQ context. Part of the reason is that this invariant allows us to make some implicit
assumptions (which might depend on the target architecture, though). For example, as we have learned
in [tutorial 11], in `AArch64`, _"all kinds of exceptions are turned off upon taking an exception,
so that by default, exception handlers can not get interrupted themselves"_ (note that an IRQ is an
exception). This is a useful property that relieves us from explicitly protecting IRQ handling from
being interrupted itself. Another reason would be that calling IRQ handling functions from arbitrary
execution contexts just doesn't make a lot of sense.

[tutorial 11]: ../11_exceptions_part1_groundwork/

So in order to ensure that this function is only being called from IRQ context, we borrow a
technique that I first saw in the [Rust embedded WG]'s [bare-metal crate]. It uses Rust's type
system to create a "token" that is only valid for the duration of the IRQ context. We create it
directly at the top of the IRQ vector function in `_arch/aarch64/exception.rs`, and pass it on to
the the implementation of the trait's handling function:

[Rust embedded WG]: https://github.com/rust-embedded/bare-metal
[bare-metal crate]: https://github.com/rust-embedded/bare-metal/blob/master/src/lib.rs#L20

```rust
#[no_mangle]
extern "C" fn current_elx_irq(_e: &mut ExceptionContext) {
    let token = unsafe { &exception::asynchronous::IRQContext::new() };
    exception::asynchronous::irq_manager().handle_pending_irqs(token);
}
```

By requiring the caller of the function `handle_pending_irqs()` to provide this `IRQContext` token,
we can prevent that the same function is accidentally being called from somewhere else. It is
evident, though, that for this to work, it is the _user's responsibility_ to only ever create this
token from within an IRQ context. If you want to circumvent this on purpose, you can do it.

### Reentrancy: What to protect?

Now that interrupt handling is live, we need to think about `reentrancy`. At [the beginning of this
tutorial], we mused about the need to protect certain functions from being re-entered, and that it
is not straight-forward to identify all the places that need protection.

[the beginning of this tutorial]: #new-challenges-reentrancy

In this tutorial, we will keep this part short nonetheless by taking a better-safe-than-sorry
approach. In the past, we already made efforts to prepare parts of `shared resources` (e.g. global
device driver instances) to be protected against parallel access. We did so by wrapping them into
`NullLocks`, which we will upgrade to real `Spinlocks` once we boot secondary CPU cores.

We can hook on that previous work and reason that anything that we wanted protected against parallel
access so far, we also want it protected against reentrancy now. Therefore, we upgrade all
`NullLocks` to `IRQSafeNullocks`:

```rust
impl<T> interface::Mutex for IRQSafeNullLock<T> {
    type Data = T;

    fn lock<R>(&self, f: impl FnOnce(&mut Self::Data) -> R) -> R {
        // In a real lock, there would be code encapsulating this line that ensures that this
        // mutable reference will ever only be given out once at a time.
        let data = unsafe { &mut *self.data.get() };

        // Execute the closure while IRQs are masked.
        exception::asynchronous::exec_with_irq_masked(|| f(data))
    }
}
```

The new part is that the call to `f(data)` is executed as a closure in
`exception::asynchronous::exec_with_irq_masked()`. Inside that function, IRQs on the executing CPU
core are masked before the `f(data)` is being executed, and restored afterwards:

```rust
/// Executes the provided closure while IRQs are masked on the executing core.
///
/// While the function temporarily changes the HW state of the executing core, it restores it to the
/// previous state before returning, so this is deemed safe.
#[inline(always)]
pub fn exec_with_irq_masked<T>(f: impl FnOnce() -> T) -> T {
    let saved = local_irq_mask_save();
    let ret = f();
    local_irq_restore(saved);

    ret
}
```

The helper functions used here are defined in `src/_arch/aarch64/exception/asynchronous.rs`.

### The Interrupt Controller Device Drivers

The previous sections explained how the `kernel` uses the `IRQManager` trait. Now, let's have a look
at the driver-side of it in the Raspberry Pi `BSP`. We start with the Broadcom interrupt controller
featured in the `Pi 3`.

#### The BCM Driver (Pi 3)

As mentioned earlier, the `BCM` driver consists of two subcomponents, a **local** and a
**peripheral** controller. The local controller owns a bunch of configuration registers, among
others, the `routing` configuration for peripheral IRQs such as those from the `UART`. Peripheral
IRQs can be routed to _one core only_. In our case, we leave the default unchanged, which means
everything is routed to the boot CPU core. The image below depicts the `struct diagram` of the
driver implementation.

![BCM Driver](../doc/14_BCM_driver.png)

We have a top-level driver, which implements the `IRQManager` trait. _Only the top-level driver_ is
exposed to the rest of the `kernel`. The top-level itself has two members, representing the local
and the peripheral controller, respectively, which implement the `IRQManager` trait as well. This
design allows for easy forwarding of function calls from the top-level driver to one of the
subcontrollers.

For this tutorial, we leave out implementation of the local controller, because we will only be
concerned with the peripheral  `UART` IRQ.

##### Peripheral Controller Register Access

When writing a device driver for a kernel with exception handling and multi-core support, it is
always important to analyze what parts of the driver will need protection against reentrancy (we
talked about this earlier in this tutorial) and/or parallel execution of other driver parts. If a
driver function needs to follow a vendor-defined sequence of multiple register operations that
include `write operations`, this is usually a good hint that protection might be needed. But that is
only one of many examples.

For the driver implementation in this tutorial, we are following a simple rule: Register read access
is deemed always safe. Write access is guarded by an `IRQSafeNullLock`, which means that we are safe
against `reentrancy` issues, and also in the future when the kernel will be running on multiple
cores, we can easily upgrade to a real spinlock, which serializes register write operations from
different CPU cores.

In fact, for this tutorial, we probably would not have needed any protection yet, because all the
driver does is read from the `PENDING_*` registers for the `handle_pending_irqs()` implementation,
and writing to the `ENABLE_*` registers for the `enable()` implementation. However, the chosen
architecture will have us set up for future extensions, when more complex register manipulation
sequences might be needed.

Since nothing complex is happening in the implementation, it is not covered in detail here. Please
refer to [the source of the **peripheral** controller] to check it out.

[the source of the **peripheral** controller]: kernel/src/bsp/device_driver/bcm/bcm2xxx_interrupt_controller/peripheral_ic.rs

##### The IRQ Handler Table

Calls to `register_handler()` result in the driver inserting the provided handler reference in a
specific table (the handler reference is a member of `IRQDescriptor`):

```rust
type HandlerTable = [Option<exception::asynchronous::IRQHandlerDescriptor<PeripheralIRQ>>;
    PeripheralIRQ::MAX_INCLUSIVE + 1];
```

One of the requirements for safe operation of the `kernel` is that those handlers are not
registered, removed or exchanged in the middle of an IRQ handling situation. This, again, is a
multi-core scenario where one core might look up a handler entry while another core is modifying the
same in parallel.

While we want to allow drivers to take the decision of registering or not registering a handler at
runtime, there is no need to allow it for the _whole_ runtime of the kernel. It is fine to restrict
this option to the kernel `init phase`, at which only a single boot core runs and IRQs are masked.

We introduce the so called `InitStateLock` for cases like that. From an API-perspective, it is a
special variant of a `Read/Write exclusion synchronization primitive`. RWLocks in the Rust standard
library [are characterized] as allowing _"a number of readers or at most one writer at any point in
time"_. For the `InitStateLock`, we only implement the `read()` and `write()` functions:

[are characterized]: https://doc.rust-lang.org/std/sync/struct.RwLock.html

```rust
impl<T> interface::ReadWriteEx for InitStateLock<T> {
    type Data = T;

    fn write<R>(&self, f: impl FnOnce(&mut Self::Data) -> R) -> R {
        assert!(
            state::state_manager().is_init(),
            "InitStateLock::write called after kernel init phase"
        );
        assert!(
            !exception::asynchronous::is_local_irq_masked(),
            "InitStateLock::write called with IRQs unmasked"
        );

        let data = unsafe { &mut *self.data.get() };

        f(data)
    }

    fn read<R>(&self, f: impl FnOnce(&Self::Data) -> R) -> R {
        let data = unsafe { &*self.data.get() };

        f(data)
    }
}
```

The `write()` function is guarded by two `assertions`. One ensures that IRQs are masked, the other
checks the `state::state_manager()` if the kernel is still in the init phase. The `State Manager` is
new since this tutorial, and implemented in `src/state.rs`. It provides atomic state transition and
reporting functions that are called when the kernel enters a new phase. In the current kernel, the
only call is happening before the transition from `kernel_init()` to `kernel_main()`:

```rust
// Announce conclusion of the kernel_init() phase.
state::state_manager().transition_to_single_core_main();
```

P.S.: Since the use case for the `InitStateLock` also applies to a few other places in the kernel
(for example, registering the system-wide console during early boot), `InitStateLock`s have been
incorporated in those other places as well.

#### The GICv2 Driver (Pi 4)

As we learned earlier, the ARM `GICv2` in the `Raspberry Pi 4` features a continuous interrupt
number range:
- IRQ numbers `0..31` represent IRQs that are private (aka local) to the respective processor core.
- IRQ numbers `32..1019` are for shared IRQs.

The `GIC` has a so-called `Distributor`, the `GICD`, and a `CPU Interface`, the `GICC`. The `GICD`,
among other things, is used to enable IRQs and route them to one or more CPU cores. The `GICC` is
used by CPU cores to check which IRQs are pending, and to acknowledge them once they were handled.
There is one dedicated `GICC` for _each CPU core_.

One neat thing about the `GICv2` is that any MMIO registers that are associated to core-private IRQs
are `banked`. That means that different CPU cores can assert the same MMIO address, but they will
end up accessing a core-private copy of the referenced register. This makes it very comfortable to
program the `GIC`, because this hardware design ensures that each core only ever gets access to its
own resources. Preventing one core to accidentally or willfully fiddle with the IRQ state of another
core must therefore not be enforced in software.

In summary, this means that any registers in the `GICD` that deal with the core-private IRQ range
are banked. Since there is one `GICC` per CPU core, the whole thing is banked. This allows us to
design the following `struct diagram` for our driver implementation:

![GICv2 Driver](../doc/14_GICv2_driver.png)

The top-level struct is composed of a `GICD`, a `GICC` and a `HandlerTable`. The latter is
implemented identically as in the `Pi 3`.

##### GICC Details

Since the `GICC` is banked wholly, the top-level driver can directly forward any requests to it,
without worrying about concurrency issues for now. Note that this only works as long as the `GICC`
implementation is only accessing the banked `GICC` registers, and does not save any state in member
variables that are stored in `DRAM`. The two main duties of the `GICC` struct are to read the `IAR`
(Interrupt Acknowledge) register, which returns the number of the highest-priority pending IRQ, and
writing to the `EOIR` (End Of Interrupt) register, which tells the hardware that handling of an
interrupt is now concluded.

##### GICD Details

The `GICD` hardware block differentiates between `shared` and `banked` registers. As with the
`GICC`, we don't have to protect the banked registers against concurrent access. The shared
registers are wrapped into an `IRQSafeNullLock` again. The important parts of the `GICD` for this
tutorial are the `ITARGETSR[256]` and `ISENABLER[32]` register arrays.

Each `ITARGETSR` is subdivided into four _bytes_. Each byte represents one IRQ, and stores a bitmask
that encodes all the `GICCs` to which the respective IRQ is forwarded. For example,
`ITARGETSR[0].byte0` would represent IRQ number 0, and `ITARGETSR[0].byte3` IRQ number 3. In the
`ISENABLER`, each _bit_ represents an IRQ. For example, `ISENABLER[0].bit3` is IRQ number 3.

In summary, this means that `ITARGETSR[0..7]` and `ISENABLER[0]` represent the first 32 IRQs (the
banked ones), and as such, we split the register block into `shared` and `banked` parts accordingly
in `gicd.rs`:

```rust
register_structs! {
    #[allow(non_snake_case)]
    SharedRegisterBlock {
        (0x000 => CTLR: ReadWrite<u32, CTLR::Register>),
        (0x004 => TYPER: ReadOnly<u32, TYPER::Register>),
        (0x008 => _reserved1),
        (0x104 => ISENABLER: [ReadWrite<u32>; 31]),
        (0x180 => _reserved2),
        (0x820 => ITARGETSR: [ReadWrite<u32, ITARGETSR::Register>; 248]),
        (0xC00 => @END),
    }
}

register_structs! {
    #[allow(non_snake_case)]
    BankedRegisterBlock {
        (0x000 => _reserved1),
        (0x100 => ISENABLER: ReadWrite<u32>),
        (0x104 => _reserved2),
        (0x800 => ITARGETSR: [ReadOnly<u32, ITARGETSR::Register>; 8]),
        (0x820 => @END),
    }
}
```

As with the implementation of the BCM interrupt controller driver, we won't cover the remaining
parts in exhaustive detail. For that, please refer to [this folder] folder which contains all the
sources.

[this folder]: kernel/src/bsp/device_driver/arm

## Test it

When you load the kernel, any keystroke results in echoing back the character by way of IRQ
handling. There is no more polling done at the end of `kernel_main()`, just waiting for events such
as IRQs:

```rust
fn kernel_main() -> ! {

    // omitted for brevity

    info!("Echoing input now");
    cpu::wait_forever();
}
```

Raspberry Pi 3:

```console
$ make chainboot
[...]
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
[MP] ⏩ Pushing 66 KiB =========================================🦀 100% 0 KiB/s Time: 00:00:00
[ML] Loaded! Executing the payload now

[    0.822492] mingo version 0.13.0
[    0.822700] Booting on: Raspberry Pi 3
[    0.823155] MMU online. Special regions:
[    0.823632]       0x00080000 - 0x0008ffff |  64 KiB | C   RO PX  | Kernel code and RO data
[    0.824650]       0x3f000000 - 0x4000ffff |  17 MiB | Dev RW PXN | Device MMIO
[    0.825539] Current privilege level: EL1
[    0.826015] Exception handling state:
[    0.826459]       Debug:  Masked
[    0.826849]       SError: Masked
[    0.827239]       IRQ:    Unmasked
[    0.827651]       FIQ:    Masked
[    0.828041] Architectural timer resolution: 52 ns
[    0.828615] Drivers loaded:
[    0.828951]       1. BCM PL011 UART
[    0.829373]       2. BCM GPIO
[    0.829731]       3. BCM Interrupt Controller
[    0.830262] Registered IRQ handlers:
[    0.830695]       Peripheral handler:
[    0.831141]              57. BCM PL011 UART
[    0.831649] Echoing input now
```

Raspberry Pi 4:

```console
$ BSP=rpi4 make chainboot
[...]
Minipush 1.0

[MP] ⏳ Waiting for /dev/ttyUSB0
[MP] ✅ Serial connected
[MP] 🔌 Please power the target now

 __  __ _      _ _                 _
|  \/  (_)_ _ (_) |   ___  __ _ __| |
| |\/| | | ' \| | |__/ _ \/ _` / _` |
|_|  |_|_|_||_|_|____\___/\__,_\__,_|

           Raspberry Pi 4

[ML] Requesting binary
[MP] ⏩ Pushing 73 KiB =========================================🦀 100% 0 KiB/s Time: 00:00:00
[ML] Loaded! Executing the payload now

[    0.886853] mingo version 0.13.0
[    0.886886] Booting on: Raspberry Pi 4
[    0.887341] MMU online. Special regions:
[    0.887818]       0x00080000 - 0x0008ffff |  64 KiB | C   RO PX  | Kernel code and RO data
[    0.888836]       0xfe000000 - 0xff84ffff |  25 MiB | Dev RW PXN | Device MMIO
[    0.889725] Current privilege level: EL1
[    0.890201] Exception handling state:
[    0.890645]       Debug:  Masked
[    0.891035]       SError: Masked
[    0.891425]       IRQ:    Unmasked
[    0.891837]       FIQ:    Masked
[    0.892227] Architectural timer resolution: 18 ns
[    0.892801] Drivers loaded:
[    0.893137]       1. BCM PL011 UART
[    0.893560]       2. BCM GPIO
[    0.893917]       3. GICv2 (ARM Generic Interrupt Controller v2)
[    0.894654] Registered IRQ handlers:
[    0.895087]       Peripheral handler:
[    0.895534]             153. BCM PL011 UART
[    0.896042] Echoing input now
```

