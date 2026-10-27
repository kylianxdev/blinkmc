# Blink installer for Windows: your Minecraft server sleeps when nobody plays and wakes up in a blink.
#
# In PowerShell, in the folder with your paper.jar:
#   irm https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.ps1 | iex
#
# Puts blink.exe and blink.toml in the server folder and writes start-blink.bat. Changes nothing
# else on your PC and needs no administrator rights. Start your server with start-blink.bat from now on.
$ErrorActionPreference = "Stop"
$repo = if ($env:BLINK_REPO) { $env:BLINK_REPO } else { "kylianxdev/blinkmc" }
$asset = "blink-windows-x64.exe"
$dir = (Get-Location).Path

Write-Host "==> Putting blink.exe in $dir" -ForegroundColor Green
$url = "https://github.com/$repo/releases/latest/download/$asset"
Invoke-WebRequest "$url" -OutFile "$dir\blink.exe" -UseBasicParsing
$expected = ((Invoke-WebRequest "$url.sha256" -UseBasicParsing).Content -split '\s+')[0]
$actual = (Get-FileHash "$dir\blink.exe" -Algorithm SHA256).Hash.ToLower()
if ($expected -ne $actual) { Remove-Item "$dir\blink.exe"; throw "Checksum of blink.exe does not match" }
& "$dir\blink.exe" --version

if (-not (Get-Command java -ErrorAction SilentlyContinue) -and -not $env:JAVA_HOME) {
    Write-Host "  ! Java 26 was not found. Install it, for example: winget install EclipseAdoptium.Temurin.26.JDK" -ForegroundColor Yellow
}

Write-Host "==> Writing blink.toml" -ForegroundColor Green
& "$dir\blink.exe" init --force
if ($LASTEXITCODE -ne 0) { throw "blink init failed" }

Set-Content -Path "$dir\start-blink.bat" -Value "@echo off`r`ncd /d `"%~dp0`"`r`nblink.exe start`r`npause" -Encoding ASCII

Write-Host ""
Write-Host "Done. Start your server with start-blink.bat from now on." -ForegroundColor Green
Write-Host "  Players join on the same address as always. 30 seconds after the last player leaves, the"
Write-Host "  server saves and freezes, and Windows compresses its memory. The next join wakes it up."
Write-Host "  Stop it with Ctrl+C in that window: the server saves and stops, also while it is frozen."
