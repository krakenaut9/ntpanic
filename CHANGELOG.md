# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

While the version is below 1.0, a minor bump may contain breaking changes.

## [Unreleased]

## [0.1.0] - 2026-10-14

First public release.

### Added

- A global `#[panic_handler]` for `#![no_std]` Windows kernel-mode drivers on
  x64, x86 and ARM64. On a panic it prints one line with `DbgPrintEx` giving the
  file, line, IRQL, process ID and thread ID, breaks into the kernel debugger if
  one is attached, and bug-checks the system.
- Bug check `0x52555354`, "RUST" in ASCII, whose parameters are the line, the
  address and length of the file path, and the column.
- `set_bug_check_code`, to report another bug check code. The handler reads it
  just before bug-checking, so a callback can also set it.
- `set_component_id` and `set_level`, to choose the `DbgPrintEx` component ID
  and level. The default, `DPFLTR_IHVDRIVER_ID` at `DPFLTR_ERROR_LEVEL`, is
  shown without any setup.
- `set_callback` and `Callback`: a function the handler calls with the panic and
  its IRQL, after the message and before the debugger break, for example to log
  the panic message with the driver's own logger. Only the first panic calls it,
  so a panic inside it cannot recurse.

### Notes

- Above DIRQL, where `DbgPrintEx` can deadlock, the handler prints nothing; the
  callback, the debugger break and the bug check still happen.
- A path longer than 256 bytes is printed as its last 256, to keep the message
  within `DbgPrintEx`'s 512-byte limit. The bug check carries the whole path.
- `windows-sys` 0.61 imports `DbgPrintEx` from `ntdll.dll`, and
  `KeGetCurrentIrql` from `ntoskrnl.exe` on every architecture. `ntpanic`
  declares both itself, taking `KeGetCurrentIrql` from `hal.dll` on x86 and
  ARM64. Its imports use `raw-dylib`, so building needs no import library and
  no WDK.
- A driver must be built with `panic = "abort"`, can have only one panic
  handler, and has to name the crate to link it, for example with
  `#[cfg(not(test))] extern crate ntpanic;`.
- Minimum supported Rust version is 1.71.

[Unreleased]: https://github.com/krakenaut9/ntpanic/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/krakenaut9/ntpanic/releases/tag/v0.1.0
