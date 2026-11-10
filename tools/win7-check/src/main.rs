//! Verificador estático de compatibilidade com Windows 7 SP1.
//!
//! `win7-check check [--exports <win7.json>] [--allow <allow.txt>] <arquivo.exe>...`
//!   Falha (código 1) se algum executável:
//!   - exige versão de SO/subsistema maior que 6.1 no cabeçalho PE;
//!   - importa (estaticamente) DLL ou função que o Win7 não tem.
//!   Imports delay-load só geram aviso: costumam ser carregados sob demanda.
//!
//! `win7-check dump <dir> <saida.json>`
//!   Roda DENTRO de um Windows 7 e grava as exportações de todas as DLLs do
//!   diretório (System32 ou SysWOW64). O JSON vira a referência do `check`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use object::{
    pe::{ImageNtHeaders32, ImageNtHeaders64},
    read::pe::{ImageNtHeaders, ImageOptionalHeader, Import, PeFile},
    FileKind, LittleEndian as LE,
};

/// DLLs que não existem no Windows 7 SP1.
const DENY_DLLS: &[&str] = &[
    "combase.dll",
    "shcore.dll",
    "api-ms-win-core-synch-l1-2-0.dll",
    "api-ms-win-core-winrt-l1-1-0.dll",
    "api-ms-win-core-winrt-string-l1-1-0.dll",
    "api-ms-win-core-path-l1-1-0.dll",
    "api-ms-win-core-memory-l1-1-2.dll",
    "api-ms-win-core-sysinfo-l1-2-0.dll",
    "api-ms-win-shcore-scaling-l1-1-1.dll",
    "windows.storage.dll",
    "coremessaging.dll",
];

/// Funções introduzidas depois do Win7 que costumam escapar em binários Rust.
/// Usado mesmo sem o dump completo de exports.
const DENY_FUNCS: &[(&str, &str)] = &[
    ("bcryptprimitives.dll", "ProcessPrng"),
    ("kernel32.dll", "WaitOnAddress"),
    ("kernel32.dll", "WakeByAddressSingle"),
    ("kernel32.dll", "WakeByAddressAll"),
    ("kernel32.dll", "GetSystemTimePreciseAsFileTime"),
    ("kernel32.dll", "SetThreadDescription"),
    ("kernel32.dll", "GetThreadDescription"),
    ("kernel32.dll", "CreateFile2"),
    ("kernel32.dll", "GetCurrentThreadStackLimits"),
    ("kernel32.dll", "PrefetchVirtualMemory"),
    ("kernel32.dll", "SetDefaultDllDirectories"), // só com KB2533623
    ("kernel32.dll", "GetOverlappedResultEx"),
    ("kernel32.dll", "GetFileInformationByName"),
    ("user32.dll", "GetDpiForWindow"),
    ("user32.dll", "GetDpiForSystem"),
    ("user32.dll", "SetProcessDpiAwarenessContext"),
    ("user32.dll", "SetThreadDpiAwarenessContext"),
    ("user32.dll", "AdjustWindowRectExForDpi"),
    ("user32.dll", "GetSystemMetricsForDpi"),
    ("user32.dll", "EnableNonClientDpiScaling"),
    ("user32.dll", "GetPointerType"),
    ("user32.dll", "GetPointerInfo"),
    ("user32.dll", "GetPointerPenInfo"),
    ("user32.dll", "GetPointerTouchInfo"),
    ("user32.dll", "EnableMouseInPointer"),
    ("ws2_32.dll", "GetAddrInfoExW"),
    ("ws2_32.dll", "GetAddrInfoExCancel"),
    ("ntdll.dll", "NtCreateWaitCompletionPacket"),
    ("ntdll.dll", "NtAssociateWaitCompletionPacket"),
    ("ntdll.dll", "NtCancelWaitCompletionPacket"),
    ("advapi32.dll", "EventSetInformation"),
];

/// Windows 7 = NT 6.1.
const MAX_VERSION: (u16, u16) = (6, 1);

type Exports = BTreeMap<String, BTreeSet<String>>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") => check_cmd(&args[1..]),
        Some("dump") if args.len() == 3 => dump_cmd(Path::new(&args[1]), Path::new(&args[2])),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

const USAGE: &str = "uso:
  win7-check check [--exports <win7.json>] [--allow <allow.txt>] <arquivo.exe>...
  win7-check dump <dir-system32> <saida.json>";

