# ntpanic

A panic handler for Windows kernel-mode Rust drivers.

A panic in a driver cannot be recovered from. `ntpanic` makes it fail usefully: it
prints where the panic happened with `DbgPrintEx`, breaks into the kernel debugger if
one is attached, and then bug-checks the system with its own bug check code, whose
parameters give the file, line and column of the panic.

---

## Highlights

- `#![no_std]`, no allocation, nothing to initialize
- x64, x86 and ARM64
- Prints the panic's location, IRQL, process ID and thread ID
- Skips the message above DIRQL, where `DbgPrintEx` can deadlock
- Breaks into KD or `WinDbg` when a debugger is attached
- Ends in a bug check that carries the file, line and column, with the code
  `0x52555354` ("RUST") or one of your own
- Optional callback, for example to log the panic message with your own logger
- Builds on stable Rust 1.71 or later, without the WDK

---

## Usage

Add the crate to your driver:

```toml
[dependencies]
ntpanic = "0.1"
```

Rust links a dependency only when the crate refers to it, so name it once in the
driver's root module:

```rust,ignore
#[cfg(not(test))]
extern crate ntpanic;
```

`cfg(not(test))` keeps it out of `cargo test` builds, which get their panic handler
from `std`: a binary can have only one. For the same reason, `ntpanic` cannot be used
together with another panic handler crate, such as `wdk-panic`.

As a `#![no_std]` driver, it must be built with `panic = "abort"`:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

---

## What happens on a panic

1. **Message.** At or below DIRQL, the handler prints one line with `DbgPrintEx`, which
   a kernel debugger, or `DebugView` with *Capture Kernel* on, displays:

   ```text
   *************** RUST KERNEL PANIC at src\driver.rs:42 (irql 2, pid 4, tid 132) ***************
   ```

   A path longer than 256 bytes is shortened to its last 256, so that the line stays
   within `DbgPrintEx`'s 512-byte limit. Above DIRQL, where `DbgPrintEx` can deadlock,
   there is no message, but the steps below still run.
2. **Callback.** The first panic calls the callback, if one is set; see
   [Panic callback](#panic-callback).
3. **Debugger.** If a kernel debugger is attached, the handler breaks into it with
   `DbgBreakPointWithStatus`, stopping on the panicking thread.
4. **Bug check.** The handler calls `KeBugCheckEx` with the code `0x52555354`, "RUST"
   in ASCII, unless you set another (see [Bug check code](#bug-check-code)), and these
   parameters:

   | Parameter | Value                                                      |
   | --------- | ---------------------------------------------------------- |
   | 1         | the line number                                            |
   | 2         | the address of the file path, which is not null-terminated |
   | 3         | the length of the file path, in bytes                      |
   | 4         | the column number                                          |

   A panic without a location reports the path `<unknown>`, the line 0 and the column 0.

In a kernel or complete memory dump, `.bugcheck` shows the parameters and `da` reads
the path:

```text
kd> .bugcheck
Bugcheck code 52555354
Arguments 00000000`0000002a fffff806`2b4c1230 00000000`0000000d 00000000`00000011
kd> da fffff806`2b4c1230 L0xd
fffff806`2b4c1230  "src\driver.rs"
```

---

## Settings

The settings can be changed at any IRQL. A panic uses the values set before it, so set
them early, for example at the start of `DriverEntry`.

### Debug output filter

The message is printed under the component `DPFLTR_IHVDRIVER_ID` (77), at the level
`DPFLTR_ERROR_LEVEL` (0), which Windows shows by default whatever the component.
`set_component_id` and `set_level` change them; Microsoft asks drivers to use one of
the six component IDs reserved for independent hardware vendors, 77 to 82:

```rust,ignore
// From the WDK's dpfilter.h.
const DPFLTR_IHVNETWORK_ID: u32 = 80;

ntpanic::set_component_id(DPFLTR_IHVNETWORK_ID);
```

A level other than `DPFLTR_ERROR_LEVEL` is shown only if the component's filter mask
enables it, so changing the level can hide the message. See
[Reading and Filtering Debugging Messages](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/reading-and-filtering-debugging-messages).

### Bug check code

`set_bug_check_code` replaces the default code, for example so that crash analysis can
tell your driver's panics apart from those of other drivers that use `ntpanic`:

```rust,ignore
ntpanic::set_bug_check_code(u32::from_be_bytes(*b"ACME"));
```

The parameters keep their meaning whatever the code. Choose a code that Windows does not
use (see the [Bug Check Code Reference](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/bug-check-code-reference2)):
`WinDbg` describes a crash, and its parameters, by its code. The handler reads the code
just before it bug-checks, so a panic callback can also set it.

### Panic callback

`ntpanic` prints the panic's location but not its message, such as
`called Option::unwrap() on a None value`: formatting the message would put the
formatting code, and the `Display` code of whatever the message contains, in the panic
path of every driver. A driver can log the message itself, from a callback:

```rust,ignore
use core::panic::PanicInfo;

// From the WDK's wdm.h.
const DISPATCH_LEVEL: u8 = 2;

fn log_panic(info: &PanicInfo, irql: u8) {
    // A logger may allocate or touch pageable memory, which is safe only below
    // DISPATCH_LEVEL.
    if irql < DISPATCH_LEVEL {
        log::error!("{info}");
    }
}

ntpanic::set_callback(Some(log_panic));
```

The callback gets the panic and the IRQL it occurred at. It runs after the message and
before the debugger break, and only once: a later panic in the same crash, including
one inside the callback, is still printed but does not call it.

It runs where the panic occurred, on that thread's stack and at any IRQL up to
`HIGH_LEVEL`, with the system in an unknown state. It must not wait or take locks, must
not allocate or touch pageable memory at `DISPATCH_LEVEL` or above, and must not call
`DbgPrintEx` above DIRQL. If it never returns, the system hangs instead of
bug-checking.

To reset a device or add data to the crash dump, register a bug check callback with
`KeRegisterBugCheckCallback` or `KeRegisterBugCheckReasonCallback` instead: Windows
calls those for every bug check, not only for panics.

---

## Requirements

- A Windows kernel-mode driver for `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc` or
  `aarch64-pc-windows-msvc`
- `panic = "abort"`
- Rust 1.71 or later

The handler imports `DbgPrintEx`, `DbgBreakPointWithStatus`,
`KdRefreshDebuggerNotPresent`, `KeBugCheckEx`, `PsGetCurrentProcessId` and
`PsGetCurrentThreadId` from `ntoskrnl.exe`, and `KeGetCurrentIrql` from `ntoskrnl.exe`
on x64 and from `hal.dll` on x86 and ARM64, as `wdm.h` declares them. It declares them
with `raw-dylib`, so building it needs no import library and no WDK.

---

## Minimum supported Rust version

**1.71**, the minimum of the `windows-sys` and `windows-link` crates that `ntpanic` uses.

The MSRV is treated as a compatibility promise: raising it is a breaking change and
will come with a minor version bump before 1.0.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
