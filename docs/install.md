# Installing Panic Button

Every release is on the [releases page](https://github.com/ohtf/panic-button/releases). Panic Button is not yet ready for use in a real practice; these downloads are for testing.

## The desktop app

This is the red button, the alerts and the settings window. Install it on every computer that should raise or receive alarms.

| Computer | What to download or run |
|---|---|
| Windows | `panic-button-desktop_<version>_x64_en-US.msi`, then run it. Windows warns that the installer is not signed: click More info, then Run anyway. |
| Mac | `brew install --cask ohtf/tap/panic-button`, or download `panic-button-desktop_<version>_universal.dmg`. The app is not signed by Apple yet; the Homebrew message says how to open it. |
| Debian, Ubuntu, Mint | `panic-button-desktop_<version>_amd64.deb`, then `sudo apt install ./panic-button-desktop_<version>_amd64.deb` |
| Fedora, RHEL, openSUSE | `panic-button-desktop-<version>-1.x86_64.rpm`, then `sudo dnf install ./panic-button-desktop-<version>-1.x86_64.rpm` |
| Arch, CachyOS, Manjaro | `yay -S panic-button-bin` (or any AUR helper) |
| Other Linux | `panic-button-desktop_<version>_amd64.AppImage`. Make it executable and run it. |

The Windows, Debian and Fedora packages start Panic Button whenever someone signs in. With the AppImage, see [home-test.md](home-test.md#installing-on-linux) for how to start it at sign-in.

## Command-line programs

These are for IT staff and servers, not for practice staff:

- `panic-button`: setup and diagnostics from a terminal.
- `panic-button-agent`: Panic Button without a window, for a server or a Raspberry Pi that should receive and forward alarms.
- `panic-button-relay`: the optional relay server (see [packaging/relay/README.md](../packaging/relay/README.md)).

Install them with Homebrew on Mac or Linux:

```sh
brew install ohtf/tap/panic-button-cli ohtf/tap/panic-button-agent ohtf/tap/panic-button-relay
```

or with the install script on the releases page (`panic-button-installer.sh`, and `.ps1` for Windows PowerShell), or with Rust's package manager:

```sh
cargo install panic-button panic-button-agent panic-button-relay
```

Each release includes `SHA256SUMS`, listing the checksum of every file.
