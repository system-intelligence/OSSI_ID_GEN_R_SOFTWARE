# OSSI ID Card Generator (Rust / Tauri)

Desktop app for creating Oriental Sentinel Services Inc. employee ID cards (front and back) as SVG files for CorelDRAW.
Rust port of the original Electron app: same screens and features, with a Rust back end built on [Tauri 2](https://tauri.app).

## Folder layout

| Path | Contents |
|---|---|
| `ui/` | The app screens (HTML/CSS/JS); `ui/vendor/` holds Font Awesome so icons work offline |
| `src-tauri/` | Rust back end: database, PIN lock, control numbers, card generation |
| `templates/` | `front-id.svg` and `back-id.svg` card designs |
| `VP-SIGNATURE.png` | Authorized representative's signature for the back ID (**not in git** - add it locally) |
| `data/` | Created on first run: `id-generator.db` (employee records) and `pin-config.json` (**not in git**) |
| `ID/` | Generated cards, one folder per control number (**not in git**) |

## Requirements (Windows)

- [Rust](https://rustup.rs) (stable, MSVC toolchain)
- Microsoft C++ Build Tools
- WebView2 Runtime (already installed on most Windows 10/11 PCs)

## Run

```sh
cd src-tauri
cargo run
```

Place `VP-SIGNATURE.png` in the project root before generating back IDs.
To use an existing Electron database, copy its `src/id-generator.db` (and `src/pin-config.json`) into `data/`,
or set `OSSI_BASE_DIR` to the Electron project folder.

## Test

```sh
cd src-tauri
cargo test
```

## Build the Windows installer

One-time setup: `cargo install tauri-cli --version "^2" --locked`

```sh
cd src-tauri
cargo tauri build
```

The installer is written to `src-tauri/target/release/bundle/nsis/ID Card Generator_1.0.0_x64-setup.exe`.
Copy that one file to the other PC and run it. It installs for the current Windows user (no administrator rights needed)
and adds a Start menu shortcut. The card templates are included in the installer.

On an installed PC the app keeps its working files in **`Documents\OSSI ID Generator\`**:

| Path | Contents |
|---|---|
| `VP-SIGNATURE.png` | **Copy it here by hand** before generating back IDs (it is never bundled) |
| `data\` | `id-generator.db` and `pin-config.json` - the PIN is set on first start |
| `ID\` | Generated cards for CorelDRAW |

These files survive updates and uninstalling. To move existing records to that PC, copy the `data\` folder
(and `ID\` for the saved cards) into `Documents\OSSI ID Generator\` before starting the app.
