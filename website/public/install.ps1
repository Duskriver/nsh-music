# Installs nsh-music, or updates it (Windows):
#   irm https://duskriver.github.io/nsh-music/install.ps1 | iex
# $env:NSH_MUSIC_BASE_URL  where the release assets are (default: the latest GitHub release)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"  # the progress bar slows downloads down a lot
# Windows PowerShell 5.1 may not offer TLS 1.2 by default.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
if ($Arch -ne "AMD64") { throw "nsh-music: the Windows build needs x86_64" }

$Base = if ($env:NSH_MUSIC_BASE_URL) { $env:NSH_MUSIC_BASE_URL } else { "https://github.com/Duskriver/nsh-music/releases/latest/download" }
$Asset = "nsh-music-windows-x86_64.zip"
$Dir = Join-Path $env:LOCALAPPDATA "Programs\nsh-music"
$Work = Join-Path ([IO.Path]::GetTempPath()) ("nsh-music-" + [IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $Work | Out-Null

try {
  Write-Host "Downloading $Asset"
  $Zip = Join-Path $Work $Asset
  Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset" -OutFile $Zip
  $Sums = Join-Path $Work "sha256sums"
  Invoke-WebRequest -UseBasicParsing -Uri "$Base/sha256sums" -OutFile $Sums
  $Line = Get-Content $Sums | Where-Object { (($_ -split "\s+")[1] -replace "^\*", "") -eq $Asset } | Select-Object -First 1
  if (-not $Line -or ($Line -split "\s+")[0] -ne (Get-FileHash -Algorithm SHA256 -Path $Zip).Hash) {
    throw "nsh-music: the checksum of $Asset does not match; nothing was installed"
  }

  $Unpacked = Join-Path $Work "unpacked"
  Expand-Archive -Path $Zip -DestinationPath $Unpacked
  $Exe = Get-ChildItem -Path $Unpacked -Recurse -Filter "nsh-music.exe" | Select-Object -First 1
  if (-not $Exe) { throw "nsh-music: $Asset holds no nsh-music.exe" }

  # A running app locks its files.
  Get-Process -Name nsh-music -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Dir\*" } | Stop-Process -Force
  Start-Sleep -Milliseconds 500
  if (Test-Path $Dir) { Remove-Item -Recurse -Force $Dir }
  New-Item -ItemType Directory -Force -Path (Split-Path $Dir) | Out-Null
  Move-Item -Path $Exe.Directory.FullName -Destination $Dir

  $Shell = New-Object -ComObject WScript.Shell
  $Link = $Shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath("Programs")) "nsh-music.lnk"))
  $Link.TargetPath = Join-Path $Dir "nsh-music.exe"
  $Link.WorkingDirectory = $Dir
  $Link.IconLocation = "$(Join-Path $Dir 'nsh-music.exe'),0"
  $Link.Save()

  Write-Host "Installed $Dir"
  Write-Host ""
  Write-Host "Open nsh-music from the Start menu."
} finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Work
}
