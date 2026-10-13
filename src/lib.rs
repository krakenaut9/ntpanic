#![no_std]
#![warn(missing_docs)]
// Here rather than in Cargo.toml's [lints], which Cargo before 1.74 ignores.
#![warn(clippy::all, clippy::pedantic)]
#![doc = include_str!("../README.md")]

use core::{
    mem,
    panic::PanicInfo,
    ptr,
    sync::atomic::{AtomicPtr, AtomicU32, Ordering},
};

#[cfg(not(test))]
mod panic;

// DbgPrintEx filter values from the WDK's dpfilter.h, which windows-sys does not define.
const DPFLTR_IHVDRIVER_ID: u32 = 77;
const DPFLTR_ERROR_LEVEL: u32 = 0;

// Read by the panic handler. Each value stands alone, so Relaxed ordering is enough.
static COMPONENT_ID: AtomicU32 = AtomicU32::new(DPFLTR_IHVDRIVER_ID);
static LEVEL: AtomicU32 = AtomicU32::new(DPFLTR_ERROR_LEVEL);

/// Sets the `DbgPrintEx` component ID that the panic message is printed under.
///
/// The default is `DPFLTR_IHVDRIVER_ID` (77). Microsoft asks drivers to use one of the
/// six component IDs reserved for independent hardware vendors in the WDK's
/// `dpfilter.h`, so that their output is not mixed with that of Windows components:
///
/// | Component ID | Value | Driver type |
/// |---|---|---|
/// | `DPFLTR_IHVDRIVER_ID` | 77 | any other type of driver |
/// | `DPFLTR_IHVVIDEO_ID` | 78 | video |
/// | `DPFLTR_IHVAUDIO_ID` | 79 | audio |
/// | `DPFLTR_IHVNETWORK_ID` | 80 | network |
/// | `DPFLTR_IHVSTREAMING_ID` | 81 | kernel streaming |
/// | `DPFLTR_IHVBUS_ID` | 82 | bus |
///
/// Can be called at any IRQL. A panic before the first call uses the default, so call
/// it early, for example at the start of `DriverEntry`.
///
/// # Examples
///
/// ```ignore
/// // From the WDK's dpfilter.h.
/// const DPFLTR_IHVNETWORK_ID: u32 = 80;
///
/// ntpanic::set_component_id(DPFLTR_IHVNETWORK_ID);
/// ```
pub fn set_component_id(component_id: u32) {
    COMPONENT_ID.store(component_id, Ordering::Relaxed);
}

/// Sets the `DbgPrintEx` level that the panic message is printed at.
///
/// The default is `DPFLTR_ERROR_LEVEL` (0). Windows combines every component's filter
/// mask with a system-wide mask that enables only this level, so by default the panic
/// message is shown whatever the component. Any other level is shown only if the
/// component's filter mask enables it: changing the level can hide the panic message.
///
/// As with `DbgPrintEx`, a level from 0 to 31 selects a single bit (`1 << level`), and a
/// larger value is used as the bit field itself. The WDK's `dpfilter.h` names the first
/// four: `DPFLTR_ERROR_LEVEL` (0), `DPFLTR_WARNING_LEVEL` (1), `DPFLTR_TRACE_LEVEL` (2)
/// and `DPFLTR_INFO_LEVEL` (3). See [Reading and Filtering Debugging Messages].
///
/// Can be called at any IRQL. A panic before the first call uses the default.
///
/// # Examples
///
/// ```ignore
/// // From the WDK's dpfilter.h.
/// const DPFLTR_WARNING_LEVEL: u32 = 1;
///
/// ntpanic::set_level(DPFLTR_WARNING_LEVEL);
/// ```
///
/// [Reading and Filtering Debugging Messages]: https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/reading-and-filtering-debugging-messages
pub fn set_level(level: u32) {
    LEVEL.store(level, Ordering::Relaxed);
}

