$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$userKey = Join-Path $env:USERPROFILE ".tauri\neeko-assistant.key"
$userPassword = Join-Path $env:USERPROFILE ".tauri\neeko-assistant.key.password"
$repoKey = Join-Path $repoRoot "capita.key"

if (Test-Path $userKey) {
    $signingKey = Get-Content $userKey -Raw
}
elseif (Test-Path $repoKey) {
    $signingKey = Get-Content $repoKey -Raw
}
else {
    throw "No encontre una signing key. Esperaba $userKey o $repoKey"
}

$env:TAURI_SIGNING_PRIVATE_KEY = $signingKey

if (Test-Path $userPassword) {
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content $userPassword -Raw
}
elseif (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    if ($signingKey -match "encrypted secret key") {
        throw "La signing key esta cifrada y no encontre password en $userPassword ni en TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
    }
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
}

npx tauri build
