# Signing

Two separate keys are involved. Both are created on **your** machine, never in this repository
and never by an automated session.

| Key | Purpose | Where the private part lives | Public part |
|-----|---------|------------------------------|-------------|
| Code-signing certificate (`.pfx`) | Signs the Windows installer and app | Repository secrets `WINDOWS_CERT_PFX_BASE64`, `WINDOWS_CERT_PASSWORD`, plus your offline backup | `.cer`, given to testers if they want to trust it |
| Update-signing key pair (Tauri) | Signs update packages and the update manifest | Repository secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, plus your offline backup | `.pub` file, copied into `tauri.conf.json` and committed |

## Private beta (now)

- The certificate is self-signed. Testers see an "unknown publisher" warning. That is expected.
- The in-app updater is **off** while the repository is private, because it would need release
  assets that anyone can download. Updates are manual: run the newer installer, which upgrades in
  place.
- Generate both with `scripts/windows/new-signing-material.ps1`, then run the `gh secret set`
  commands it prints.

## The update key cannot be replaced casually

The public key is built into every installed copy. If the private key is lost or leaked, copies
already installed can no longer be updated automatically and testers must reinstall. Keep an
offline, encrypted backup, and do not paste the key into chats, tickets or issues.

## Before the repository goes public

1. Replace the self-signed certificate with a publicly trusted code-signing certificate.
2. Scan the git history for secrets and personal paths; rotate anything found.
3. Keep OAuth client secrets out of the repository (use PKCE public clients, or CI secrets).
4. Choose a license; add a security policy (`SECURITY.md`).
5. Turn the updater on and publish the signed manifest as a release asset.