/// A function for the panic handler to call, with the panic and the IRQL it occurred at;
/// see [`set_callback`].
pub type Callback = fn(&PanicInfo<'_>, u8);

// The callback as a pointer, null when there is none. Stored with Release and read with
// Acquire, so that a panic on another processor sees what was written before setting it.
static CALLBACK: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

/// Sets a function for the panic handler to call before it bug-checks, or removes it.
///
/// The handler calls it with the panic, which gives its location and message, and the
/// IRQL the panic occurred at. It does so after printing the panic message, which it
/// skips above DIRQL, and before breaking into a debugger. Only the first panic calls it:
/// a later one, even inside the function itself, bug-checks without calling any callback,
/// including one set in the meantime.
///
/// The function runs where the panic occurred: on that thread's stack, at any IRQL up to
/// `HIGH_LEVEL`, and with the system in an unknown state. It must not wait or take locks,
/// must not allocate or touch pageable memory at `DISPATCH_LEVEL` or above, and must not
/// call `DbgPrintEx` above DIRQL. If it never returns, the system hangs instead of
/// bug-checking. To reset a device or add data to the crash dump, use a bug check callback
/// (`KeRegisterBugCheckReasonCallback`) instead: it is made for that, and it also runs for
/// bug checks that are not panics.
///
/// Can be called at any IRQL.
///
/// # Examples
///
/// ```ignore
/// use core::panic::PanicInfo;
///
/// // From the WDK's wdm.h.
/// const DISPATCH_LEVEL: u8 = 2;
///
/// fn log_panic(info: &PanicInfo, irql: u8) {
///     // A logger may allocate or touch pageable memory, which is safe only below
///     // DISPATCH_LEVEL.
///     if irql < DISPATCH_LEVEL {
///         log::error!("{info}");
///     }
/// }
///
/// ntpanic::set_callback(Some(log_panic));
/// ```
pub fn set_callback(callback: Option<Callback>) {
    let callback = callback.map_or(ptr::null_mut(), |callback| callback as *mut ());
    CALLBACK.store(callback, Ordering::Release);
}

/// The callback, if one is set.
fn current_callback() -> Option<Callback> {
    let callback = CALLBACK.load(Ordering::Acquire);
    if callback.is_null() {
        None
    } else {
        // Only `set_callback` stores a non-null value, and it stores a `Callback`.
        Some(unsafe { mem::transmute::<*mut (), Callback>(callback) })
    }
}

// DbgPrintEx transmits at most 512 bytes per call. The panic message keeps only the
// end of a long path, its informative part, and gives it half of that budget; the
// other half is left for the rest of the message.
const MAX_PRINTED_PATH_LEN: usize = 256;

/// The part of `path` that the panic message prints: all of it, or its last
/// `MAX_PRINTED_PATH_LEN` bytes.
fn printed_path(path: &str) -> &[u8] {
    let bytes = path.as_bytes();
    bytes
        .get(bytes.len().saturating_sub(MAX_PRINTED_PATH_LEN)..)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // From the WDK's dpfilter.h.
    const DPFLTR_IHVNETWORK_ID: u32 = 80;
    const DPFLTR_INFO_LEVEL: u32 = 3;

    // One byte more than is printed, so exactly the first byte must be cut.
    const OVERLONG_PATH_LEN: usize = MAX_PRINTED_PATH_LEN + 1;

    fn callback(_info: &PanicInfo<'_>, _irql: u8) {}

    #[test]
    fn printed_path_keeps_a_path_of_the_maximum_length_whole() {
        let bytes = [b'a'; MAX_PRINTED_PATH_LEN];
        let path = core::str::from_utf8(&bytes).unwrap();
        assert_eq!(printed_path(path), &bytes[..]);
    }

    #[test]
    fn printed_path_keeps_the_end_of_a_longer_path() {
        let mut bytes = [b'a'; OVERLONG_PATH_LEN];
        // Mark the byte that must be cut, so that keeping the start instead fails.
        *bytes.first_mut().unwrap() = b'X';
        let path = core::str::from_utf8(&bytes).unwrap();
        assert_eq!(
            printed_path(path),
            &bytes[OVERLONG_PATH_LEN - MAX_PRINTED_PATH_LEN..]
        );
    }

    // One test, because the settings are global and tests run in parallel.
    #[test]
    fn settings_start_at_the_defaults_and_follow_the_setters() {
        assert_eq!(COMPONENT_ID.load(Ordering::Relaxed), DPFLTR_IHVDRIVER_ID);
        assert_eq!(LEVEL.load(Ordering::Relaxed), DPFLTR_ERROR_LEVEL);

        set_component_id(DPFLTR_IHVNETWORK_ID);
        set_level(DPFLTR_INFO_LEVEL);

        assert_eq!(COMPONENT_ID.load(Ordering::Relaxed), DPFLTR_IHVNETWORK_ID);
        assert_eq!(LEVEL.load(Ordering::Relaxed), DPFLTR_INFO_LEVEL);
    }

    // One test, because the callback is global and tests run in parallel.
    #[test]
    fn the_callback_starts_unset_and_follows_the_setter() {
        assert!(current_callback().is_none());

        set_callback(Some(callback));
        assert_eq!(
            current_callback().map(|current| current as *const ()),
            Some(callback as *const ())
        );

        set_callback(None);
        assert!(current_callback().is_none());
    }
}
