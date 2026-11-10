#!/usr/bin/env node
// Monta latest.json (formato do tauri-plugin-updater) a partir dos artefatos
// de build e copia os instaladores com nomes estáveis para o diretório de saída.
//
// uso: node scripts/make-latest-json.mjs --version 1.2.3 --base-url <url> \
//        --out <dir> --x64 <dir-bundle-nsis> --x86 <dir-bundle-nsis> [--notes "..."]
import { copyFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const args = {};
for (let i = 2; i < process.argv.length; i += 2) {
  args[process.argv[i].replace(/^--/, "")] = process.argv[i + 1];
}
for (const k of ["version", "base-url", "out"]) {
  if (!args[k]) {
    console.error(`faltando --${k}`);
    process.exit(2);
  }
}

const version = args.version.replace(/^v/, "");
const baseUrl = args["base-url"].replace(/\/$/, "");
const out = args.out;
mkdirSync(out, { recursive: true });

// arch do Tauri -> chave de plataforma do updater
const ARCHES = { x64: "x86_64", x86: "i686" };
const platforms = {};

for (const [arch, platformArch] of Object.entries(ARCHES)) {
  const dir = args[arch];
  if (!dir) continue;
  const setup = readdirSync(dir).find((f) => f.endsWith("-setup.exe"));
  if (!setup) throw new Error(`nenhum *-setup.exe em ${dir}`);
  const sig = readFileSync(join(dir, `${setup}.sig`), "utf8").trim();

  // Sem espaços: o GitHub troca espaços por pontos nos nomes dos assets.
  const name = `RocketRust_${version}_${arch}-setup.exe`;
  copyFileSync(join(dir, setup), join(out, name));
  writeFileSync(join(out, `${name}.sig`), sig);

  const entry = { signature: sig, url: `${baseUrl}/${name}` };
  platforms[`windows-${platformArch}`] = entry;
  platforms[`windows-${platformArch}-nsis`] = entry;
}

if (Object.keys(platforms).length === 0) throw new Error("passe --x64 e/ou --x86");

const latest = {
  version,
  notes: args.notes ?? `Versão ${version}`,
  pub_date: new Date().toISOString(),
  platforms,
};
writeFileSync(join(out, "latest.json"), JSON.stringify(latest, null, 2) + "\n");
console.log(`latest.json gerado em ${out} para: ${Object.keys(platforms).join(", ")}`);
