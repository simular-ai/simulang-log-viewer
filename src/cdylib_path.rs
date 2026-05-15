//! Discover the on-disk path of *this* cdylib (the loaded `.node` file).
//!
//! The npm package ships the `simulang-log-viewer` executable next to the
//! `.node` file, so once we know where the cdylib was loaded from we can
//! derive the binary path with one `Path::with_file_name`. JS doesn't need
//! to pass `__dirname` in via the constructor any more.
//!
//! Implementation: minimal inline FFI. On Unix, `dladdr` resolves any
//! address inside a loaded shared object back to its `dli_fname`. On
//! Windows, `GetModuleHandleExW` with `FROM_ADDRESS` does the same. Both
//! are passed a function pointer from this module, which is guaranteed to
//! sit inside the cdylib's text segment.

use std::path::PathBuf;

/// Returns the absolute path of the currently-loaded cdylib (`.node`) on
/// disk, or `None` if the platform call fails.
pub fn current() -> Option<PathBuf> {
    current_impl()
}

#[cfg(unix)]
fn current_impl() -> Option<PathBuf> {
    use std::ffi::{CStr, c_char, c_int, c_void};

    #[repr(C)]
    struct DlInfo {
        dli_fname: *const c_char,
        dli_fbase: *mut c_void,
        dli_sname: *const c_char,
        dli_saddr: *mut c_void,
    }

    unsafe extern "C" {
        fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
    }

    // SAFETY: `dladdr` accepts any pointer; we initialise `info` to all
    // zeros, and only read `dli_fname` if the call succeeds. The address we
    // pass is `current_impl` itself, which is guaranteed to live inside this
    // cdylib's text segment.
    let mut info: DlInfo = unsafe { std::mem::zeroed() };
    let res = unsafe { dladdr(current_impl as *const c_void, &mut info) };
    if res == 0 || info.dli_fname.is_null() {
        return None;
    }
    // SAFETY: dladdr guarantees `dli_fname` is a NUL-terminated string when
    // the call succeeds and the pointer is non-null.
    let cstr = unsafe { CStr::from_ptr(info.dli_fname) };
    Some(PathBuf::from(cstr.to_string_lossy().into_owned()))
}

#[cfg(windows)]
fn current_impl() -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    type Hmodule = *mut std::ffi::c_void;
    type Dword = u32;
    type Wchar = u16;
    type Lpcwstr = *const u16;

    const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: Dword = 0x0000_0004;
    const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: Dword = 0x0000_0002;

    unsafe extern "system" {
        fn GetModuleHandleExW(
            dw_flags: Dword,
            lp_module_name: Lpcwstr,
            ph_module: *mut Hmodule,
        ) -> i32;
        fn GetModuleFileNameW(h_module: Hmodule, lp_filename: *mut Wchar, n_size: Dword) -> Dword;
    }

    let mut module: Hmodule = std::ptr::null_mut();
    let flags =
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;

    // SAFETY: `current_impl` is a function in this cdylib; treating its
    // address as an LPCWSTR is allowed by the FROM_ADDRESS flag (the
    // documented Win32 contract is that `lpModuleName` is reinterpreted as
    // an arbitrary address pointing inside the desired module).
    let ok = unsafe { GetModuleHandleExW(flags, current_impl as *const Wchar, &mut module) };
    if ok == 0 || module.is_null() {
        return None;
    }

    let mut buffer = [0u16; 4096];
    // SAFETY: We pass a writable buffer of `len` u16s. `GetModuleFileNameW`
    // writes at most `n_size` u16s and returns the number written.
    let len = unsafe { GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as Dword) };
    if len == 0 {
        return None;
    }
    let path = OsString::from_wide(&buffer[..len as usize]);
    Some(PathBuf::from(path))
}

#[cfg(not(any(unix, windows)))]
fn current_impl() -> Option<PathBuf> {
    None
}

/// Convenience: cdylib is at `<dir>/simulang-log-viewer.<triple>.node`,
/// the binary is at `<dir>/simulang-log-viewer[.exe]`.
pub fn viewer_binary() -> Option<PathBuf> {
    let cdylib = current()?;
    let dir = cdylib.parent()?;
    let exe = if cfg!(target_os = "windows") {
        "simulang-log-viewer.exe"
    } else {
        "simulang-log-viewer"
    };
    Some(dir.join(exe))
}
