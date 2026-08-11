# Download Ephemera

## Get a build

**→ [github.com/ManasesLovera/ephemera/releases](https://github.com/ManasesLovera/ephemera/releases)**

Every tagged release lists its own binaries under **Assets** — the top of that page is
always the latest version; older tags stay listed below it if you need a previous one.
Every push to `main` also runs the full test suite and a from-scratch build via CI, so
`main` itself is always known-buildable even between tags — see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) and
[`.github/workflows/release.yml`](.github/workflows/release.yml).

### Which file to pick

Ephemera is a single native Slint binary — no installer, no bundle, no runtime
dependency to install separately. Download the asset for your platform and run it.

| Your system | Asset name |
| --- | --- |
| Linux x86_64 | `ephemera-app-linux-x86_64` |
| Linux arm64 | `ephemera-app-linux-arm64` |
| Windows x64 | `ephemera-app-windows-x64.exe` |
| macOS Apple Silicon (M1/M2/M3/M4) | `ephemera-app-macos-arm64` |
| macOS Intel | `ephemera-app-macos-x64` |

On Linux/macOS, mark it executable after downloading: `chmod +x ephemera-app-*`.

## Build it yourself

Needed on every platform: **[Rust](https://rustup.rs/)** (installs `cargo`, which does
the actual compiling) — that's it, no Node/pnpm required.

```bash
git clone https://github.com/ManasesLovera/ephemera.git
cd ephemera/crates/ephemera-app
cargo build --release
```

Output: `target/release/ephemera-app` (`.exe` on Windows).

#### Linux

Also needs Slint's windowing/rendering build dependencies (Debian/Ubuntu shown — see
[Slint's Linux prerequisites](https://slint.dev/) for other package managers):

```bash
sudo apt install -y libfontconfig1-dev libfreetype6-dev libx11-dev libxkbcommon-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxcursor-dev \
  libxrandr-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev libwayland-dev \
  libgtk-3-dev build-essential pkg-config
```

Cargo builds for whatever architecture it's run on natively, so this is the same
process on x86_64 or arm64 — no cross-compilation setup needed when building on the
target machine itself.

#### macOS

Also needs Xcode Command Line Tools: `xcode-select --install`.

To build for the *other* Mac architecture than the one you're on (e.g. an Intel binary
built on Apple Silicon), add the target first:
`rustup target add x86_64-apple-darwin`, then
`cargo build --release --target x86_64-apple-darwin` (from `crates/ephemera-app`).

#### Windows

Needs the **Visual Studio Build Tools** (C++ build tools workload, needed to link the
Rust binary).

## What each build needs at runtime

Regardless of how you got the binary:

- **RAM and Disk tiers work standalone.** No setup needed.
- **Database tier** needs Postgres reachable at the `DATABASE_URL` in a `.env` file
  next to the binary (or set as an environment variable) — see
  [`docker-compose.yml`](docker-compose.yml) and
  [`docs/08-database-tier.md`](docs/08-database-tier.md). If unreachable, that panel
  just shows "offline" — the rest of the app is unaffected.
- **Cloud tier** needs a GCS service-account key at `gcs-key.json` next to the `.env`
  file — see [`docs/09-gcs-tier.md`](docs/09-gcs-tier.md) for the full setup guide.
  Same graceful-offline behavior if it's missing.

## Verifying what you downloaded

Every release binary is built directly from a tagged commit by the GitHub Actions
workflows in this repo — you can always audit exactly what went into any release by
checking out that tag (`git checkout v0.2.0`, etc.) and reading the source, rather than
trusting the binary alone.