struct Imported {
    dll: String,
    /// `None` = import por ordinal.
    func: Option<String>,
    delay: bool,
}

struct PeInfo {
    machine: &'static str,
    os_version: (u16, u16),
    subsystem_version: (u16, u16),
    imports: Vec<Imported>,
}

fn check_cmd(args: &[String]) -> Result<bool, String> {
    let mut exports_path: Option<PathBuf> = None;
    let mut allow_path: Option<PathBuf> = None;
    let mut files = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--exports" => exports_path = it.next().map(PathBuf::from),
            "--allow" => allow_path = it.next().map(PathBuf::from),
            _ => files.push(PathBuf::from(a)),
        }
    }
    if files.is_empty() {
        return Err(USAGE.into());
    }

    let exports = match &exports_path {
        Some(p) if p.exists() => Some(load_exports(p)?),
        Some(p) => {
            println!(
                "AVISO: {} não existe; usando só a lista de APIs proibidas embutida.\n       \
                 Gere-a com `win7-check dump` numa VM Win7 (veja tests/win7-vm/README.md).",
                p.display()
            );
            None
        }
        None => None,
    };
    let allow = match &allow_path {
        Some(p) => load_allow(p)?,
        None => BTreeSet::new(),
    };

    // DLLs entregues junto com o app não estão no Windows, mas são válidas.
    let shipped: BTreeSet<String> = files
        .iter()
        .filter_map(|f| f.file_name())
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .collect();

    let mut ok = true;
    for file in &files {
        ok &= check_file(file, exports.as_ref(), &allow, &shipped)?;
    }
    println!("{}", if ok { "\nOK: compatível com Windows 7 SP1" } else { "\nFALHOU" });
    Ok(ok)
}

fn check_file(
    path: &Path,
    exports: Option<&Exports>,
    allow: &BTreeSet<String>,
    shipped: &BTreeSet<String>,
) -> Result<bool, String> {
    let info = read_pe(path)?;
    println!("== {} ({})", path.display(), info.machine);
    let mut ok = true;

    for (what, v) in [("SO", info.os_version), ("subsistema", info.subsystem_version)] {
        if v > MAX_VERSION {
            println!("  ERRO: versão mínima de {what} {}.{} > 6.1", v.0, v.1);
            ok = false;
        }
    }

    let mut errors = BTreeSet::new();
    let mut warnings = BTreeSet::new();
    for imp in &info.imports {
        let key = match &imp.func {
            Some(f) => format!("{}!{}", imp.dll, f),
            None => format!("{}!#ordinal", imp.dll),
        };
        if allow.contains(&key) || allow.contains(&format!("{}!*", imp.dll)) {
            continue;
        }
        if shipped.contains(&imp.dll) {
            continue;
        }
        if let Some(problem) = problem(imp, exports) {
            let line = format!("{key}: {problem}");
            if imp.delay {
                warnings.insert(format!("{line} (delay-load)"));
            } else {
                errors.insert(line);
            }
        }
    }
    for e in &errors {
        println!("  ERRO: {e}");
    }
    for w in &warnings {
        println!("  aviso: {w}");
    }
    ok &= errors.is_empty();
    println!(
        "  {} imports verificados, {} erro(s), {} aviso(s)",
        info.imports.len(),
        errors.len(),
        warnings.len()
    );
    Ok(ok)
}

fn problem(imp: &Imported, exports: Option<&Exports>) -> Option<&'static str> {
    if DENY_DLLS.contains(&imp.dll.as_str()) {
        return Some("DLL inexistente no Windows 7");
    }
    if let Some(f) = &imp.func {
        if DENY_FUNCS.iter().any(|(d, n)| *d == imp.dll && n == f) {
            return Some("função introduzida após o Windows 7");
        }
    }
    let exports = exports?;
    let Some(dll_exports) = exports.get(&imp.dll) else {
        return Some("DLL ausente no dump do Windows 7");
    };
    match &imp.func {
        Some(f) if !dll_exports.contains(f) => Some("função ausente no dump do Windows 7"),
        _ => None,
    }
}

