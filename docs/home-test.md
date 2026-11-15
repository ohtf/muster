# Home test on a few Windows and Linux computers

This is a guide for a small test before Panic Button goes near a practice. It covers getting the installers, setting up a test practice on one computer, adding the others, and what to try.

Use computers on the same home network. Panic Button finds the others on the network by itself, so nothing else needs setting up.

## Getting the installers

The Installers workflow on GitHub builds both (Actions, then Installers, then Run workflow). When it finishes, download `panic-button-windows-msi` and `panic-button-linux` from the run's page. Each is a zip containing:

- `Panic Button_0.1.0_x64_en-US.msi` for Windows;
- `Panic Button_0.1.0_amd64.AppImage` for any 64-bit Linux, including Arch;
- `Panic Button_0.1.0_amd64.deb` for Debian and Ubuntu.

If GitHub Actions is not available, see [Building the MSI on a Windows PC](#building-the-msi-on-a-windows-pc) at the end.

## Installing on Windows

1. Before you start, make sure Windows treats your home network as private: Settings, Network and internet, then your Wi-Fi or Ethernet connection, and choose Private network. The installer only opens Panic Button's port on private and work networks.
2. Run the MSI. The installer is not signed yet, so Windows shows "Windows protected your PC". Click More info, then Run anyway. It asks for administrator permission.
3. On the last screen, leave Launch Panic Button ticked and click Finish. Panic Button also starts whenever anyone signs in.

To remove it, use Settings, Apps, Installed apps, Panic Button, Uninstall. That also removes its firewall rules. Your test practice stays in `C:\ProgramData\Panic Button`; delete that folder for a clean start.

Use one Windows account per computer for this test. Panic Button keeps its data in `C:\ProgramData\Panic Button`, and for now only the account that first ran it can write there.

## Installing on Linux

On Arch, or anything that is not Debian or Ubuntu, use the AppImage:

```sh
mkdir -p ~/Applications
mv ~/Downloads/Panic Button_0.1.0_amd64.AppImage ~/Applications/
chmod +x ~/Applications/Panic Button_0.1.0_amd64.AppImage
~/Applications/Panic Button_0.1.0_amd64.AppImage
```

If AppImageLauncher is installed, it asks whether to integrate the AppImage. Either answer works.

To start it when you sign in:

```sh
mkdir -p ~/.config/autostart
printf '[Desktop Entry]\nType=Application\nName=Panic Button\nExec=%s\n' \
  ~/Applications/Panic Button_0.1.0_amd64.AppImage > ~/.config/autostart/panic-button.desktop
```

On Debian or Ubuntu, install the deb instead (`sudo apt install ./Panic Button_0.1.0_amd64.deb`). It starts Panic Button at sign-in by itself.

If a firewall is running (for example `ufw` or `firewalld`), allow UDP and TCP port 47474.

On Wayland the desktop decides where windows go, so the red button may not sit in the corner. On niri, the window rules in [gui/README.md](gui/README.md#wayland) put it there.

## Setting up the test practice

Pick one computer to be the first admin.

1. In the Panic Button window, choose Set up a new practice.
2. Give it a name, a name for this computer, and a passphrase of at least 12 characters. Save the recovery kit where it suggests. For a test, any passphrase will do, but keep the kit until the test is over: it is the only way to get admin access back.
3. Choose a room, then try the practice hold. It does not alert anyone.
4. Open the Admin tab, and under Add a computer, click Create code. Copy the code.

On each of the other computers:

1. In the Panic Button window, choose Join an existing practice.
2. Paste the code, then choose a room.

Send the code to yourself in whatever way is handy (a note, a message to yourself). Anyone who has it can join your test practice until it expires, which does not matter at home but would in a practice.

Within half a minute, the small shape on each button should turn into a green circle. The Status tab on any computer lists the others.

## What to try

Call for help from each computer in turn. Hold the red button until the ring fills (one and a half seconds). Every other computer should show the red Help needed window and sound the alarm within two seconds. Click I'm coming on one of them, and check the computer you called from shows who is coming. Then click Resolved.

Switch one computer off, or unplug its network cable. Within a minute, the others should count one device fewer, and if only two computers are in the test, the remaining one turns red. The Admin tab shows when each computer was last heard. Call for help while one is off, resolve it, then turn that computer back on. It should not ring for an alarm that has already been resolved.

Run a test alarm from the Admin tab. The others should show "Test alarm received - no action needed" without the alarm sound. After a minute the report on the Admin tab shows which computers answered.

Try a few things that went wrong for the commercial products: put a computer to sleep and wake it, turn its sound off, or move a laptop to another Wi-Fi network. Note what each status light says.

Rename a computer or change its room from the Admin tab on the admin computer, and check the change appears on the others.

## What to send back

For anything odd, note the time, what you did and what you saw on each computer. The logs help:

- Windows: `C:\ProgramData\Panic Button\logs`
- Linux: `~/.local/share/panic-button/logs`

There is one file per day. They hold connection events and device names, not the content of alarms.

## Known gaps in this version

- The installer is unsigned, so Windows warns about it.
- On Windows, only one account per computer can use Panic Button.
- There is no keyboard shortcut yet, and the button does not remember where you moved it.
- Panic Button does not yet notice a muted computer or turn the volume up, so a muted computer stays silent and still shows green.

## Building the MSI on a Windows PC

If GitHub Actions cannot run, build the MSI on one of the Windows computers. It takes about half an hour the first time.

1. Install [Rust](https://rustup.rs) (accept the offer to install the Visual Studio C++ build tools), [Node.js 24](https://nodejs.org), and [Git](https://git-scm.com).
2. In PowerShell:

   ```powershell
   git clone https://github.com/ohtf/panic-button.git
   cd panic-button\apps\desktop
   npm ci
   npx tauri build --bundles msi
   ```

3. The MSI is in `panic-button\target\release\bundle\msi`. Copy it to the other Windows computers.
