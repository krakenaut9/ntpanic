# ntpanic

`ntpanic` provides a global `#[panic_handler]` for **Rust Windows kernel-mode drivers**.

When a panic occurs, the handler:

1. prints the panic location with `DbgPrintEx` (if available),
2. checks whether a kernel debugger is attached,
3. breaks into the debugger with `DbgBreakPointWithStatus` when one is present,
4. terminates the system with `KeBugCheckEx`.

This crate is intended for `#![no_std]` Windows kernel driver development where a deterministic panic path is required.

---

## Features

- `#![no_std]` compatible
- Windows kernel-mode panic handling
- Logs panic source location (`file:line`) through `DbgPrintEx`
- Breaks into KD/WinDbg when a debugger is present
- Falls back to a bug check for guaranteed termination
- Minimal integration: add the crate as a dependency and link it into your driver

---

## Scope

This crate is **Windows kernel-mode only**.

It is designed for Rust drivers built against Windows kernel APIs exposed through `windows-sys`.

It does **not** attempt to recover from panic. A panic is treated as a fatal, unrecoverable condition.

---

## Installation

Add the crate to your driver crate:

```toml
[dependencies]
ntpanic = "0.1"