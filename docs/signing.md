# Signing

Two separate keys are involved. Both are created on **your** machine, never in this repository
and never by an automated session.

| Key | Purpose | Where the private part lives | Public part |
|-----|---------|------------------------------|-------------|
| Code-signing certificate (`.pfx`) | Signs the Windows installer and app | Repository secrets `WINDOWS_CERT_PFX_BASE64`, `WINDOWS_CERT_PASSWORD`, plus your offline backup | `.cer`, given to testers if they want to trust it |
| Update-signing key pair (Tauri) | **Not used yet.** The updater checks the SHA-256 in the release's `SHA256SUMS.txt`. Reserved for a signed update manifest | Repository secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, plus your offline backup | `.pub` file, copied into `tauri.conf.json` and committed |

## What is signed

- The **installer** is signed by the release workflow (`scripts/windows/sign.ps1`) when the
  secrets `WINDOWS_CERT_PFX_BASE64` and `WINDOWS_CERT_PASSWORD` exist. Without them the workflow
  still builds an unsigned installer and says so in the log.
- The **program inside the installer** (`CargoFrame.exe`) is **not signed yet**. Windows shows the
  publisher on the installer, but SmartScreen and security tools may still treat the installed
  program as unknown. Signing it means signing before the installer is packed; that is the first
  thing to add when a trusted certificate replaces the self-signed one.
- The Linux package is not signed. Publish `SHA256SUMS.txt` (the release draft includes it).
- None of this has run with a real certificate yet.

## Private beta (now)

- The certificate is self-signed. Testers see an "unknown publisher" warning. That is expected.
- "Check for updates" (Settings, General) works while the repository is private, with a
  read-only GitHub token you create (fine-grained, *Contents: read* on this repository only). It is
  kept in the system credential store. The daily automatic check is **off** by default. You can
  also update by hand: run the newer installer, which upgrades in place and keeps your settings.
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
5. Make the repository public, then the token is no longer needed: remove it in Settings, General, Updates. Consider a signed update manifest on top of the checksum file.
