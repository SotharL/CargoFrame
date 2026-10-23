<#
.SYNOPSIS
  Creates CargoFrame's private-beta signing material on YOUR machine.

.DESCRIPTION
  1. A self-signed code-signing certificate (.pfx) for the Windows installer. Testers will see
     an "unknown publisher" warning; a publicly trusted certificate replaces it before the
     public release.
  2. The update-signing key pair used by the in-app updater (via the Tauri CLI).

  Nothing is uploaded and nothing is written inside a git repository. The script prints the
  commands that store the private parts as GitHub repository secrets; you run them yourself.

  Keep an offline backup of both private files. The update public key is embedded in every
  build, so losing the private key strands installed copies.

.EXAMPLE
  .\new-signing-material.ps1 -OutDir "$HOME\CargoFrame-signing"
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$OutDir,
    [string]$Subject = 'CN=CargoFrame (private beta)',
    [int]$Years = 3,
    [switch]$SkipUpdateKey
)

$ErrorActionPreference = 'Stop'

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$out = (Resolve-Path -LiteralPath $OutDir).Path

# Refuse to write secrets inside any git working tree.
$probe = $out
while ($probe) {
    if (Test-Path -LiteralPath (Join-Path $probe '.git')) {
        throw "OutDir '$out' is inside a git repository ('$probe'). Choose a folder outside it."
    }
    $parent = Split-Path -Parent $probe
    if (-not $parent -or $parent -eq $probe) { break }
    $probe = $parent
}

# 1. Code-signing certificate ------------------------------------------------------------
$pfxPassword = Read-Host -AsSecureString 'Choose a password for the .pfx file'
$cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 `
    -KeyExportPolicy Exportable `
    -CertStoreLocation 'Cert:\CurrentUser\My' `
    -NotAfter (Get-Date).AddYears($Years)

$pfx = Join-Path $out 'cargoframe-beta.pfx'
$cer = Join-Path $out 'cargoframe-beta.cer'
Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $pfxPassword | Out-Null
Export-Certificate -Cert $cert -FilePath $cer | Out-Null
[Convert]::ToBase64String([IO.File]::ReadAllBytes($pfx)) |
    Set-Content -LiteralPath (Join-Path $out 'WINDOWS_CERT_PFX_BASE64.txt') -NoNewline

Write-Host "Certificate thumbprint: $($cert.Thumbprint)"
Write-Host "Written: $pfx (private), $cer (public)"

# 2. Update-signing key pair -------------------------------------------------------------
if (-not $SkipUpdateKey) {
    if (Get-Command npx -ErrorAction SilentlyContinue) {
        $key = Join-Path $out 'cargoframe-update.key'
        Write-Host 'Generating the update-signing key pair (you will be asked for a password)...'
        npx --yes @tauri-apps/cli signer generate -w $key
        Write-Host "Written: $key (private) and $key.pub (public)"
    } else {
        Write-Warning 'npx not found. Install Node.js, then run: npx @tauri-apps/cli signer generate -w <path>'
    }
}

Write-Host @"

Next steps (run them yourself):

  1. Store the private parts as repository secrets:
       Get-Content -Raw "$out\WINDOWS_CERT_PFX_BASE64.txt" | gh secret set WINDOWS_CERT_PFX_BASE64 --repo SotharL/CargoFrame
       gh secret set WINDOWS_CERT_PASSWORD --repo SotharL/CargoFrame            # the .pfx password
       Get-Content -Raw "$out\cargoframe-update.key" | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo SotharL/CargoFrame
       gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo SotharL/CargoFrame

  2. Copy the PUBLIC update key (the .pub file) into the updater settings in tauri.conf.json.
     It is safe to commit; the private files are not.

  3. Back up '$out' offline (encrypted drive), then delete WINDOWS_CERT_PFX_BASE64.txt.

  4. Optional: remove the certificate from your own store when you no longer need it:
       Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)"
"@
