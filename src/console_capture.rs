// src/console_capture.rs

use winapi::shared::minwindef::{DWORD, FALSE};
use winapi::shared::ntdef::NULL;
use winapi::um::processenv::GetStdHandle;
use winapi::um::winbase::STD_OUTPUT_HANDLE;
use winapi::um::wincon::{
    GetConsoleScreenBufferInfo, ReadConsoleOutputCharacterW, CONSOLE_SCREEN_BUFFER_INFO,
};
use winapi::um::winnt::HANDLE;

/// Converts a slice of wide characters (UTF-16) to a Rust `String`.
fn wide_to_string(wide: &[u16]) -> String {
    String::from_utf16_lossy(wide)
}

/// Captures the last few lines of the Windows console output.
///
/// This function reads the last `lines_to_capture` lines from the console's screen buffer.
/// It is tailored for Windows systems.
///
/// # Returns
///
/// A `String` containing the captured console output.
pub fn get_last_console_output() -> String {
    unsafe {
        // Get the handle to the standard output
        let handle: HANDLE = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle == NULL {
            log::error!("Failed to get standard output handle.");
            return String::new();
        }

        // Retrieve console screen buffer info
        let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(handle, &mut csbi) == FALSE {
            log::error!("Failed to get console screen buffer info.");
            return String::new();
        }

        let buffer_width = csbi.dwSize.X as usize;
        let buffer_height = csbi.dwSize.Y as usize;

        // Define how many lines you want to capture
        let lines_to_capture = 10.min(buffer_height); // Capture last 10 lines or less

        let mut output = String::new();

        for i in 0..lines_to_capture {
            let y = csbi.dwCursorPosition.Y.saturating_sub(i as i16 + 1);
            if y < 0 {
                break;
            }

            let mut buffer: Vec<u16> = vec![0; buffer_width];
            let mut chars_read: DWORD = 0;

            // Read a single line from the console buffer
            if ReadConsoleOutputCharacterW(
                handle,
                buffer.as_mut_ptr(),
                buffer_width as DWORD,
                winapi::um::wincon::COORD { X: 0, Y: y },
                &mut chars_read,
            ) == FALSE
            {
                log::error!("Failed to read console output.");
                continue;
            }

            // Convert wide characters to String
            let line = wide_to_string(&buffer[..chars_read as usize]);
            output = format!("{}\n{}", line, output);
        }

        output.trim_start().to_string()
    }
}
