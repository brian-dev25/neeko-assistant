# Downloads the official CPU runtime only; does not install drivers or alter Windows.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$version = '1.24.3'
$expected = '4fbfb85d0e9de9bb6fb8a9866a7cb477cbad404d889b236931bf3f5d547e5f48'
$archive = Join-Path ([IO.Path]::GetTempPath()) 'neeko-onnxruntime-1.24.3.zip'
$extract = Join-Path ([IO.Path]::GetTempPath()) ('neeko-onnxruntime-' + [guid]::NewGuid().ToString())
$resources = Join-Path $PSScriptRoot 'resources'
Invoke-WebRequest -UseBasicParsing -Uri "https://github.com/microsoft/onnxruntime/releases/download/v$version/onnxruntime-win-x64-$version.zip" -OutFile $archive
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
    throw 'ONNX Runtime checksum mismatch'
}
Expand-Archive -LiteralPath $archive -DestinationPath $extract
$package = Join-Path $extract "onnxruntime-win-x64-$version"
New-Item -ItemType Directory -Force -Path $resources | Out-Null
Copy-Item -LiteralPath (Join-Path $package 'lib/onnxruntime.dll') -Destination $resources
Copy-Item -LiteralPath (Join-Path $package 'lib/onnxruntime_providers_shared.dll') -Destination $resources
Copy-Item -LiteralPath (Join-Path $package 'LICENSE') -Destination (Join-Path $resources 'LICENSE-ONNXRuntime.txt')
Copy-Item -LiteralPath (Join-Path $package 'ThirdPartyNotices.txt') -Destination (Join-Path $resources 'ThirdPartyNotices-ONNXRuntime.txt')
Write-Output "AEC runtime preparado en $resources"
