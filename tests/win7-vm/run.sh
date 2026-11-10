#!/usr/bin/env bash
# E2E em VM Windows 7 (QEMU/KVM): instala a versão antiga, roda o self-test,
# abre o app, espera o auto-update para a versão nova e roda o self-test de novo.
#
# uso: tests/win7-vm/run.sh <x64|x86> <dir-e2e>
#   <dir-e2e> contém old/ e new/ com *-setup.exe e *.sig (artefato `e2e-<arch>` do CI:
#   gh run download <run-id> -n e2e-x64 -D tests/win7-vm/work/e2e-x64)
#
# Variáveis:
#   WIN7_BASE_<ARCH>  imagem base preparada (padrão tests/win7-vm/work/win7-<arch>-base.qcow2)
#   WIN7_USER         usuário com autologon na VM (padrão: tester)
#   VNC_DISPLAY       display VNC para acompanhar (padrão :7 → porta 5907)
#   UPDATE_WAIT       segundos esperando o update (padrão 240)
set -euo pipefail

ARCH=${1:?arch: x64 ou x86}
E2E_DIR=$(realpath "${2:?diretório com old/ e new/}")
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
WORK="$HERE/work/run-$ARCH"
BASE_VAR="WIN7_BASE_${ARCH^^}"
BASE=${!BASE_VAR:-$HERE/work/win7-$ARCH-base.qcow2}
WIN7_USER=${WIN7_USER:-tester}
VNC_DISPLAY=${VNC_DISPLAY:-:7}
UPDATE_WAIT=${UPDATE_WAIT:-240}
PORT=8765
QGA_SOCK="$WORK/qga.sock"
qga() { python3 -I "$HERE/qga.py" "$QGA_SOCK" "$@"; }

[ -f "$BASE" ] || { echo "imagem base não encontrada: $BASE (veja tests/win7-vm/README.md)"; exit 2; }
OLD_SETUP=$(ls "$E2E_DIR"/old/*-setup.exe | head -1)
NEW_SETUP=$(ls "$E2E_DIR"/new/*-setup.exe | head -1)

rm -rf "$WORK" && mkdir -p "$WORK/www" "$WORK/out"

# --- servidor de update no host (a VM enxerga o host em 10.0.2.2) ---
node "$ROOT/scripts/make-latest-json.mjs" --version 0.0.2 \
  --base-url "http://10.0.2.2:$PORT" --out "$WORK/www" --"$ARCH" "$E2E_DIR/new" >/dev/null
python3 -I -m http.server "$PORT" --bind 127.0.0.1 --directory "$WORK/www" >"$WORK/http.log" 2>&1 &
HTTP_PID=$!

# --- VM descartável: overlay sobre a base, a base nunca muda ---
qemu-img create -q -f qcow2 -F qcow2 -b "$BASE" "$WORK/disk.qcow2"
QEMU=qemu-system-x86_64
[ "$ARCH" = x86 ] && QEMU=qemu-system-i386
$QEMU -enable-kvm -cpu host -smp 2 -m 3072 \
  -drive file="$WORK/disk.qcow2",if=ide \
  -nic user,model=e1000 \
  -device virtio-serial \
  -chardev socket,path="$QGA_SOCK",server=on,wait=off,id=qga0 \
  -device virtserialport,chardev=qga0,name=org.qemu.guest_agent.0 \
  -display none -vnc "$VNC_DISPLAY" \
  -pidfile "$WORK/qemu.pid" -daemonize

cleanup() {
  kill "$HTTP_PID" 2>/dev/null || true
  [ -f "$WORK/qemu.pid" ] && kill "$(cat "$WORK/qemu.pid")" 2>/dev/null || true
}
trap cleanup EXIT

echo ">> aguardando o guest agent (boot do Win7)…"
for _ in $(seq 1 180); do qga ping 2>/dev/null && break; sleep 2; done
qga ping
sleep 20 # autologon + desktop

qga exec 30 cmd.exe /c "if not exist C:\\e2e mkdir C:\\e2e" >/dev/null
qga push "$HERE/guest/e2e.cmd" 'C:\e2e\e2e.cmd'
qga push "$OLD_SETUP" 'C:\e2e\old-setup.exe'

# Roda um passo na sessão interativa do usuário e espera o done-<passo>.txt.
step() {
  local name=$1 timeout=$2; shift 2
  qga exec 30 cmd.exe /c "del /q C:\\e2e\\out\\done-$name.txt 2>nul" >/dev/null || true
  qga exec 30 schtasks.exe /create /f /tn "e2e-$name" /sc once /st 23:59 /ru "$WIN7_USER" /it \
    /tr "C:\\e2e\\e2e.cmd $name $*" >/dev/null
  qga exec 30 schtasks.exe /run /tn "e2e-$name" >/dev/null
  for _ in $(seq 1 "$timeout"); do
    if qga pull "C:\\e2e\\out\\done-$name.txt" "$WORK/out/done-$name.txt" 2>/dev/null; then
      local code; code=$(tr -d '\r\n ' <"$WORK/out/done-$name.txt")
      echo "   passo $name → código $code"
      [ "$code" = 0 ]
      return
    fi
    sleep 1
  done
  echo "   passo $name → timeout"; return 1
}

check_selftest() {
  local tag=$1 want=$2 f="$WORK/out/selftest-$1.json"
  qga pull "C:\\e2e\\out\\selftest-$tag.json" "$f"
  cat "$f"; echo
  python3 -I - "$f" "$want" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))
want = sys.argv[2]
assert r["ok"], f"self-test falhou: {r.get('error')}"
assert r["appVersion"] == want, f"versão {r['appVersion']} != {want}"
assert (r.get("webviewVersion") or "").startswith("109."), f"WebView2 inesperado: {r.get('webviewVersion')}"
assert "Windows 6.1" in r["os"], f"não é Windows 7: {r['os']}"
PY
}

echo ">> 1/4 instalando versão antiga"
step install 300 'C:\e2e\old-setup.exe'

echo ">> 2/4 self-test da versão antiga"
step selftest 90 old
check_selftest old 0.0.1

echo ">> 3/4 abrindo o app e esperando o auto-update (até ${UPDATE_WAIT}s)"
step launch 30
updated=0
for _ in $(seq 1 $((UPDATE_WAIT / 10))); do
  sleep 10
  if grep -q "new-setup\|RocketRust_0.0.2" "$WORK/http.log" 2>/dev/null; then updated=1; fi
  # O instalador passivo encerra o app e reinstala; quando terminar, o self-test responde 0.0.2.
  if [ $updated = 1 ]; then
    sleep 45
    break
  fi
done
[ $updated = 1 ] || { echo "app nunca baixou o update. log http:"; cat "$WORK/http.log"; exit 1; }
step kill 30

echo ">> 4/4 self-test após update"
step selftest 90 new
check_selftest new 0.0.2

echo
echo "E2E Win7 $ARCH: OK (instalação, execução e auto-update 0.0.1 → 0.0.2)"
