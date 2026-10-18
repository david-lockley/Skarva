$ErrorActionPreference = "Stop"
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
$packageId = cargo pkgid --offline
if ($LASTEXITCODE -ne 0) { throw "Could not read application version" }
$version = ($packageId -split '@')[-1]
$architecture = switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { "x64" }
    "ARM64" { "arm64" }
    default { throw "Unsupported Windows architecture" }
}
$package = "skarva-$version-windows-$architecture"
$stage = Join-Path "target/release" $package
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item "target/release/skarva.exe", "LICENSE", "README.md", "THIRD_PARTY_NOTICES.md" -Destination $stage
Copy-Item "licenses" -Destination $stage -Recurse -Force
$archive = Join-Path "target/release" "$package.zip"
Compress-Archive -Path "$stage/*" -DestinationPath $archive -Force
Write-Output $archive
$makensisCommand = Get-Command makensis.exe -ErrorAction SilentlyContinue
$makensis = if ($makensisCommand) { $makensisCommand.Source } else { $null }
if (-not $makensis) {
    $candidate = Join-Path ${env:ProgramFiles(x86)} "NSIS/makensis.exe"
    if (Test-Path $candidate) { $makensis = (Get-Item $candidate).FullName }
}
if (-not $makensis) { throw "Install NSIS (choco install nsis) to build the Windows installer." }
$installer = Join-Path (Resolve-Path "target/release") "$package-setup.exe"
$numericVersion = ($version -split '[-+]')[0] + ".0"
& $makensis "/DAPP_VERSION=$version" "/DNUMERIC_VERSION=$numericVersion" "/DPACKAGE_DIR=$((Resolve-Path $stage).Path)" "/DOUT_FILE=$installer" "packaging/windows/installer.nsi"
if ($LASTEXITCODE -ne 0) { throw "Installer build failed" }
Write-Output $installer
