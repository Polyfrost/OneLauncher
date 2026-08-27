use anyhow::{Result, bail};
use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR};

fn check(status: WIN32_ERROR, what: &str) -> Result<()> {
    if status == ERROR_SUCCESS {
        return Ok(());
    }
    bail!("{what} failed with Windows error {}", status.0)
}

fn close(key: HKEY) {
    unsafe {
        let _ = RegCloseKey(key);
    };
}

fn name_ptr(name: Option<&HSTRING>) -> PCWSTR {
    name.map_or_else(PCWSTR::null, |name| PCWSTR(name.as_ptr()))
}

pub fn write_string(subkey: &str, name: Option<&str>, value: &str) -> Result<()> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(subkey),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &raw mut key,
            None,
        )
    };
    check(status, "creating the registry key")?;

    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };

    let name = name.map(HSTRING::from);
    let status =
        unsafe { RegSetValueExW(key, name_ptr(name.as_ref()), None, REG_SZ, Some(bytes)) };
    close(key);

    check(status, "writing the registry value")
}

pub fn read_string(subkey: &str, name: Option<&str>) -> Option<String> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(subkey),
            None,
            KEY_READ,
            &raw mut key,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }

    let name = name.map(HSTRING::from);
    let name = name_ptr(name.as_ref());

    let mut size = 0u32;
    let status = unsafe { RegQueryValueExW(key, name, None, None, None, Some(&raw mut size)) };
    if status != ERROR_SUCCESS || size == 0 {
        close(key);
        return None;
    }

    let mut buffer = vec![0u8; size as usize];
    let status = unsafe {
        RegQueryValueExW(
            key,
            name,
            None,
            None,
            Some(buffer.as_mut_ptr()),
            Some(&raw mut size),
        )
    };
    close(key);
    if status != ERROR_SUCCESS {
        return None;
    }

    buffer.truncate(size as usize);
    let wide: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();

    Some(
        String::from_utf16_lossy(&wide)
            .trim_end_matches('\0')
            .to_string(),
    )
}
