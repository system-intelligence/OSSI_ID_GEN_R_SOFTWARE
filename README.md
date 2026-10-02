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

`VP-SIGNATURE.png` must be in the project root when building: it is bundled into the installer.
The installer is written to `src-tauri/target/release/bundle/nsis/OSSI ID Card Generator_<version>_x64-setup.exe`.
Copy that one file to the other PC and run it. It installs for the current Windows user (no administrator rights needed)
and adds a Start menu shortcut. The card templates and the signature are included in the installer,
so keep the setup file within the office - anyone with a copy can extract the signature from it.

On an installed PC the app keeps its working files in **`Documents\OSSI ID Generator\`**:

| Path | Contents |
|---|---|
| `VP-SIGNATURE.png` | Optional: a signature placed here replaces the bundled one, without reinstalling |
| `data\` | `id-generator.db` and `pin-config.json` - the PIN is set on first start |
| `ID\` | Generated cards for CorelDRAW |

These files survive updates and uninstalling. To move existing records to that PC, copy the `data\` folder
(and `ID\` for the saved cards) into `Documents\OSSI ID Generator\` before starting the app.

## Correcting a card (Edit)

**Records → View → Edit** fixes a misspelling or updates details without making a new card:

- The **control number never changes**, so **hire date and city of birth are locked** (they make up the number).
  If one of those is wrong, make a new card instead.
- The photo and signatures are kept from the saved card; **Replace** swaps them.
- Saving regenerates the front and back in place and records every change (from → to) in the card's history.
- If the card was already printed, print it again with the reprint reason **Correction** and collect the old card.
- A card sent to another PC can only be edited on that PC.

## Printing on another PC (Send / Receive)

To print cards made on PC A with the ID printer on PC B, without printing any card twice:

1. **PC A: Records → Send to other PC.** Only cards PC A has never printed are included. Choose a password
   (8+ characters); the encrypted file is saved in `Documents\OSSI ID Generator\exports\`. Those cards are then
   marked **"Sent to other PC"** on PC A, and printing them there asks for a reprint reason first.
2. Copy the `.ossi` file to PC B (USB or shared folder). Give the password separately - never on the same USB.
3. **PC B: Records → Receive**, pick the file and enter the password. Cards are only added: nothing on PC B is
   overwritten, and a card PC B already has is skipped. PC B then prints them like its own cards.

Without the password the file cannot be read (Argon2id key + AES-256-GCM). Delete it from the USB once imported.
