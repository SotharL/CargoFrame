<#
.SYNOPSIS
  Signs files with the code-signing certificate held in repository secrets. Used by the release
  workflow; does nothing (and says so) when no certificate is configured, so a build without
  secrets still produces an unsigned installer.
.DESCRIPTION
  Reads WINDOWS_CERT_PFX_BASE64 and WINDOWS_CERT_PASSWORD from the environment. The certificate
  is written to a temporary file for the signing call only and removed afterwards. Neither value
  is printed. See docs/signing.md.
.PARAMETER Files
  The files to sign (the installer).
#>
param([Parameter(Mandatory = $true)][string[]]$Files)

$ErrorActionPreference = 'Stop'

if (-not $env:WINDOWS_CERT_PFX_BASE64) {
  Write-Host 'No signing certificate configured (WINDOWS_CERT_PFX_BASE64 is empty): leaving the files unsigned.'
  exit 0
}
if (-not $env:WINDOWS_CERT_PASSWORD) { throw 'WINDOWS_CERT_PASSWORD is empty' }

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
  Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) { throw 'signtool.exe was not found (Windows SDK missing)' }

$pfx = Join-Path ([IO.Path]::GetTempPath()) ("cargoframe-sign-" + [guid]::NewGuid().ToString('N') + '.pfx')
[IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERT_PFX_BASE64))
try {
  foreach ($file in $Files) {
    & $signtool.FullName sign /fd SHA256 /f $pfx /p $env:WINDOWS_CERT_PASSWORD `
      /tr http://timestamp.digicert.com /td SHA256 $file
    if ($LASTEXITCODE -ne 0) { throw "signtool failed for $file" }
    # A self-signed certificate is "not trusted" on purpose; what matters is that it is signed.
    $sig = Get-AuthenticodeSignature -FilePath $file
    if (-not $sig.SignerCertificate) { throw "$file has no signature after signing" }
    Write-Host "signed: $file ($($sig.Status), $($sig.SignerCertificate.Subject))"
  }
}
finally {
  Remove-Item $pfx -Force -ErrorAction SilentlyContinue
}
