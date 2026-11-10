#!/usr/bin/env node
// uso: pnpm release 1.2.3
// Atualiza a versão em package.json, tauri.conf.json e Cargo.toml, commita e cria a tag.
// O push da tag dispara .github/workflows/release.yml.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("uso: pnpm release X.Y.Z");
  process.exit(2);
}

const json = (path, fn) => {
  const data = JSON.parse(readFileSync(path, "utf8"));
  fn(data);
  writeFileSync(path, JSON.stringify(data, null, 2) + "\n");
};
json("package.json", (d) => (d.version = version));
json("src-tauri/tauri.conf.json", (d) => (d.version = version));

const cargo = "src-tauri/Cargo.toml";
writeFileSync(
  cargo,
  readFileSync(cargo, "utf8").replace(/^version = ".*"$/m, `version = "${version}"`),
);

const git = (...a) => execFileSync("git", a, { stdio: "inherit" });
execFileSync("cargo", ["update", "-p", "rocket-rust", "--manifest-path", cargo], { stdio: "inherit" });
git("add", "package.json", "src-tauri/tauri.conf.json", cargo, "src-tauri/Cargo.lock");
git("commit", "-m", `release: v${version}`);
git("tag", `v${version}`);
console.log(`\nPronto. Publique com:\n  git push && git push origin v${version}`);
