use core::{panic::PanicInfo, ptr::null};
use windows_sys::{core::PCSTR, Wdk::System::SystemServices::{
    DbgBreakPointWithStatus, DbgPrintEx, KdRefreshDebuggerNotPresent, KeBugCheckEx
}};

const RUST_KERNEL_BUGCHECK: u32 = 0xE0525354;

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
        if !file_ptr.is_null() {
            DbgPrintEx(
                0,
                0,
                PCSTR::from(c"*************** RUST KERNEL PANIC at %.*s:%u ***************\n".as_ptr().cast()),
                file_len,
                file_ptr,
                line,
            );
        } else {
            DbgPrintEx(
                0,
                0,
                PCSTR::from(c"*************** RUST KERNEL PANIC (unknown location) ***************\n".as_ptr().cast()),
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
            file_len as usize,
            0,
        );
    }

    unreachable!();
}
