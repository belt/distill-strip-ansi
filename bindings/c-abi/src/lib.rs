use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;

const ABI_VERSION: u32 = 1;
const ERROR_INVALID_ARGUMENT: isize = -1;
const ERROR_PANIC: isize = -2;

#[unsafe(no_mangle)]
pub extern "C" fn dsa_abi_version() -> u32 {
    ABI_VERSION
}

/// Strip ANSI control sequences into a caller-owned output buffer.
///
/// `output` must be writable for `input_len` bytes, and input/output must not
/// overlap. Returns the output byte count, or a negative error code.
/// `-1` means an invalid pointer or an input too large for `isize`; `-2` means
/// the Rust implementation panicked.
///
/// # Safety
///
/// For nonzero `input_len`, `input` must be readable and `output` writable for
/// `input_len` bytes. The regions must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsa_strip(input: *const u8, input_len: usize, output: *mut u8) -> isize {
    if input_len > isize::MAX as usize || (input_len > 0 && (input.is_null() || output.is_null())) {
        return ERROR_INVALID_ARGUMENT;
    }
    if input_len == 0 {
        return 0;
    }

    catch_unwind(AssertUnwindSafe(|| {
        let input = unsafe { slice::from_raw_parts(input, input_len) };
        let stripped = strip_ansi::strip(input).into_owned();
        let output_len = stripped.len();
        unsafe { ptr::copy_nonoverlapping(stripped.as_ptr(), output, output_len) };
        output_len as isize
    }))
    .unwrap_or(ERROR_PANIC)
}

/// Return 1 if `input` contains an ANSI escape sequence, 0 if not, or -1 for
/// an invalid pointer.
///
/// # Safety
///
/// For nonzero `input_len`, `input` must be readable for `input_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsa_contains_ansi(input: *const u8, input_len: usize) -> i32 {
    if input_len > isize::MAX as usize || (input_len > 0 && input.is_null()) {
        return -1;
    }
    if input_len == 0 {
        return 0;
    }
    let input = unsafe { slice::from_raw_parts(input, input_len) };
    i32::from(strip_ansi::contains_ansi(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_into_caller_buffer() {
        let input = b"a\x1b[31mb\x1b[0m";
        let mut output = vec![0; input.len()];
        let output_len = unsafe { dsa_strip(input.as_ptr(), input.len(), output.as_mut_ptr()) };
        assert_eq!(output_len, 2);
        assert_eq!(&output[..output_len as usize], b"ab");
    }

    #[test]
    fn accepts_empty_input_and_rejects_null_nonempty_input() {
        assert_eq!(unsafe { dsa_strip(ptr::null(), 0, ptr::null_mut()) }, 0);
        assert_eq!(
            unsafe { dsa_strip(ptr::null(), 1, ptr::null_mut()) },
            ERROR_INVALID_ARGUMENT
        );
        assert_eq!(unsafe { dsa_contains_ansi(ptr::null(), 0) }, 0);
        assert_eq!(unsafe { dsa_contains_ansi(ptr::null(), 1) }, -1);
    }

    #[test]
    fn reports_abi_version_and_escape_presence() {
        assert_eq!(dsa_abi_version(), ABI_VERSION);
        assert_eq!(unsafe { dsa_contains_ansi(b"plain".as_ptr(), 5) }, 0);
        assert_eq!(unsafe { dsa_contains_ansi(b"\x1b[31m".as_ptr(), 5) }, 1);
    }
}
