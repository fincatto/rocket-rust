<#
.SYNOPSIS
  Baixa e valida o WebView2 Fixed Version Runtime 109.0.1518.78 (o último com suporte
  a Windows 7) e o coloca em src-tauri/webview2/, onde o tauri.win7-<arch>.conf.json espera.

.DESCRIPTION
  A Microsoft não oferece mais o 109 para download direto. Fonte padrão: o pacote NuGet
  WebView2.Runtime.<Arch> 109.0.1518.78, um reempacotamento de terceiros do runtime
  oficial. Como a fonte não é da Microsoft, o script:
    1. confere o SHA256 do pacote contra o valor fixado abaixo;
    2. exige assinatura Authenticode VÁLIDA da Microsoft em TODOS os .exe/.dll;
    3. confere a versão do msedgewebview2.exe.
  Para usar outra fonte (ex.: um asset de release do próprio repositório), defina
  WEBVIEW2_109_URL_X64 / WEBVIEW2_109_URL_X86 e WEBVIEW2_109_SHA256_X64 / _X86.
#>
param(
  [Parameter(Mandatory = $true)][ValidateSet('x64', 'x86')][string]$Arch
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Version = '109.0.1518.78'
$Sources = @{
  x64 = @{
    Url    = "https://api.nuget.org/v3-flatcontainer/webview2.runtime.x64/$Version/webview2.runtime.x64.$Version.nupkg"
    Sha256 = '7016c03ed2ec423fe68c187f4068ccdfafd49a52e59e7c55792142e8d8334906'
  }
  x86 = @{
    Url    = "https://api.nuget.org/v3-flatcontainer/webview2.runtime.x86/$Version/webview2.runtime.x86.$Version.nupkg"
    Sha256 = '6686674da8f48b4ee6e90df85fa68a1c717311cd29d88e950ff7f0fdbd12e05b'
  }
}

$ArchUpper = $Arch.ToUpper()
$Url = [Environment]::GetEnvironmentVariable("WEBVIEW2_109_URL_$ArchUpper")
$Sha = [Environment]::GetEnvironmentVariable("WEBVIEW2_109_SHA256_$ArchUpper")
if (-not $Url) { $Url = $Sources[$Arch].Url; $Sha = $Sources[$Arch].Sha256 }
if (-not $Sha) { throw "Defina WEBVIEW2_109_SHA256_$ArchUpper junto com WEBVIEW2_109_URL_$ArchUpper" }

$Root = Split-Path -Parent $PSScriptRoot
$Dest = Join-Path $Root "src-tauri/webview2/Microsoft.WebView2.FixedVersionRuntime.$Version.$Arch"
$Marker = Join-Path $Dest '.verified'

function Assert-Runtime([string]$Dir) {
  $exe = Join-Path $Dir 'msedgewebview2.exe'
  if (-not (Test-Path $exe)) { throw "msedgewebview2.exe não encontrado em $Dir" }

  $fileVersion = (Get-Item $exe).VersionInfo.FileVersion
  if ($fileVersion -ne $Version) { throw "versão inesperada: $fileVersion (esperado $Version)" }

  # Exceções explícitas: componentes de terceiros que a própria Microsoft distribui no Edge.
  $OtherSigners = @{
    'widevinecdm.dll' = 'O=Google LLC'  # módulo DRM Widevine, assinado pelo Google
  }

  $bad = @()
  $files = Get-ChildItem $Dir -Recurse -File -Include *.exe, *.dll
  foreach ($f in $files) {
    $sig = Get-AuthenticodeSignature $f.FullName
    $subject = if ($sig.SignerCertificate) { $sig.SignerCertificate.Subject } else { '' }
    $expected = $OtherSigners[$f.Name.ToLower()]
    if (-not $expected) { $expected = 'O=Microsoft Corporation' }
    if ($sig.Status -ne 'Valid' -or $subject -notmatch [regex]::Escape($expected)) {
      $bad += "$($f.Name): $($sig.Status) [$subject]"
    }
  }
  if ($bad.Count -gt 0) {
    throw "binários sem assinatura válida da Microsoft:`n  " + ($bad -join "`n  ")
  }
  Write-Host "OK: $($files.Count) binários com assinatura válida (Microsoft; Widevine: Google), versão $fileVersion"
}

if (Test-Path $Marker) {
  # Cache do CI: revalida as assinaturas, que é rápido.
  Assert-Runtime $Dest
  return
}

$Tmp = Join-Path ([IO.Path]::GetTempPath()) "webview2-$Version-$Arch"
Remove-Item $Tmp -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $Tmp | Out-Null
$Pkg = Join-Path $Tmp 'pkg.zip'

Write-Host "Baixando $Url"
Invoke-WebRequest -Uri $Url -OutFile $Pkg -UseBasicParsing

$actual = (Get-FileHash $Pkg -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $Sha.ToLower()) { throw "SHA256 divergente: $actual (esperado $Sha)" }

$Extract = Join-Path $Tmp 'x'
if ($Url -match '\.cab($|\?)') {
  New-Item -ItemType Directory $Extract | Out-Null
  & expand.exe -F:* $Pkg $Extract | Out-Null
} else {
  Expand-Archive -Path $Pkg -DestinationPath $Extract
}

# Acha o diretório que contém msedgewebview2.exe, seja qual for o layout do pacote.
$runtimeDir = (Get-ChildItem $Extract -Recurse -Filter msedgewebview2.exe | Select-Object -First 1).DirectoryName
if (-not $runtimeDir) { throw 'msedgewebview2.exe não encontrado no pacote' }

Assert-Runtime $runtimeDir

Remove-Item $Dest -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory (Split-Path $Dest) -Force | Out-Null
Move-Item $runtimeDir $Dest
Set-Content $Marker "sha256=$actual"
Remove-Item $Tmp -Recurse -Force
Write-Host "WebView2 $Version ($Arch) pronto em $Dest"
