//! The desk readings, taken out of the IO registry in process.
//!
//! WHY THE SYMBOLS ARE DECLARED HERE rather than taken from a binding crate:
//! six functions and two constants is the whole surface this needs, which is
//! smaller than either `io-kit-sys` or `core-foundation` would add to the
//! dependency graph of a tool other people install.

use std::ffi::{CStr, c_char, c_int, c_uint, c_void};

/// The two desk readings the registry answers, behind a seam so a test can
/// drive a locked console, an unreadable device or a call that never returns
/// without touching the machine the suite runs on.
pub trait ConsoleRegistry: Send + Sync + 'static {
    /// Nanoseconds since the last human input at the desk, or None where the
    /// property could not be read.
    fn idle_nanoseconds(&self) -> Option<u64>;

    /// Whether the console is locked, from the Root node's own aggregate, or
    /// None where the property could not be read.
    fn console_locked(&self) -> Option<bool>;
}

/// The live registry.
pub struct IoKitRegistry;

impl ConsoleRegistry for IoKitRegistry {
    /// `HIDIdleTime` off the `IOHIDSystem` service, which is the property
    /// `ioreg -c IOHIDSystem` was being dumped and line-searched for.
    fn idle_nanoseconds(&self) -> Option<u64> {
        let service = matching_service(c"IOHIDSystem")?;
        let reading = number_property(service, c"HIDIdleTime");
        // SAFETY: `service` is a live object returned by
        // IOServiceGetMatchingService and released exactly once here.
        unsafe { IOObjectRelease(service) };
        reading
    }

    /// `IOConsoleLocked` off the registry Root node: the aggregate the kernel
    /// already computed, which is what `ioreg -n Root -d1` was printing.
    ///
    /// NOT `CGSessionCopyCurrentDictionary`, whose per-session
    /// `CGSSessionScreenIsLocked` answers for the window session the calling
    /// process belongs to and is null in a session that has none. Given the
    /// fail direction below, a null there would silently stop locking.
    fn console_locked(&self) -> Option<bool> {
        let root = root_entry()?;
        let reading = boolean_property(root, c"IOConsoleLocked");
        // SAFETY: `root` is a live object returned by IORegistryGetRootEntry
        // and released exactly once here.
        unsafe { IOObjectRelease(root) };
        reading
    }
}

/// The first service matching a class name, or None where nothing matched.
fn matching_service(class: &CStr) -> Option<io_object_t> {
    // SAFETY: the name is NUL terminated and copied by IOServiceMatching; the
    // dictionary it returns is consumed (and released) by
    // IOServiceGetMatchingService, so it is not released again here.
    let service = unsafe {
        let matching = IOServiceMatching(class.as_ptr());
        if matching.is_null() {
            return None;
        }
        IOServiceGetMatchingService(DEFAULT_MAIN_PORT, matching)
    };
    (service != 0).then_some(service)
}

/// The registry's Root node, or None where the port refused it.
fn root_entry() -> Option<io_object_t> {
    // SAFETY: no arguments to get wrong; the returned object is owned by the
    // caller and released by the one caller above.
    let root = unsafe { IORegistryGetRootEntry(DEFAULT_MAIN_PORT) };
    (root != 0).then_some(root)
}

/// One integer property, refused unless the registry really answered a number.
fn number_property(entry: io_object_t, key: &CStr) -> Option<u64> {
    let property = copy_property(entry, key)?;
    let mut value: i64 = 0;
    // SAFETY: the type is checked before the read, so the reference is a
    // CFNumber, and `value` is a 64-bit slot matching the requested type.
    let read = unsafe {
        CFGetTypeID(property) == CFNumberGetTypeID()
            && CFNumberGetValue(property, CF_NUMBER_SINT64_TYPE, (&raw mut value).cast())
    };
    // SAFETY: the property came from a Create call, so it is owned here.
    unsafe { CFRelease(property) };
    read.then(|| u64::try_from(value).ok()).flatten()
}

/// One boolean property, refused unless the registry really answered a boolean.
fn boolean_property(entry: io_object_t, key: &CStr) -> Option<bool> {
    let property = copy_property(entry, key)?;
    // SAFETY: the type is checked before the read, so the reference is a
    // CFBoolean.
    let read = unsafe {
        (CFGetTypeID(property) == CFBooleanGetTypeID()).then(|| CFBooleanGetValue(property))
    };
    // SAFETY: the property came from a Create call, so it is owned here.
    unsafe { CFRelease(property) };
    read
}

/// The raw property reference for a key, or None where the node does not
/// carry it.
fn copy_property(entry: io_object_t, key: &CStr) -> Option<CFTypeRef> {
    // SAFETY: the key is NUL terminated and copied into the CFString, which is
    // released here; the property reference is returned to the caller, which
    // owns it.
    unsafe {
        let key_string =
            CFStringCreateWithCString(std::ptr::null(), key.as_ptr(), CF_STRING_ENCODING_UTF8);
        if key_string.is_null() {
            return None;
        }
        let property = IORegistryEntryCreateCFProperty(entry, key_string, std::ptr::null(), 0);
        CFRelease(key_string);
        (!property.is_null()).then_some(property)
    }
}

/// A mach port name, which is what both IOKit object handles are.
#[allow(non_camel_case_types)]
type io_object_t = c_uint;
type CFTypeRef = *const c_void;

/// Zero asks IOKit for the default port, which is what `kIOMainPortDefault`
/// holds.
const DEFAULT_MAIN_PORT: c_uint = 0;
/// `kCFStringEncodingUTF8`.
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
/// `kCFNumberSInt64Type`.
const CF_NUMBER_SINT64_TYPE: c_int = 4;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> CFTypeRef;
    fn IOServiceGetMatchingService(main_port: c_uint, matching: CFTypeRef) -> io_object_t;
    fn IORegistryGetRootEntry(main_port: c_uint) -> io_object_t;
    fn IORegistryEntryCreateCFProperty(
        entry: io_object_t,
        key: CFTypeRef,
        allocator: CFTypeRef,
        options: u32,
    ) -> CFTypeRef;
    fn IOObjectRelease(object: io_object_t) -> c_int;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        allocator: CFTypeRef,
        cstr: *const c_char,
        encoding: u32,
    ) -> CFTypeRef;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(number: CFTypeRef, the_type: c_int, value_ptr: *mut c_void) -> bool;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(boolean: CFTypeRef) -> bool;
    fn CFRelease(cf: CFTypeRef);
}
