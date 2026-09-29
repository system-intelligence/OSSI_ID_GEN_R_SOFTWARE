# OSSI ID Card Generator (Rust / Tauri)

Desktop app for creating Oriental Sentinel Services Inc. employee ID cards (front and back) as SVG files for CorelDRAW.
Rust port of the original Electron app: same screens and features, with a Rust back end built on [Tauri 2](https://tauri.app).

## Folder layout

| Path | Contents |
|---|---|
| `ui/` | The app screens (HTML/CSS/JS), unchanged from the Electron version |
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
