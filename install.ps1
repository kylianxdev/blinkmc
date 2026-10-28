# Blink installer for Windows: your Minecraft server sleeps when nobody plays and wakes up when
# someone joins.
#
# In PowerShell, in your server folder (paper.jar, a Fabric or vanilla jar, or Forge's run.bat):
#   irm https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.ps1 | iex
# Undo it (puts the server's own port and settings back, removes Blink):
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.ps1))) -Uninstall
#
# Puts blink.exe, blink.toml and start-blink.bat in the server folder and needs no administrator
# rights. Nothing outside the folder changes. When Blink starts the server it moves it to another
# port in server.properties and raises Paper's watchdog; blink.exe restore (or -Uninstall) undoes that.
# Running it again updates blink.exe and keeps your blink.toml.
param([switch]$Uninstall)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"   # Invoke-WebRequest is much faster without its progress bar
$repo = if ($env:BLINK_REPO) { $env:BLINK_REPO } else { "kylianxdev/blinkmc" }
$asset = "blink-windows-x64.exe"
$dir = (Get-Location).Path
$blink = Join-Path $dir "blink.exe"

function Step($text) { Write-Host "==> $text" -ForegroundColor Green }
function Note($text) { Write-Host "  ! $text" -ForegroundColor Yellow }

# A download as text. Windows PowerShell 5.1 hands files from GitHub's release storage over as bytes.
function Get-Text($url) {
    $content = (Invoke-WebRequest $url -UseBasicParsing).Content
    if ($content -is [byte[]]) { [Text.Encoding]::ASCII.GetString($content) } else { $content }
}

function Stop-IfRunning {
    $running = Get-Process blink -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $blink }
    if ($running) { throw "Blink is running in this folder. Stop it first (Ctrl+C in its window: the server saves and stops), then run this again." }
}

if ($Uninstall) {
    Step "Removing Blink from $dir"
    Stop-IfRunning
    if ((Test-Path $blink) -and (Test-Path "$dir\blink.toml")) {
        & $blink restore
        if ($LASTEXITCODE -ne 0) { throw "blink restore failed; nothing was removed" }
    }
    Remove-Item -Force -ErrorAction SilentlyContinue $blink, "$dir\blink.toml", "$dir\start-blink.bat"
    Write-Host ""
    Write-Host "Done. Start your server the way you did before Blink." -ForegroundColor Green
    return
}

Step "Checking $dir"
$jars = @(Get-ChildItem -Path $dir -Filter *.jar -File -ErrorAction SilentlyContinue)
if ($jars.Count -eq 0 -and -not (Test-Path "$dir\run.bat")) {
    throw "No Minecraft server here. Run this in the folder with your server (paper.jar, a Fabric or vanilla jar, or Forge's run.bat)."
}
Stop-IfRunning

Step "Putting blink.exe in $dir"
if ($env:BLINK_BINARY) {
    Copy-Item $env:BLINK_BINARY $blink -Force
} else {
    $url = "https://github.com/$repo/releases/latest/download/$asset"
    $tmp = "$blink.download"
    Invoke-WebRequest $url -OutFile $tmp -UseBasicParsing
    $expected = ((Get-Text "$url.sha256").Trim() -split '\s+')[0].ToLower()
    $actual = (Get-FileHash $tmp -Algorithm SHA256).Hash.ToLower()
    if ($expected -ne $actual) { Remove-Item $tmp; throw "The checksum of blink.exe does not match; try again later" }
    Move-Item $tmp $blink -Force
}
& $blink --version

# Java: Paper starts fastest with Java 26; any server needs the Java its Minecraft version needs
$java = $null
if ($env:JAVA_HOME -and (Test-Path "$env:JAVA_HOME\bin\java.exe")) { $java = "$env:JAVA_HOME\bin\java.exe" }
elseif (Get-Command java -ErrorAction SilentlyContinue) { $java = (Get-Command java).Source }
else {
    $found = Get-ChildItem "C:\Program Files\*\*\bin\java.exe" -ErrorAction SilentlyContinue | Select-Object -Last 1
    if ($found) { $java = $found.FullName }
}
if (-not $java) {
    Note "No Java found. Install it, for example: winget install EclipseAdoptium.Temurin.26.JDK"
}

if (Test-Path "$dir\blink.toml") {
    Step "Keeping your blink.toml (delete it and run this again for a fresh one)"
} else {
    Step "Writing blink.toml"
    & $blink init
    if ($LASTEXITCODE -ne 0) { throw "blink init failed (see above)" }
}

Set-Content -Path "$dir\start-blink.bat" -Value "@echo off`r`ncd /d `"%~dp0`"`r`nblink.exe start`r`npause" -Encoding ASCII

$eula = (Test-Path "$dir\eula.txt") -and (Select-String -Path "$dir\eula.txt" -Pattern '^\s*eula=true' -Quiet)
Write-Host ""
Write-Host "Done. Start your server with start-blink.bat from now on." -ForegroundColor Green
if (-not $eula) {
    Note "First accept the Minecraft EULA (https://aka.ms/MinecraftEULA): put eula=true in eula.txt."
}
Write-Host "  Players join on the same address as always. 30 seconds after the last player leaves, the"
Write-Host "  server saves and freezes, and Windows compresses its memory. The next join wakes it up."
Write-Host "  Type stop or press Ctrl+C in that window: the server saves and stops, also while frozen."
Write-Host "  Undo: & ([scriptblock]::Create((irm https://raw.githubusercontent.com/$repo/main/install.ps1))) -Uninstall"
