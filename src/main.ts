import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type UpdateEvent =
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string }
  | { kind: "downloading"; downloaded: number; total: number | null }
  | { kind: "installing" }
  | { kind: "error"; message: string };

interface AppInfo {
  appVersion: string;
  webviewVersion: string;
  os: string;
  selfTest: boolean;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function renderUpdate(ev: UpdateEvent) {
  const status = $<HTMLParagraphElement>("update-status");
  const progress = $<HTMLProgressElement>("update-progress");
  const button = $<HTMLButtonElement>("check-now");

  progress.hidden = ev.kind !== "downloading";
  button.disabled = ev.kind === "checking" || ev.kind === "downloading" || ev.kind === "installing";

  switch (ev.kind) {
    case "checking":
      status.textContent = "Verificando atualizações…";
      break;
    case "upToDate":
      status.textContent = "Você está na versão mais recente.";
      break;
    case "available":
      status.textContent = `Nova versão ${ev.version} encontrada.`;
      break;
    case "downloading":
      if (ev.total) {
        progress.value = Math.round((ev.downloaded / ev.total) * 100);
        status.textContent = `Baixando atualização… ${progress.value}%`;
      } else {
        progress.removeAttribute("value");
        status.textContent = "Baixando atualização…";
      }
      break;
    case "installing":
      status.textContent = "Instalando atualização, o aplicativo vai reiniciar…";
      break;
    case "error":
      status.textContent = `Falha ao atualizar: ${ev.message}`;
      break;
  }
}

async function main() {
  await listen<UpdateEvent>("update://status", (e) => renderUpdate(e.payload));

  const info = await invoke<AppInfo>("app_info");
  $("app-version").textContent = info.appVersion;
  $("webview-version").textContent = info.webviewVersion;
  $("os-info").textContent = info.os;

  if (info.selfTest) {
    // Prova que o WebView carregou e executou JS de verdade.
    await invoke("selftest_report", { userAgent: navigator.userAgent });
    return;
  }

  $<HTMLButtonElement>("check-now").addEventListener("click", () => {
    invoke("check_for_updates").catch((err) =>
      renderUpdate({ kind: "error", message: String(err) }),
    );
  });
}

main().catch((err) => console.error(err));
