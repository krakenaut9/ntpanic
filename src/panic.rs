//! Panic handling for Windows kernel-mode Rust drivers.
//!
//! This module defines the crate's global `#[panic_handler]`.
//!
//! On panic, the handler:
//!
//! 1. extracts panic location information from [`core::panic::PanicInfo`],
//! 2. prints a diagnostic message using `DbgPrintEx`,
//! 3. checks whether a kernel debugger is attached,
//! 4. breaks into the debugger with `DbgBreakPointWithStatus` when available,
//! 5. terminates the system with `KeBugCheckEx`.
//!
//! The panic path is intentionally fatal. Continuing execution after a panic in
//! kernel mode is unsafe and unsupported by this crate.

use core::{panic::PanicInfo, ptr::null};
use windows_sys::{core::PCSTR, Wdk::System::SystemServices::{
    DbgBreakPointWithStatus, DbgPrintEx, KdRefreshDebuggerNotPresent, KeBugCheckEx
}};

/// Custom bug check code used by this crate when a Rust panic reaches the
/// kernel panic handler.
///
/// The accompanying bug check arguments are:
///
/// - Arg1: panic line number
/// - Arg2: pointer to the file path bytes
/// - Arg3: file path length
/// - Arg4: reserved (`0`)
const RUST_KERNEL_BUGCHECK: u32 = 0xE052_5354;


/// Global panic handler for Windows kernel-mode drivers.
///
/// This function is the crate's global [`panic_handler`]. When a panic occurs,
/// it attempts to report the panic location through `DbgPrintEx`, optionally
/// breaks into the kernel debugger if one is attached, and then terminates the
/// system with `KeBugCheckEx`.
///
/// # Behavior
///
/// On panic, the handler:
///
/// 1. extracts the source file path and line number from [`PanicInfo`], when available;
/// 2. prints a diagnostic message with `DbgPrintEx`;
/// 3. checks whether a kernel debugger is present via `KdRefreshDebuggerNotPresent`;
/// 4. calls `DbgBreakPointWithStatus(0)` if a debugger is attached;
/// 5. invokes `KeBugCheckEx` with a crate-specific bug check code.
///
/// The bug check arguments are used as follows:
///
/// - **Arg1**: panic line number
/// - **Arg2**: pointer to the source file path bytes
/// - **Arg3**: source file path length
/// - **Arg4**: reserved (`0`)
///
/// # Panics
///
/// This function never returns.
///
/// # Notes
///
/// This handler is intended for `#![no_std]` Rust drivers running in
/// Windows kernel mode. Panic recovery is not attempted, since continuing
/// execution after a panic in kernel space is considered unsafe.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let (file_ptr, file_len, line) = match info.location() {
        Some(loc) => {
            let f = loc.file();
            (f.as_ptr(), f.len(), loc.line())
        }
        None => (null(), 0usize, 0u32),
    };

    unsafe {
        if file_ptr.is_null() {
            DbgPrintEx(
                0,
                0,
                PCSTR::from(c"*************** RUST KERNEL PANIC (unknown location) ***************\n".as_ptr().cast()),
            );
        } else {
            DbgPrintEx(
                0,
                0,
                PCSTR::from(c"*************** RUST KERNEL PANIC at %.*s:%u ***************\n".as_ptr().cast()),
                file_len,
                file_ptr,
                line,
            );
        }
    }

    let debugger_not_present = unsafe { KdRefreshDebuggerNotPresent() };

    if !debugger_not_present {
        unsafe {
            DbgBreakPointWithStatus(0);
        }
    }

    unsafe {
        KeBugCheckEx(
            RUST_KERNEL_BUGCHECK,
            line as usize,
            file_ptr as usize,
            file_len,
            0,
        );
    }

    unreachable!();
}