fn read_pe(path: &Path) -> Result<PeInfo, String> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let kind = FileKind::parse(&*data).map_err(|e| format!("{}: {e}", path.display()))?;
    let r = match kind {
        FileKind::Pe32 => read_pe_generic::<ImageNtHeaders32>(&data, "x86"),
        FileKind::Pe64 => read_pe_generic::<ImageNtHeaders64>(&data, "x64"),
        other => return Err(format!("{}: não é PE ({other:?})", path.display())),
    };
    r.map_err(|e| format!("{}: {e}", path.display()))
}

fn read_pe_generic<Pe: ImageNtHeaders>(
    data: &[u8],
    machine: &'static str,
) -> object::Result<PeInfo> {
    let file = PeFile::<Pe>::parse(data)?;
    let opt = file.nt_headers().optional_header();
    let mut imports = Vec::new();

    if let Some(table) = file.import_table()? {
        let mut descs = table.descriptors()?;
        while let Some(d) = descs.next()? {
            let dll = String::from_utf8_lossy(table.name(d.name.get(LE))?).to_ascii_lowercase();
            let mut thunk_rva = d.original_first_thunk.get(LE);
            if thunk_rva == 0 {
                thunk_rva = d.first_thunk.get(LE);
            }
            let mut thunks = table.thunks(thunk_rva)?;
            while let Some(t) = thunks.next::<Pe>()? {
                imports.push(Imported {
                    dll: dll.clone(),
                    func: name_of(table.import::<Pe>(t)?),
                    delay: false,
                });
            }
        }
    }

    if let Some(table) = file.delay_load_import_table()? {
        let mut descs = table.descriptors()?;
        while let Some(d) = descs.next()? {
            let dll =
                String::from_utf8_lossy(table.name(d.dll_name_rva.get(LE))?).to_ascii_lowercase();
            let mut thunks = table.thunks(d.import_name_table_rva.get(LE))?;
            while let Some(t) = thunks.next::<Pe>()? {
                imports.push(Imported {
                    dll: dll.clone(),
                    func: name_of(table.import::<Pe>(t)?),
                    delay: true,
                });
            }
        }
    }

    Ok(PeInfo {
        machine,
        os_version: (
            opt.major_operating_system_version(),
            opt.minor_operating_system_version(),
        ),
        subsystem_version: (opt.major_subsystem_version(), opt.minor_subsystem_version()),
        imports,
    })
}

fn name_of(imp: Import<'_>) -> Option<String> {
    match imp {
        Import::Name(_, n) => Some(String::from_utf8_lossy(n).into_owned()),
        Import::Ordinal(_) => None,
    }
}

fn load_exports(path: &Path) -> Result<Exports, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str::<BTreeMap<String, BTreeSet<String>>>(&text)
        .map(|m| m.into_iter().map(|(k, v)| (k.to_ascii_lowercase(), v)).collect())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Uma entrada `dll!Funcao` (ou `dll!*`) por linha; `#` comenta.
fn load_allow(path: &Path) -> Result<BTreeSet<String>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(|l| match l.split_once('!') {
            Some((d, f)) => format!("{}!{}", d.to_ascii_lowercase(), f),
            None => l.to_ascii_lowercase(),
        })
        .collect())
}

fn dump_cmd(dir: &Path, out: &Path) -> Result<bool, String> {
    let mut all = Exports::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dll = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dll"));
        if !is_dll {
            continue;
        }
        let Ok(data) = fs::read(&path) else { continue };
        let names = match FileKind::parse(&*data) {
            Ok(FileKind::Pe32) => export_names::<ImageNtHeaders32>(&data),
            Ok(FileKind::Pe64) => export_names::<ImageNtHeaders64>(&data),
            _ => continue,
        };
        let Ok(names) = names else { continue };
        let dll = entry.file_name().to_string_lossy().to_ascii_lowercase();
        all.insert(dll, names);
    }
    let json = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    fs::write(out, json).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("{} DLLs gravadas em {}", all.len(), out.display());
    Ok(true)
}

fn export_names<Pe: ImageNtHeaders>(data: &[u8]) -> object::Result<BTreeSet<String>> {
    let file = PeFile::<Pe>::parse(data)?;
    let mut names = BTreeSet::new();
    if let Some(table) = file.export_table()? {
        for (name_rva, _) in table.name_iter() {
            names.insert(String::from_utf8_lossy(table.name_from_pointer(name_rva)?).into_owned());
        }
    }
    Ok(names)
}
