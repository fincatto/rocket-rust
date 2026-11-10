# Patches de dependências para Windows 7

## windows-core 0.62.2

O `windows-core` liga `CoTaskMemAlloc`, `CoTaskMemFree` e
`CoCreateFreeThreadedMarshaler` à `combase.dll`, que **não existe no Windows 7**.
As três funções existem com o mesmo nome na `ole32.dll` desde o Windows 2000,
e a partir do Windows 8 a `ole32.dll` só encaminha para a `combase.dll`.
O comportamento é o mesmo.

Únicas mudanças em relação ao crate publicado:
- `src/imp/bindings.rs`: `CoTaskMemAlloc`/`CoTaskMemFree` → `ole32.dll`
- `src/imp/marshaler.rs`: `CoCreateFreeThreadedMarshaler` → `ole32.dll`

`CoIncrementMTAUsage`/`RoGetAgileReference` ficaram na `combase.dll`. Elas são
Win8+ em qualquer DLL e não são usadas pelo app. Se passarem a ser, o
`win7-check` acusa.

### Ao atualizar o Tauri

Se o `windows-core` mudar de versão, o cargo avisa que o patch não foi usado
("Patch `windows-core` was not used"). Nesse caso, copie a nova versão do
registry e reaplique as mesmas trocas:

```bash
cp -r ~/.cargo/registry/src/*/windows-core-<versão> src-tauri/patches/windows-core
```
