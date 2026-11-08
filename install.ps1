# Installs Spotish, or updates it (Windows):
#   irm https://huacnlee.github.io/spotish/install.ps1 | iex
# $env:SPOTISH_BASE_URL  where the release assets are (default: the latest GitHub release)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"  # the progress bar slows downloads down a lot
# Windows PowerShell 5.1 may not offer TLS 1.2 by default.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
if ($Arch -ne "AMD64") { throw "Spotish: the Windows build needs x86_64" }

$Base = if ($env:SPOTISH_BASE_URL) { $env:SPOTISH_BASE_URL } else { "https://github.com/huacnlee/spotish/releases/latest/download" }
$Asset = "spotish-windows-x86_64.zip"
$Dir = Join-Path $env:LOCALAPPDATA "Programs\Spotish"
$Work = Join-Path ([IO.Path]::GetTempPath()) ("spotish-" + [IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $Work | Out-Null

try {
  Write-Host "Downloading $Asset"
  $Zip = Join-Path $Work $Asset
  Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset" -OutFile $Zip
  $Sums = Join-Path $Work "sha256sums"
  Invoke-WebRequest -UseBasicParsing -Uri "$Base/sha256sums" -OutFile $Sums
  $Line = Get-Content $Sums | Where-Object { (($_ -split "\s+")[1] -replace "^\*", "") -eq $Asset } | Select-Object -First 1
  if (-not $Line -or ($Line -split "\s+")[0] -ne (Get-FileHash -Algorithm SHA256 -Path $Zip).Hash) {
    throw "Spotish: the checksum of $Asset does not match; nothing was installed"
  }

  $Unpacked = Join-Path $Work "unpacked"
  Expand-Archive -Path $Zip -DestinationPath $Unpacked
  $Exe = Get-ChildItem -Path $Unpacked -Recurse -Filter "Spotish.exe" | Select-Object -First 1
  if (-not $Exe) { throw "Spotish: $Asset holds no Spotish.exe" }

  # A running app locks its files.
  Get-Process -Name Spotish -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Dir\*" } | Stop-Process -Force
  Start-Sleep -Milliseconds 500
  if (Test-Path $Dir) { Remove-Item -Recurse -Force $Dir }
  New-Item -ItemType Directory -Force -Path (Split-Path $Dir) | Out-Null
  Move-Item -Path $Exe.Directory.FullName -Destination $Dir

  $Shell = New-Object -ComObject WScript.Shell
  $Link = $Shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath("Programs")) "Spotish.lnk"))
  $Link.TargetPath = Join-Path $Dir "Spotish.exe"
  $Link.WorkingDirectory = $Dir
  $Link.IconLocation = "$(Join-Path $Dir 'Spotish.exe'),0"
  $Link.Save()

  Write-Host "Installed $Dir"
  Write-Host ""
  Write-Host "Open Spotish from the Start menu. It plays through Spotify's web player in Microsoft Edge."
} finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Work
}
