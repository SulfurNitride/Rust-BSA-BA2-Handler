# BSA/BA2 Tool

A GUI and CLI tool for packing and unpacking Bethesda archive files.

You can find me in the [NaK Discord](https://discord.gg/9JWQzSeUWt) 

If you want to support the things I put out, I do have a [Ko-Fi](https://ko-fi.com/sulfurnitride) I will never charge money for any of my content.

The Nexus release if you prefer [here](https://www.nexusmods.com/site/mods/1679). The Github for source code or the release [here](https://github.com/SulfurNitride/Rust-BSA-BA2-Handler).

## Supported Formats

- **BSA** — Morrowind, Oblivion, Fallout 3, Fallout: New Vegas, Skyrim LE, Skyrim SE
- **BA2** — Fallout 4, Fallout 76, Fallout 4 Next Gen, Starfield

## Download

Grab the latest release from the [Releases](https://github.com/SulfurNitride/Rust-BSA-BA2-Handler/releases) page.

## Usage

### GUI

Run the executable with no arguments to launch the GUI. Or by double clicking it.

### CLI

```
bsa-ba2-tool unpack <archive> [output_folder]
bsa-ba2-tool pack <folder> <output> <game>
bsa-ba2-tool list <archive>
```

#### Game Versions

| Argument       | Game                            |
|----------------|---------------------------------|
| `morrowind`    | Morrowind (BSA)                 |
| `oblivion`     | Oblivion (BSA v103)             |
| `fo3`          | Fallout 3 (BSA v104)            |
| `fonv`         | Fallout: New Vegas (BSA v104)   |
| `skyrimle`     | Skyrim LE (BSA v104)            |
| `skyrimse`     | Skyrim SE (BSA v105)            |
| `fo4-fo76`     | Fallout 4 / Fallout 76 (BA2 v1) |
| `fo4ng-v7`     | Fallout 4 Next Gen (BA2 v7)     |
| `fo4ng-v8`     | Fallout 4 Next Gen (BA2 v8)     |
| `starfield-v2` | Starfield (BA2 v2)              |
| `starfield-v3` | Starfield (BA2 v3)              |

## Building

```
cargo build --release
```


## License

MIT
