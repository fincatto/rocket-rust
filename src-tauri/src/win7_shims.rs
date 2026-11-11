//! Shims de link para rodar no Windows 7.
//!
//! Algumas dependências importam estaticamente funções que o Win7 não tem:
//! - `windows`/`windows-core` ligam `CoTaskMemAlloc`, `CoTaskMemFree` e
//!   `CoCreateFreeThreadedMarshaler` à `combase.dll`, que só existe a partir do Win8.
//!   As mesmas funções existem na `ole32.dll` desde sempre; no Win8+ a `ole32.dll`
//!   só encaminha para a `combase.dll`.
//! - o `WebView2LoaderStatic.lib` (C, do `webview2-com-sys`) importa
//!   `advapi32!EventSetInformation` (Win8+), que só serve para telemetria ETW.
//!
//! Com um import estático ausente, o Windows recusa carregar o executável. Por isso
//! definimos aqui os próprios ponteiros de IAT (`__imp_<nome>`). O linker usa a
//! nossa definição antes de procurar nas import libs, e o import some do
//! executável. Em runtime, cada shim resolve a função real com `GetProcAddress`
//! na primeira chamada.
//!
//! O `win7-check` no CI confirma que nenhum desses imports volta.

use std::{
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
}

const UNRESOLVED: usize = 0;
const MISSING: usize = 1;

/// Endereço da função real (ou `MISSING`), resolvido uma vez e guardado em `cache`.
fn resolve(cache: &AtomicUsize, dll: &str, name: &std::ffi::CStr) -> usize {
    let addr = cache.load(Ordering::Acquire);
    if addr != UNRESOLVED {
        return addr;
    }
    let dll: Vec<u16> = dll.encode_utf16().chain(Some(0)).collect();
    // SAFETY: strings terminadas em NUL; LoadLibraryW de DLL do sistema.
    let f = unsafe { GetProcAddress(LoadLibraryW(dll.as_ptr()), name.as_ptr().cast()) };
    let addr = if f.is_null() { MISSING } else { f as usize };
    cache.store(addr, Ordering::Release);
    addr
}

/// Define `__imp_<nome>` apontando para um shim que repassa para `<dll>!<nome>`
/// ou devolve `$fallback` se a função não existir.
///
/// `$x86` é o nome decorado stdcall (`_Nome@<bytes de argumentos>`) sem o `_`
/// inicial, que o LLVM acrescenta sozinho no i686.
macro_rules! shim {
    ($dll:literal, $name:ident, $x86:literal,
     fn($($arg:ident: $ty:ty),*) $(-> $ret:ty)?, fallback = $fallback:expr) => {
        #[allow(non_snake_case, clippy::unused_unit)]
        mod $name {
            use super::*;

            type Fn = unsafe extern "system" fn($($ty),*) $(-> $ret)?;
            static REAL: AtomicUsize = AtomicUsize::new(UNRESOLVED);

            unsafe extern "system" fn shim($($arg: $ty),*) $(-> $ret)? {
                let addr = resolve(&REAL, $dll, ::std::ffi::CStr::from_bytes_with_nul(
                    concat!(stringify!($name), "\0").as_bytes()).unwrap());
                if addr == MISSING {
                    return $fallback;
                }
                // SAFETY: endereço de GetProcAddress para a função com esta assinatura.
                let real: Fn = unsafe { std::mem::transmute(addr) };
                unsafe { real($($arg),*) }
            }

            #[cfg(target_arch = "x86_64")]
            #[used]
            #[export_name = concat!("__imp_", stringify!($name))]
            static IMP: Fn = shim;

            #[cfg(target_arch = "x86")]
            #[used]
            #[export_name = concat!("_imp__", $x86)]
            static IMP: Fn = shim;
        }
    };
}

// ERROR_NOT_SUPPORTED: o TraceLogging trata como "recurso indisponível".
shim!("advapi32.dll", EventSetInformation, "EventSetInformation@20",
      fn(reg_handle: u64, class: i32, info: *mut c_void, len: u32) -> u32, fallback = 50);

// ole32.dll sempre tem estas três; os fallbacks nunca devem ser usados.
shim!("ole32.dll", CoTaskMemAlloc, "CoTaskMemAlloc@4",
      fn(cb: usize) -> *mut c_void, fallback = std::ptr::null_mut());
shim!("ole32.dll", CoTaskMemFree, "CoTaskMemFree@4",
      fn(pv: *const c_void), fallback = ());
// E_NOTIMPL
shim!("ole32.dll", CoCreateFreeThreadedMarshaler, "CoCreateFreeThreadedMarshaler@8",
      fn(outer: *mut c_void, marshal: *mut *mut c_void) -> i32, fallback = 0x8000_4001_u32 as i32);
