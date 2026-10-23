# Security review (V1)

_Reviewed 2026-10-23, on the code merged to `main` in pull request #1. This is a code review plus tests, not an outside audit._

## What CargoFrame must protect

| Asset | Where it is | The rule |
|-------|-------------|----------|
| Sign-in secrets (API tokens, OAuth tokens, passwords) | The operating system's credential store (Windows Credential Manager, Secret Service on Linux) | Never in a config file, a log, a zip, the window, or an error message. The windows can say *whether* one is stored, never read it back. |
| The capture (screenshot, files, values read from files) | Memory, and a saved zip only when a share fails | Leaves the computer only when you press a sharing button. No telemetry, no background upload. |
| Files outside the project folder | The user's disk | Data rules and the files block read only inside the resolved project folder, read-only, through an allow-list. |

## How the pieces are separated

- **The tray process has no web view.** The Settings and CargoFrame windows are web views created on demand.
- **Web views get no plugin permissions.** `app/capabilities/*.json` grant each window only the app's own commands (and event listening). There is no file-system, shell, HTTP or opener access from JavaScript; the CSP also forbids connecting anywhere but the app backend.
- **The CargoFrame window is the less trusted of the two.** It cannot save items, set secrets, connect or disconnect, or change settings. It can only ask for what the open capture allows: the share uses the layout, connection, mappings and files **held by the backend for that capture**, not what the window sends.
- **Every network call goes through one client** (`core/net`), which enforces the rules in the next section.

## Findings

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | The HTTP client let its library follow redirects without checking each hop. A service (or anyone able to alter its answer) could redirect an https request with a body (307/308) to plain `http://`, or to a local port on this computer, and the screenshot would follow. | Medium | **Fixed.** Redirects are followed by `core/net` itself. Each hop must pass the URL check (https; plain http only between local test servers; no user name in the address), may not leave https for http, and may not carry a sign-in header (`Authorization`, `Proxy-Authorization`, `Cookie`) to another origin. A 301/302/303 on a POST becomes a GET without the body. At most 5 hops. Tests: `core/net/src/url.rs` and `core/net/tests/client.rs`. |
| 2 | The content security policy did not forbid plugins, `<base>` tags, frames or form posts. Nothing in the app used them, so this was hardening. | Low | **Fixed.** Added `object-src 'none'; base-uri 'none'; frame-src 'none'; form-action 'none'`. A test (`shipped_config_tests` in `app/src/lib.rs`) fails if the policy is loosened, if a window is given a non-app permission, or if the CargoFrame window is given a settings or secrets command. |
| 3 | Nothing proved that a stored secret stays out of every file CargoFrame writes. | Low | **Fixed.** `a_stored_secret_is_in_the_secret_store_and_in_no_file` stores a token and searches every file under the settings and data folders. |
| 4 | A secret input sat inside another form (nested forms are invalid HTML). | Low | **Fixed.** It is no longer a form; Enter in the box stores the token and never submits the connection form. |
| 5 | The weekly dependency check (`cargo audit`) found three advisories in the `zip` crate, version 6 (RUSTSEC-2026-0367, -0368, -0369): extracting an archive could follow links outside the target folder or keep set-user-id bits. | Low for CargoFrame | **Fixed.** CargoFrame only *writes* zips and never extracts one, so it was not exposed, but `zip` is now version 9, which has the fixes. |

## Checked and found sound

- **OAuth (Google, Reddit):** one-time listener bound to `127.0.0.1` only, random `state` that must match, PKCE (S256) where the service supports it, sign-in can be cancelled, a wrong state is refused (`core/connectors/src/oauth.rs`).
- **Injection through templates:** values placed into a Custom API's JSON body are JSON-escaped, into paths and queries URL-encoded, and a request that mentions a field nothing fills is refused instead of sent with a hole. A header value with a line break is refused, not sent. Multipart field names cannot break out of their header.
- **Zip contents:** names with `..`, absolute paths or other unsafe forms are errors, not skips (`core/bundle`). A fallback zip holds the screenshot, the share description, the error, the ticked files and a manifest, and no secrets.
- **Reading files:** only inside the project folder, only paths the allow-list accepts, never writing.
- **Opening links:** only `https://`, `http://127.0.0.1`, `http://localhost` and `claude://claude.ai/` can be opened from a window.
- **Logs:** record events (trigger id, connection id, attempt count, failure class, timings). They do not record window titles, file names, payloads, headers or tokens. The log file is size-capped.
- **Errors from services:** HTML pages and very long messages are cut before they reach a window.
- **Updates:** the in-app updater is off while the repository is private, so there is no update channel to attack yet.
- **Dependencies:** `npm audit` and `cargo audit` report no vulnerabilities. A weekly and on-change workflow (`.github/workflows/security.yml`) runs both; its first run is what found finding 5. `cargo audit` still lists nine *warnings*, all "unmaintained" or "unsound" notices on libraries pulled in by Tauri 2 (GTK 3 bindings, two build-time helpers) and by the credential-store crate (D-Bus bindings, `zbus` and `zvariant`). They are not vulnerabilities and the workflow does not fail on them, but they are the list to re-read at each release.

## Known limits (honest list)

- **Credential store on Linux** needs a running Secret Service (GNOME Keyring, KWallet). Without one, a connection cannot keep its token and the Connections page says so. It has been exercised in unit tests only, not on a real desktop.
- **Signing.** The release workflow signs the Windows installer when the certificate secrets exist. The program inside the installer is **not** signed yet, and the beta certificate is self-signed, so Windows will warn. Untested until you add the secrets (see `docs/signing.md`).
- **Custom API connections are user-defined.** CargoFrame requires https and checks every redirect, but it cannot know whether the service you describe deserves the screenshot. The share button's destination is always shown by name.
- **An exported settings file** holds layouts, data rules and info types only. It names the connection a layout uses, and may name files a data rule reads (`package.json`, `logs/*.log`) and text you wrote in a layout, so read it before handing it on. Connections, tokens, keys, triggers and shortcuts are never in it. Importing reads a file you pick (at most 4 MB, 500 items per group), checks every item like one typed into Settings, and changes nothing until you have seen the preview and chosen. The export, import and template commands are given to the Settings window only; the CargoFrame window cannot call them (checked by `shipped_config_tests`).
- **A saved fallback zip contains your screenshot and files** and lives in the safe folder you chose (or CargoFrame's own data folder). Protect that folder like the originals.
- **Screen capture on Wayland** goes through the desktop portal and asks permission; behavior on each compositor is the spike S3 question.
- **Before the repository goes public:** history scan for secrets and personal paths, a `SECURITY.md`, a licence, a trusted certificate (`docs/signing.md`).
