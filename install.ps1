$ErrorActionPreference = 'Stop'

$Repo = "nhatcoi/procman"
$InstallDir = "$Home\.local\bin"
if (Test-Path "$Home\.cargo\bin") {
    $InstallDir = "$Home\.cargo\bin"
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Write-Host "==> Installing procman on Windows..." -ForegroundColor Cyan

$TargetVersion = $env:VERSION
if (-not $TargetVersion) {
    try {
        $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
        $TargetVersion = $Release.tag_name
    } catch {
        $TargetVersion = "v0.1.3"
    }
}

$ZipName = "procman-x86_64-pc-windows-msvc.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$TargetVersion/$ZipName"
$TempZip = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), $ZipName)

Write-Host "==> Downloading $TargetVersion from $DownloadUrl..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZip -UseBasicParsing

$TempExtract = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "procman_extract")
Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force
Move-Item -Path "$TempExtract\procman.exe" -Destination "$InstallDir\procman.exe" -Force
Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue


try {
    $AllReleases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases"
    $TotalDownloads = ($AllReleases.assets.download_count | Measure-Object -Sum).Sum
    if ($TotalDownloads -gt 0) {
        Write-Host "==> Total installations: #$TotalDownloads"
    }
} catch {}

Write-Host ""
Write-Host "==> procman installed successfully to $InstallDir\procman.exe" -ForegroundColor Green
Write-Host "==> Make sure $InstallDir is in your User PATH."
Write-Host "==> Verify by opening a new terminal and running: procman --version"
