# Fast gate for Type Foundry. Formats, tests, and lints the workspace.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$app = Split-Path -Parent $PSScriptRoot
Push-Location $app
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo test --workspace
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "typefoundry checks passed"
}
finally {
    Pop-Location
}
