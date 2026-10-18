$ErrorActionPreference = "Stop"
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
$package = "skarva-windows-$env:PROCESSOR_ARCHITECTURE".ToLowerInvariant()
$stage = Join-Path "target/release" $package
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item "target/release/skarva.exe", "LICENSE", "README.md", "THIRD_PARTY_NOTICES.md" -Destination $stage
Copy-Item "licenses" -Destination $stage -Recurse -Force
$archive = Join-Path "target/release" "$package.zip"
Compress-Archive -Path "$stage/*" -DestinationPath $archive -Force
Write-Output $archive
