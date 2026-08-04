# Download Ephemera

## Get a build

**→ [github.com/ManasesLovera/ephemera/releases](https://github.com/ManasesLovera/ephemera/releases)**

Every tagged release lists its own binaries under **Assets** — the top of that page is
always the latest version; older tags stay listed below it if you need a previous one.
Every push to `main` also runs the full test suite and a from-scratch build on every
platform via CI, so `main` itself is always known-buildable even between tags — see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) and
[`.github/workflows/release.yml`](.github/workflows/release.yml).

### Which file to pick

Asset names encode the platform and architecture. On a release's Assets list, look for:

| Your system | File pattern | Notes |
| --- | --- | --- |
| Linux x86_64, any distro | `*_amd64.AppImage` | Bundles its own runtime — works everywhere, nothing to install |
| Debian / Ubuntu / Mint / Pop!_OS | `*_amd64.deb` | `sudo apt install ./<file>.deb` |
| Fedora / openSUSE / RHEL | `*.x86_64.rpm` | `sudo rpm -i <file>.rpm` or `sudo dnf install ./<file>.rpm` |
| Linux arm64 | *(not currently produced — see below)* | Build from source |
| Windows x64 | `*_x64-setup.exe` or `*_x64_en-US.msi` | Either installs the same app; `.msi` if your org manages installs via Group Policy |
| macOS Apple Silicon (M1/M2/M3/M4) | `*_aarch64.dmg` | |
| macOS Intel | `*_x64.dmg` | |

> [!note]
> The Linux `arm64` leg of the release build is currently failing in CI
> (`ubuntu-24.04-arm`, `pnpm tauri build` exits non-zero) and no arm64 Linux asset is
> published yet. Every other platform above — Linux x86_64, Windows x64, macOS
> (both architectures) — has built successfully on every release since v0.2.0. If
> you're on Linux arm64, use the build-from-source steps below in the meantime.

> [!note]
> The `.deb`/`.rpm` are built against a current Ubuntu runner's `webkit2gtk-4.1` /
> `libsoup-3.0`. On a distro old enough not to have those, package install may fail on
> dependency resolution — the AppImage has no such dependency and is the safer default
> on Linux if you're unsure.

## Build it yourself

Needed regardless of platform:

- **[Rust](https://rustup.rs/)** (installs `cargo`, which does the actual compiling)
- **[Node.js](https://nodejs.org/)** LTS
- **[pnpm](https://pnpm.io/installation)** (`npm install -g pnpm`)

Then, on every platform:

```bash
git clone https://github.com/ManasesLovera/ephemera.git
cd ephemera
pnpm install
pnpm tauri build
```

`pnpm tauri build` runs the frontend build via Vite and the Rust build via
`cargo build --release` in sequence, then packages whatever your OS produces. Platform
specifics:

#### Windows

Also needs the **WebView2 runtime** (already present on Windows 11 and current
Windows 10; otherwise grab it from
[Microsoft's WebView2 page](https://developer.microsoft.com/microsoft-edge/webview2/))
and the **Visual Studio Build Tools** (C++ build tools workload, needed to link the
Rust binary).

Output: `src-tauri\target\release\bundle\msi\*.msi` and
`src-tauri\target\release\bundle\nsis\*.exe`.

#### macOS

Also needs Xcode Command Line Tools: `xcode-select --install`.

Output: `src-tauri/target/release/bundle/dmg/*.dmg` and
`src-tauri/target/release/bundle/macos/*.app`.

To build for the *other* Mac architecture than the one you're on (e.g. an Intel binary
built on Apple Silicon), add the target first:
`rustup target add x86_64-apple-darwin`, then
`pnpm tauri build --target x86_64-apple-darwin`.

#### Linux — any distro, any architecture (including arm64)

Also needs the Tauri Linux build dependencies first (Debian/Ubuntu shown — see
[Tauri's Linux prerequisites](https://v2.tauri.app/start/prerequisites/) for other
package managers):

```bash
sudo apt install -y libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev \
  libsoup-3.0-dev libgtk-3-dev librsvg2-dev build-essential pkg-config \
  libayatana-appindicator3-dev
```

Output in `src-tauri/target/release/bundle/{appimage,deb,rpm}/`. Cargo and the Tauri
CLI build for whatever architecture they're run on natively, so this is the same
process on x86_64 or arm64 — no cross-compilation setup needed when building on the
target machine itself. This is currently the only way to get an arm64 Linux build (see
the CI note above).

## What each build needs at runtime

Regardless of how you got the binary:

- **RAM and Disk tiers work standalone.** No setup needed.
- **Database tier** needs Postgres reachable at the `DATABASE_URL` in
  `src-tauri/.env` — see [`docker-compose.yml`](docker-compose.yml) and
  [`docs/08-database-tier.md`](docs/08-database-tier.md). If unreachable, that panel
  just shows "offline" — the rest of the app is unaffected.
- **Cloud tier** needs a GCS service-account key at `src-tauri/gcs-key.json` — see
  [`docs/09-gcs-tier.md`](docs/09-gcs-tier.md) for the full setup guide. Same
  graceful-offline behavior if it's missing.

## Verifying what you downloaded

Every release binary is built directly from a tagged commit by the GitHub Actions
workflows in this repo — you can always audit exactly what went into any release by
checking out that tag (`git checkout v0.2.0`, etc.) and reading the source, rather than
trusting the binary alone.
