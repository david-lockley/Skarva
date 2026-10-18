# Development

Use the latest stable Rust toolchain. The checked-in Cargo.lock defines the desktop application dependencies. Normal builds do not need Python, Node, Office, or a browser engine.

## macOS

Install Xcode Command Line Tools (`xcode-select --install`), then run `cargo run --release`. `sh scripts/package-macos.sh release` builds an ad-hoc signed `target/release/Skarva.app`. Ad-hoc signing is useful for local previews; public distribution needs a Developer ID signature and Apple notarization for a smooth Gatekeeper experience. CI packages the preview bundle without those distribution credentials.

## Windows

Install Rust's MSVC toolchain and Visual Studio Build Tools with Desktop development with C++. The Windows SDK supplies the resource compiler for the embedded app icon. Run `cargo run --release` or `./scripts/package-windows.ps1` in PowerShell. The latter produces a ZIP containing `skarva.exe` and license notices. The release app does not open a console window. Public builds are currently unsigned and can trigger SmartScreen.

## Linux

On Ubuntu/Debian, install desktop build dependencies:

```sh
sudo apt-get install build-essential pkg-config libdbus-1-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libx11-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libegl1-mesa-dev libvulkan1
cargo run --release
```

Use a graphical session with X11 or Wayland, Vulkan or OpenGL/EGL drivers, and an unlocked Secret Service provider (GNOME Keyring, or KeePassXC with Secret Service enabled). File dialogs require a working `xdg-desktop-portal` and a desktop-specific portal backend. Minimal window managers may require installing and starting these services explicitly.

`sh scripts/package-linux.sh` creates an architecture-specific tarball. Extract it and run `./skarva`. For application-menu integration, install the binary on your PATH as `skarva`, copy `share/applications/dev.skarva.app.desktop` into `~/.local/share/applications/`, and copy the PNG under `share/icons/` into the corresponding `~/.local/share/icons/` directory. The tarball relies on the host's desktop libraries and is not a static universal Linux build.

## Checks

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

GitHub Actions runs these checks and builds native packages on macOS, Windows, and Ubuntu. CI compilation does not replace testing trackpad controls, keyring access, file dialogs, and OAuth in a real desktop session.

For clean-profile QA, run `cargo run -- --data-dir /absolute/path/to/test-profile`. This keeps test projects separate from your normal data. Do not connect live accounts unless a test requires them. `--help` and `--version` also work without a display server in debug builds.

## Assets

After changing dependencies, install `cargo-about` (`cargo install cargo-about --version 0.9.2 --features cli --locked`) and regenerate notices with `cargo about generate -o licenses/THIRD_PARTY.txt packaging/licenses.hbs`. Review any unrecognized licenses before packaging. The generated report is included in all platform packages.

Figma source exports live in `assets/app-icon.png`, `assets/logo-mark.png`, and `assets/logo-mark.svg`. The OS icon uses the gradient; the interface uses the transparent mark. Install Pillow and run `python3 scripts/generate-icons.py` to regenerate platform icons after changing the source images.
