//! Versão real do Windows via `RtlGetVersion` (presente desde o XP, não sofre o
//! "version lie" do `GetVersionEx` sem manifesto).

#[cfg(windows)]
pub fn describe() -> String {
    #[repr(C)]
    struct OsVersionInfoW {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform_id: u32,
        csd_version: [u16; 128],
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(info: *mut OsVersionInfoW) -> i32;
    }

    let mut info = OsVersionInfoW {
        size: std::mem::size_of::<OsVersionInfoW>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform_id: 0,
        csd_version: [0; 128],
    };
    // SAFETY: estrutura com o layout de OSVERSIONINFOW e `size` preenchido.
    if unsafe { RtlGetVersion(&mut info) } != 0 {
        return "Windows (versão desconhecida)".into();
    }
    let sp_len = info.csd_version.iter().position(|&c| c == 0).unwrap_or(0);
    let sp = String::from_utf16_lossy(&info.csd_version[..sp_len]);
    format!(
        "Windows {}.{}.{} {} ({})",
        info.major,
        info.minor,
        info.build,
        sp,
        std::env::consts::ARCH
    )
    .replace("  ", " ")
}

#[cfg(not(windows))]
pub fn describe() -> String {
    format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH)
}
