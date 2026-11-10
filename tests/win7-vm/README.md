# E2E em Windows 7 (QEMU/KVM)

O GitHub Actions não tem runner Windows 7, então a garantia vem em duas camadas:

| Camada | Onde roda | O que prova |
|---|---|---|
| `win7-check` | CI, em todo build | O `.exe` só importa DLLs/funções que existem no Win7 SP1 e o cabeçalho PE pede NT ≤ 6.1. Pega regressões de dependência na hora. |
| `run.sh` (este diretório) | Sua máquina, VM Win7 | O app instala, abre o WebView2 109, executa JS e **se atualiza sozinho** de 0.0.1 para 0.0.2. |

## 1. Preparar a imagem base (uma vez por arquitetura)

Você precisa de uma ISO do Windows 7 SP1 (x64 e/ou x86) e da licença.

```bash
mkdir -p tests/win7-vm/work
qemu-img create -f qcow2 tests/win7-vm/work/win7-x64-base.qcow2 40G
qemu-system-x86_64 -enable-kvm -cpu host -smp 2 -m 3072 \
  -drive file=tests/win7-vm/work/win7-x64-base.qcow2,if=ide \
  -cdrom /caminho/Win7SP1_x64.iso -boot d \
  -nic user,model=e1000 \
  -device virtio-serial -vnc :7
```

Use `qemu-system-i386` e `win7-x86-base.qcow2` para a VM 32-bit. Dentro da VM:

1. Instale o Windows e crie o usuário **`tester`**.
2. Instale as atualizações (pacotes `.msu` do Microsoft Update Catalog):
   - **KB4474419** e **KB4490628**: suporte a assinatura SHA-2. Sem eles, o WebView2 e instaladores assinados recentes não rodam.
   - **KB2533623**: `SetDefaultDllDirectories`, que alguns runtimes exigem.
   - Opcional: KB3140245 (TLS 1.2 no SChannel). O app não usa SChannel, mas outras ferramentas usam.
3. Instale o **QEMU Guest Agent** de uma ISO virtio-win antiga com suporte a Win7 (a linha 0.1.17x; as mais novas abandonaram o Win7). Instale também o driver *VirtIO Serial*. Confira se o serviço "QEMU Guest Agent" está em início automático.
4. Configure o **autologon** do `tester`: `control userpasswords2` e desmarque "Usuários devem digitar…".
5. Desligue o Windows Update automático e a proteção de tela.
6. Desligue a VM. A base fica pronta e **nunca é alterada**: cada execução cria um overlay descartável.

## 2. Gerar os dumps de exports do Win7 (uma vez)

Isso deixa o `win7-check` estrito. Sem os dumps, ele só aplica a lista embutida de APIs proibidas.

1. Baixe o artefato `win7-check-exe` de um run do CI (é o próprio checker compilado para Win7).
2. Copie `win7-check.exe` e `guest/dump-exports.cmd` para a VM **x64** e rode o `.cmd`.
3. Traga `win7sp1-exports-x64.json` e `win7sp1-exports-x86.json` (o x86 vem do SysWOW64) para `tools/win7-check/data/` e commite.

## 3. Rodar o E2E

```bash
gh run download <run-id> -n e2e-x64 -D tests/win7-vm/work/e2e-x64
tests/win7-vm/run.sh x64 tests/win7-vm/work/e2e-x64
```

O script:

1. Sobe um `http.server` no host com um `latest.json` apontando para o instalador 0.0.2. A VM enxerga o host em `10.0.2.2`.
2. Boota a VM a partir de um overlay da base.
3. Via guest agent e `schtasks /it`, roda os passos **na sessão interativa** do usuário:
   - instala a versão 0.0.1 em modo silencioso;
   - roda `rocket-rust.exe --self-test`, que precisa reportar app 0.0.1, WebView2 109.x e Windows 6.1;
   - abre o app normalmente e espera ele baixar e instalar o 0.0.2;
   - roda o self-test de novo e exige a versão 0.0.2.

Para acompanhar na tela, conecte um cliente VNC em `localhost:5907`.

## Checklist manual (antes de uma release importante)

- [ ] UI renderiza corretamente com DPI em 100% e 125%
- [ ] Ícone e nome no menu Iniciar
- [ ] Update via endpoint real do GitHub, com a VM tendo acesso à internet
- [ ] Desinstalação pelo Painel de Controle remove o app
