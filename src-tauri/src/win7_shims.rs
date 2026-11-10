//! Shims de link para rodar no Windows 7.
//!
//! O `WebView2LoaderStatic.lib` (C, distribuído pelo `webview2-com-sys`) importa
//! `advapi32!EventSetInformation` estaticamente. Essa função só existe a partir do
//! Windows 8, então no Win7 o executável nem carregaria. Ela serve apenas para
//! telemetria ETW do loader.
//!
//! Definimos aqui o próprio símbolo `__imp_EventSetInformation`, o ponteiro da
//! IAT que o código C usa. O linker resolve com a nossa definição antes de procurar
//! na `advapi32.lib`, e o import some do executável. Em runtime, chamamos a
//! função real se ela existir; senão devolvemos ERROR_NOT_SUPPORTED, que o
//! TraceLogging trata como "recurso indisponível".
//!
//! O `win7-check` no CI confirma que o import não volta.

use std::{
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};

type EventSetInformationFn =
    unsafe extern "system" fn(reg_handle: u64, class: i32, info: *mut c_void, len: u32) -> u32;

const ERROR_NOT_SUPPORTED: u32 = 50;
const UNRESOLVED: usize = 0;
const MISSING: usize = 1;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
}

static REAL: AtomicUsize = AtomicUsize::new(UNRESOLVED);

unsafe extern "system" fn event_set_information(
    reg_handle: u64,
    class: i32,
    info: *mut c_void,
    len: u32,
) -> u32 {
    let mut addr = REAL.load(Ordering::Acquire);
    if addr == UNRESOLVED {
        let dll: Vec<u16> = "advapi32.dll\0".encode_utf16().collect();
        // SAFETY: strings terminadas em NUL; advapi32 está sempre presente.
        let f = unsafe { GetProcAddress(LoadLibraryW(dll.as_ptr()), c"EventSetInformation".as_ptr().cast()) };
        addr = if f.is_null() { MISSING } else { f as usize };
        REAL.store(addr, Ordering::Release);
    }
    if addr == MISSING {
        return ERROR_NOT_SUPPORTED;
    }
    // SAFETY: endereço obtido de GetProcAddress para a função com esta assinatura.
    let real: EventSetInformationFn = unsafe { std::mem::transmute(addr) };
    unsafe { real(reg_handle, class, info, len) }
}

// x64: sem decoração. x86 (stdcall): `_EventSetInformation@20` (8+4+4+4 bytes de
// argumentos); o `\x01` impede o LLVM de acrescentar outro `_` ao nome.
#[cfg(target_arch = "x86_64")]
#[used]
#[export_name = "__imp_EventSetInformation"]
static IMP_EVENT_SET_INFORMATION: EventSetInformationFn = event_set_information;

#[cfg(target_arch = "x86")]
#[used]
#[export_name = "\x01__imp__EventSetInformation@20"]
static IMP_EVENT_SET_INFORMATION: EventSetInformationFn = event_set_information;
