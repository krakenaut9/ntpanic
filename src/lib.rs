#![no_std]
#![warn(missing_docs)]
// Here rather than in Cargo.toml's [lints], which Cargo before 1.74 ignores.
#![warn(clippy::all, clippy::pedantic)]
#![doc = include_str!("../README.md")]

use core::sync::atomic::{AtomicU32, Ordering};

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
/// kernel_panic::set_component_id(DPFLTR_IHVNETWORK_ID);
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
/// kernel_panic::set_level(DPFLTR_WARNING_LEVEL);
/// ```
///
/// [Reading and Filtering Debugging Messages]: https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/reading-and-filtering-debugging-messages
pub fn set_level(level: u32) {
    LEVEL.store(level, Ordering::Relaxed);
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
}
