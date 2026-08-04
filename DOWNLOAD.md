# Download Ephemera

## Linux x86_64 — ready now

From [release v0.1.0](https://github.com/ManasesLovera/ephemera/releases/tag/v0.1.0):

| File | Use it if… |
| --- | --- |
| [`Ephemera_0.1.0_amd64.AppImage`](https://github.com/ManasesLovera/ephemera/releases/download/v0.1.0/Ephemera_0.1.0_amd64.AppImage) | You want it to just run, on **any** distro — bundles its own runtime, no install needed |
| [`Ephemera_0.1.0_amd64.deb`](https://github.com/ManasesLovera/ephemera/releases/download/v0.1.0/Ephemera_0.1.0_amd64.deb) | You're on Debian, Ubuntu, or a derivative and want it in your package manager |
| [`Ephemera-0.1.0-1.x86_64.rpm`](https://github.com/ManasesLovera/ephemera/releases/download/v0.1.0/Ephemera-0.1.0-1.x86_64.rpm) | You're on Fedora, openSUSE, RHEL, or a derivative |

**AppImage:**

```bash
chmod +x Ephemera_0.1.0_amd64.AppImage
./Ephemera_0.1.0_amd64.AppImage
```

**.deb** (Debian/Ubuntu/Mint/Pop!_OS):

```bash
sudo apt install ./Ephemera_0.1.0_amd64.deb
```

**.rpm** (Fedora/openSUSE/RHEL):

```bash
sudo rpm -i Ephemera-0.1.0-1.x86_64.rpm
# or on dnf-based systems:
sudo dnf install ./Ephemera-0.1.0-1.x86_64.rpm
```

> [!note]
> The `.deb` was built against this build machine's `webkit2gtk-4.1`/`libsoup-3.0`.
> If your distro is old enough not to have those (anything much older than Ubuntu
> 24.04/Debian 13-equivalent), the `.deb`/`.rpm` may fail to install on dependency
> resolution — reach for the **AppImage** instead, or build from source below.

## Not yet built: Windows, macOS, Linux arm64

Nobody has run this on those platforms yet — no CPU architecture or OS beyond Linux
x86_64 has a binary in this repo right now. There are two ways to get one:

### Option A — wait for the next tagged release

[`.github/workflows/release.yml`](.github/workflows/release.yml) builds all of the
following automatically whenever a `v*` tag is pushed, and attaches them to that
release: Linux x86_64, Linux arm64, Windows x86_64, macOS Apple Silicon, macOS Intel.
v0.1.0 predates this workflow — check the
[releases page](https://github.com/ManasesLovera/ephemera/releases) for anything
tagged v0.2.0 or later.

### Option B — build it yourself right now

Same steps on every platform; only the prerequisite install differs.

#### Windows

1. Install [Rust](https://rustup.rs/), [Node.js LTS](https://nodejs.org/), and
   `pnpm` (`npm install -g pnpm`).
2. Install the **WebView2** runtime — already present on Windows 11 and current
   Windows 10; if missing, get it from
   [Microsoft's WebView2 page](https://developer.microsoft.com/microsoft-edge/webview2/).
3. Install the **Visual Studio Build Tools** (C++ build tools workload) — required
   for linking the Rust binary on Windows.
4. Then:

   ```powershell
   git clone https://github.com/ManasesLovera/ephemera.git
   cd ephemera
   pnpm install
   pnpm tauri build
   ```

   Output: `src-tauri\target\release\bundle\msi\*.msi` and
   `src-tauri\target\release\bundle\nsis\*.exe`.

#### macOS

1. Install [Rust](https://rustup.rs/), [Node.js LTS](https://nodejs.org/), `pnpm`,
   and Xcode Command Line Tools (`xcode-select --install`).
2. Then:

   ```bash
   git clone https://github.com/ManasesLovera/ephemera.git
   cd ephemera
   pnpm install
   pnpm tauri build
   ```

   Output: `src-tauri/target/release/bundle/dmg/*.dmg` and
   `src-tauri/target/release/bundle/macos/*.app`.

   Building for the *other* Mac architecture than the one you're on (e.g. building
   an Intel binary on Apple Silicon) needs the target installed first:
   `rustup target add x86_64-apple-darwin`, then
   `pnpm tauri build --target x86_64-apple-darwin`.

#### Linux — any distro, any architecture (including arm64)

```bash
git clone https://github.com/ManasesLovera/ephemera.git
cd ephemera
```

Install build dependencies (Debian/Ubuntu shown; see
[Tauri's Linux prerequisites](https://v2.tauri.app/start/prerequisites/) for other
package managers):

```bash
sudo apt install -y libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev \
  libsoup-3.0-dev libgtk-3-dev librsvg2-dev build-essential pkg-config \
  libayatana-appindicator3-dev
```

Then:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # if Rust isn't installed
npm install -g pnpm                                              # if pnpm isn't installed
pnpm install
pnpm tauri build
```

Output in `src-tauri/target/release/bundle/{appimage,deb,rpm}/`. This is the same
process whether you're on x86_64 or arm64 — Cargo and the Tauri CLI build for
whatever architecture they're run on natively; no cross-compilation setup needed if
you're building on the target machine itself.

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

Every release binary is built by the GitHub Actions workflows in this repo
([`ci.yml`](.github/workflows/ci.yml) / [`release.yml`](.github/workflows/release.yml))
directly from a tagged commit — you can always audit exactly what went into any release
by checking out that tag and reading the source, rather than trusting the binary alone.
