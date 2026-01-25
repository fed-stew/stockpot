//! Display Detection Module
//!
//! Provides cross-platform detection of graphical display availability.
//! Used by the `spot` binary to determine whether to launch GUI or TUI mode.

use std::env;

/// Detects whether a graphical display is available on the current system.
///
/// # Platform Behavior
///
/// - **Linux**: Returns `true` if `DISPLAY` or `WAYLAND_DISPLAY` environment
///   variable is set (indicating X11 or Wayland session).
/// - **macOS**: Returns `true` if `SSH_TTY` environment variable is NOT set.
///   macOS generally has display access unless running over SSH.
/// - **Windows**: Always returns `true`. Windows GUI applications typically
///   have display access.
///
/// # Examples
///
/// ```no_run
/// use stockpot::display_detect::has_display;
///
/// if has_display() {
///     println!("Launching GUI mode");
/// } else {
///     println!("Falling back to TUI mode");
/// }
/// ```
#[cfg(target_os = "linux")]
pub fn has_display() -> bool {
    env::var("DISPLAY").is_ok() || env::var("WAYLAND_DISPLAY").is_ok()
}

#[cfg(target_os = "macos")]
pub fn has_display() -> bool {
    // macOS generally has display unless we're in an SSH session
    env::var("SSH_TTY").is_err()
}

#[cfg(target_os = "windows")]
pub fn has_display() -> bool {
    // Windows GUI apps generally have display access
    true
}

// Fallback for other platforms (BSDs, etc.)
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn has_display() -> bool {
    // Conservative fallback: check for DISPLAY like Linux
    env::var("DISPLAY").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_display_returns_bool() {
        // Just verify the function compiles and returns a bool
        let result = has_display();
        let _: bool = result; // Type check proves it returns bool
    }

    #[cfg(target_os = "linux")]
    mod linux_tests {
        use super::*;
        use std::env;

        #[test]
        fn detects_x11_display() {
            // Save original values
            let orig_display = env::var("DISPLAY").ok();
            let orig_wayland = env::var("WAYLAND_DISPLAY").ok();

            // Test with DISPLAY set
            env::set_var("DISPLAY", ":0");
            env::remove_var("WAYLAND_DISPLAY");
            assert!(has_display());

            // Restore
            match orig_display {
                Some(v) => env::set_var("DISPLAY", v),
                None => env::remove_var("DISPLAY"),
            }
            match orig_wayland {
                Some(v) => env::set_var("WAYLAND_DISPLAY", v),
                None => env::remove_var("WAYLAND_DISPLAY"),
            }
        }

        #[test]
        fn detects_wayland_display() {
            // Save original values
            let orig_display = env::var("DISPLAY").ok();
            let orig_wayland = env::var("WAYLAND_DISPLAY").ok();

            // Test with WAYLAND_DISPLAY set
            env::remove_var("DISPLAY");
            env::set_var("WAYLAND_DISPLAY", "wayland-0");
            assert!(has_display());

            // Restore
            match orig_display {
                Some(v) => env::set_var("DISPLAY", v),
                None => env::remove_var("DISPLAY"),
            }
            match orig_wayland {
                Some(v) => env::set_var("WAYLAND_DISPLAY", v),
                None => env::remove_var("WAYLAND_DISPLAY"),
            }
        }
    }

    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::*;
        use std::env;

        #[test]
        fn detects_ssh_session() {
            let orig_ssh_tty = env::var("SSH_TTY").ok();

            // When SSH_TTY is set, no display
            env::set_var("SSH_TTY", "/dev/pts/0");
            assert!(!has_display());

            // Restore
            match orig_ssh_tty {
                Some(v) => env::set_var("SSH_TTY", v),
                None => env::remove_var("SSH_TTY"),
            }
        }
    }
}
