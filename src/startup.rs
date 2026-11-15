//! "Start with Windows" support through the per-user Run key.

use crate::win32::*;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "CrosshairX-Open";

fn current_command() -> String {
    match std::env::current_exe() {
        Ok(path) => format!("\"{}\"", path.display()),
        Err(_) => String::new(),
    }
}

/// Add or remove the autostart entry. Returns a human-readable status.
pub fn set_launch_on_startup(enabled: bool) -> Result<(), String> {
    let sub_key = wide(RUN_KEY);
    let value_name = wide(VALUE_NAME);
    unsafe {
        let mut key: Hkey = 0;
        let mut disposition = 0u32;
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            sub_key.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            &mut disposition,
        );
        if status != 0 || key == 0 {
            return Err(format!("cannot open the Run key (error {status})"));
        }
        let result = if enabled {
            let command = current_command();
            if command.is_empty() {
                RegCloseKey(key);
                return Err("cannot determine the executable path".to_string());
            }
            let mut data: Vec<u8> = Vec::new();
            for unit in command.encode_utf16() {
                data.extend_from_slice(&unit.to_le_bytes());
            }
            data.extend_from_slice(&[0, 0]);
            let status = RegSetValueExW(
                key,
                value_name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr(),
                data.len() as u32,
            );
            if status == 0 {
                Ok(())
            } else {
                Err(format!("cannot write the Run entry (error {status})"))
            }
        } else {
            // Removing a value that is not there is fine.
            RegDeleteValueW(key, value_name.as_ptr());
            Ok(())
        };
        RegCloseKey(key);
        result
    }
}

/// Whether the Run entry currently exists.
pub fn is_launch_on_startup() -> bool {
    let sub_key = wide(RUN_KEY);
    let value_name = wide(VALUE_NAME);
    unsafe {
        let mut key: Hkey = 0;
        let mut disposition = 0u32;
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            sub_key.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            0x0001, // KEY_QUERY_VALUE
            std::ptr::null(),
            &mut key,
            &mut disposition,
        );
        if status != 0 || key == 0 {
            return false;
        }
        let mut buffer = [0u8; 1024];
        let mut length = buffer.len() as u32;
        let status = RegQueryValueExW(
            key,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            buffer.as_mut_ptr(),
            &mut length,
        );
        RegCloseKey(key);
        status == 0
    }
}
