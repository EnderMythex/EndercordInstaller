# EnderCord Installer

A fast, native installer for [EnderCord](https://github.com/EnderMythex/Endercord), written in Rust. It patches your Discord desktop app to load EnderCord, and can repair, uninstall, or add [OpenAsar](https://github.com/GooseMod/OpenAsar).

## Download

Windows

| | X64 | ARM64 |
| --- | --- | --- |
| GUI | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstaller.exe) | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstaller-arm64.exe) |
| CLI | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstallerCli.exe) | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstallerCli-arm64.exe) |

macOS (universal, Intel and Apple Silicon)

| GUI | CLI |
| --- | --- |
| [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstaller-macos.zip) | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstallerCli-universal) |

The app isn't notarized, so right click it and choose Open the first time.

Linux

| | X64 | ARM64 |
| --- | --- | --- |
| GUI | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstaller-linux) | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstaller-linux-arm64) |
| CLI | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstallerCli-linux) | [Download](https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstallerCli-linux-arm64) |

Run `chmod +x <file>` after downloading. Or use the one liner:

```sh
sh -c "$(curl -sS https://raw.githubusercontent.com/EnderMythex/EndercordInstaller/main/install.sh)"
```

On Windows, from PowerShell:

```powershell
iwr -useb https://raw.githubusercontent.com/EnderMythex/EndercordInstaller/main/install.ps1 | iex
```

## CLI usage

Run it without arguments for an interactive menu, or:

```
EndercordInstallerCli --install [--branch auto|stable|ptb|canary] [--location <path>]
EndercordInstallerCli --repair | --uninstall | --install-openasar | --uninstall-openasar
EndercordInstallerCli --update-self | --version | --debug
```

## Building

Install [Rust](https://rustup.rs), then:

```sh
cargo build --release                        # GUI
cargo build --release --no-default-features  # CLI
```

On Linux the GUI needs `libxkbcommon-dev libwayland-dev libgl1-mesa-dev` to build.

Releases are built by GitHub Actions when you push a tag starting with `v`:

```sh
git tag v1.0.0 && git push origin v1.0.0
```

## Credits

Based on the [Vencord Installer](https://github.com/Vencord/Installer) by Vendicated and contributors, and its Equicord and PawsomeVencord forks. Licensed under the GPL 3.0 or later.
