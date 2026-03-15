$ErrorActionPreference = "Stop"

$repo = "SatvikOfficial/RepoCrunch"
$asset = "repocrunch-windows.exe"
$installDir = "$env:LOCALAPPDATA\RepoCrunch"
$binary = "repocrunch.exe"

Write-Host "🔍 Detecting latest release..." -ForegroundColor Cyan

$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$tag = $release.tag_name
$url = "https://github.com/$repo/releases/download/$tag/$asset"

Write-Host "📦 Downloading RepoCrunch $tag..." -ForegroundColor Cyan

if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

$dest = Join-Path $installDir $binary
Invoke-WebRequest -Uri $url -OutFile $dest

# Add to PATH if not already present
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($currentPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$currentPath;$installDir", "User")
    Write-Host "📁 Added $installDir to PATH" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "✅ RepoCrunch $tag installed successfully!" -ForegroundColor Green
Write-Host "   Restart your terminal, then run: repocrunch" -ForegroundColor White
