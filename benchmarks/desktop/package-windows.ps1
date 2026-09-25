param(
    [ValidateSet('release', 'dist')]
    [string]$Profile = 'release',
    [string]$RuntimeDll = "$env:SystemRoot/System32/vcruntime140.dll"
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path "$PSScriptRoot/../..").Path
$bundleName = if ($Profile -eq 'release') { 'bundle' } else { "$Profile-bundle" }
$bundle = Join-Path $root "target/desktop-benchmark/$bundleName"
$results = Join-Path $PSScriptRoot "results/bundle-$Profile"
New-Item -ItemType Directory -Force $bundle, $results | Out-Null
Copy-Item -LiteralPath (Join-Path $root "target/$Profile/examples/issue_tracker.exe") -Destination (Join-Path $bundle 'issue_tracker.exe')
Copy-Item -LiteralPath $RuntimeDll -Destination (Join-Path $bundle 'vcruntime140.dll')
$manifest = @('issue_tracker.exe', 'vcruntime140.dll') | ForEach-Object {
    $file = Get-Item -LiteralPath (Join-Path $bundle $_)
    @{
        name = $file.Name
        bytes = $file.Length
        sha256 = (Get-FileHash -LiteralPath $file.FullName).Hash.ToLowerInvariant()
    }
}
$manifest | ConvertTo-Json | Set-Content (Join-Path $results 'bundle-manifest.json')
Write-Output "Staged local benchmark bundle: $bundle"
