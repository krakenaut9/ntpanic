//! Panic handling for Windows kernel-mode Rust drivers.
//!
//! This module defines the crate's global `#[panic_handler]`, described at [`panic()`].
//! The panic path is intentionally fatal. Continuing execution after a panic in
//! kernel mode is unsafe and unsupported by this crate.

use core::{panic::PanicInfo, sync::atomic::Ordering};
use windows_sys::Wdk::System::SystemServices::{
    DbgBreakPointWithStatus, KdRefreshDebuggerNotPresent,
};

// windows-sys imports DbgPrintEx from ntdll.dll, which a driver cannot load.
windows_link::link!("ntoskrnl.exe" "C" fn DbgPrintEx(componentid: u32, level: u32, format: *const u8, ...) -> u32);
// Declared here for `-> !`: the WDK marks it DECLSPEC_NORETURN, windows-sys does not.
windows_link::link!("ntoskrnl.exe" "system" fn KeBugCheckEx(bugcheckcode: u32, bugcheckparameter1: usize, bugcheckparameter2: usize, bugcheckparameter3: usize, bugcheckparameter4: usize) -> !);

/// Custom bug check code used by this crate when a Rust panic reaches the
/// kernel panic handler.
///
/// The accompanying bug check arguments are:
///
/// - Arg1: panic line number
/// - Arg2: pointer to the file path bytes
/// - Arg3: file path length
/// - Arg4: reserved (`0`)
///
/// A panic without a location reports the file path `<unknown>` and line `0`.
const RUST_KERNEL_BUGCHECK: u32 = 0xE052_5354;

// Reported when the panic carries no location.
const UNKNOWN_FILE: &str = "<unknown>";
const UNKNOWN_LINE: u32 = 0;

// Status sent to the debugger with the break-in.
const DEBUG_BREAK_STATUS: u32 = 0;

// Bug check Arg4, which this crate leaves unused.
const RESERVED_BUGCHECK_PARAMETER: usize = 0;

/// Global panic handler for Windows kernel-mode drivers.
///
/// This function is the crate's global `#[panic_handler]`. When a panic occurs,
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
/// 5. invokes `KeBugCheckEx` with the code and parameters described at
///    [`RUST_KERNEL_BUGCHECK`].
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
    let (file, line) = info
        .location()
        .map_or((UNKNOWN_FILE, UNKNOWN_LINE), |location| {
            (location.file(), location.line())
        });

    let component_id = crate::COMPONENT_ID.load(Ordering::Relaxed);
    let level = crate::LEVEL.load(Ordering::Relaxed);

    unsafe {
        DbgPrintEx(
            component_id,
            level,
            concat!(
                "*************** RUST KERNEL PANIC at %.*s:%u ***************\n",
                "\0"
            )
            .as_ptr(),
            i32::try_from(file.len()).unwrap_or(i32::MAX),
            file.as_ptr(),
            line,
        );
    }

    let debugger_not_present = unsafe { KdRefreshDebuggerNotPresent() };

    if !debugger_not_present {
        unsafe {
            DbgBreakPointWithStatus(DEBUG_BREAK_STATUS);
        }
    }

    unsafe {
        KeBugCheckEx(
            RUST_KERNEL_BUGCHECK,
            line as usize,
            file.as_ptr() as usize,
            file.len(),
            RESERVED_BUGCHECK_PARAMETER,
        )
    }
}
