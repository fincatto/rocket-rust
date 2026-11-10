# Rocket Rust

App desktop em **Tauri 2 + Vite/TypeScript** que roda no **Windows 7 SP1 (x64 e x86)** e **se atualiza sozinho** a cada release publicada no GitHub.

## Por que dá trabalho rodar no Windows 7

| Problema | Solução aqui |
|---|---|
| O Rust ≥ 1.78 não suporta mais Win7 nos targets `*-pc-windows-msvc` (o std importa `ProcessPrng`, `WaitOnAddress`…) | Targets oficiais **`x86_64-win7-windows-msvc` / `i686-win7-windows-msvc`** (tier 3), com nightly fixado em [`rust-toolchain.toml`](rust-toolchain.toml) e `-Zbuild-std` |
| O WebView2 parou na versão **109** no Win7 | O instalador embarca o **Fixed Version Runtime 109.0.1518.78** ([`tauri.win7-x64.conf.json`](src-tauri/tauri.win7-x64.conf.json)), baixado e validado por [`fetch-webview2-109.ps1`](scripts/fetch-webview2-109.ps1) (SHA256 fixado + Authenticode da Microsoft em todos os binários) |
| O JS/CSS precisa rodar no Chromium 109 | `build.target = "chrome109"` no [`vite.config.ts`](vite.config.ts) |
| O repositório de certificados do Win7 está desatualizado | O updater usa rustls com **raízes da Mozilla embutidas** ([`updater.rs`](src-tauri/src/updater.rs)) |
| Sem VC++ Redistributable moderno | CRT estático (`+crt-static` em [`.cargo/config.toml`](.cargo/config.toml)) |
| O GitHub Actions não tem Windows 7 | Verificação estática no CI ([`tools/win7-check`](tools/win7-check)) + E2E em VM QEMU ([`tests/win7-vm`](tests/win7-vm/README.md)) |

## Desenvolvimento

```bash
pnpm install
pnpm tauri dev
```

No Linux, o `tauri dev` precisa de `libwebkit2gtk-4.1-dev`. Ele valida UI e lógica, mas **não** o Win7.

## Auto-update

- No startup (após 5 s) e a cada 6 h, o app consulta `https://github.com/fincatto/rocket-rust/releases/latest/download/latest.json`.
- Havendo versão nova, baixa, confere a assinatura minisign e roda o instalador NSIS em modo `passive`. A instalação é `currentUser`, então não pede UAC. Depois o app reinicia.
- A UI mostra o status e tem o botão "Verificar agora".

### Configuração única

1. A chave de assinatura dos updates já foi gerada em `~/.tauri/rocket-rust.key`. A chave pública está em `tauri.conf.json`. **Guarde a privada:** se ela se perder, os apps instalados não aceitam mais updates.
2. Cadastre-a nos secrets do repositório:
   ```bash
   gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/rocket-rust.key
   ```
   A chave não tem senha, então `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` não precisa existir (o workflow recebe string vazia). Se gerar uma chave com senha, cadastre esse secret também.

### Publicar uma versão

```bash
pnpm release 0.2.0
```
```bash
git push && git push origin v0.2.0
```

O workflow [`release.yml`](.github/workflows/release.yml) compila x64 e x86 para Win7, roda o `win7-check` e só então publica os instaladores, os `.sig` e o `latest.json` no GitHub Release.

## Como a compatibilidade com Win7 é garantida

1. **`win7-check` em todo build do CI**: lê imports e delay-imports do PE e reprova qualquer DLL ou função ausente no Win7 SP1, além de cabeçalho exigindo NT > 6.1.
   - Sempre aplica uma lista embutida de APIs pós-Win7 conhecidas.
   - Com os dumps reais de exports do Win7 em `tools/win7-check/data/`, passa a checar **cada** import. Para gerar os dumps, veja o [README da VM](tests/win7-vm/README.md).
2. **Autoteste do checker**: o CI compila um hello-world com o target padrão (Win10+) e exige que o `win7-check` o **reprove**.
3. **E2E na VM Win7** ([`tests/win7-vm/run.sh`](tests/win7-vm/run.sh)): instala a 0.0.1, roda `rocket-rust.exe --self-test` (exige WebView2 109 e Windows 6.1), abre o app e confirma que ele se atualizou sozinho para a 0.0.2. O CI gera esse par de instaladores como artefato `e2e-<arch>`.

## Estrutura

```
src/                 frontend (TS)
src-tauri/           app Rust (updater.rs, selftest.rs, osinfo.rs)
tools/win7-check/    verificador de imports PE x Windows 7
scripts/             fetch do WebView2 109, latest.json, bump de versão
tests/win7-vm/       E2E em QEMU/KVM
.github/             CI, release e action composta de build Win7
```
