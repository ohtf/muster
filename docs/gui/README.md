# The Muster desktop app

The desktop app (`apps/desktop`) is a Tauri 2 shell with a Svelte 5 frontend around the embedded agent. Rust owns behaviour: the hold timer, delivery and status all come from `muster-core`. The webviews draw four windows:

- **Button**: the floating red button (bottom right), with the status shape and a badge for how many are coming.
- **Panel**: beside the button, the sender panel after you call for help ("Delivered to N devices", who is coming, Cancel, Resolved), and the drill toast.
- **Alert**: the receiver alert, white on deep red, with "I'm coming" and "Seen - can't come".
- **Main**: status, history, settings, admin and about; or first run if the computer is not set up yet.

## Running it

```sh
s/gui-dev
```

This starts Vite and the Tauri shell. On first run you choose **Join an existing practice** (paste an enrolment code) or **Set up a new practice**. Then you pick a room and try a practice hold.

It needs WebKitGTK 4.1 and libsoup 3 on Linux (Debian and Ubuntu: `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev`). Windows needs WebView2, which is normally present.

## Two devices on one computer

To see an alarm go from one device to another without a second computer, give each its own data folder and port, and point them at each other:

```sh
mkdir -p /tmp/muster-demo && cd /tmp/muster-demo
printf '[network]\nport = 47480\nmdns = false\nseed_peers = ["127.0.0.1:47481"]\n' > a.toml
printf '[network]\nport = 47481\nmdns = false\nseed_peers = ["127.0.0.1:47480"]\n[enrolment]\ntoken_file = "/tmp/muster-demo/token"\n' > b.toml

# Device A: the desktop app. Set up a new practice in the window, then in
# Admin > Add a computer, create a code and save it as /tmp/muster-demo/token.
MUSTER_DATA_DIR=/tmp/muster-demo/a MUSTER_CONFIG=/tmp/muster-demo/a.toml \
  MUSTER_CONTROL_SOCKET=/tmp/muster-demo/a.sock s/gui-dev

# Device B: a headless agent standing in for a second computer.
cargo run --release -p muster-agent -- --headless --config /tmp/muster-demo/b.toml \
  --policy /dev/null --data-dir /tmp/muster-demo/b --control-socket /tmp/muster-demo/b.sock \
  --label "Consulting Room 4" --allow-raise

# Raise from B and watch A's alert window; or hold A's button and watch B:
cargo run --release -p muster-cli -- --control-socket /tmp/muster-demo/b.sock ctl raise
cargo run --release -p muster-cli -- --control-socket /tmp/muster-demo/b.sock ctl alarms
```

With `MUSTER_DATA_DIR` set, the single-instance check is off and the tray offers Quit, so you can run several copies.

## Checking it

```sh
s/desktop-check
```

This type-checks and builds the frontend, then runs clippy and the tests for the Tauri crate. `s/test` and `s/lint` leave the desktop crate out because it needs WebKit and a built frontend.

Each view also runs in an ordinary browser against a mock agent, for layout work and screenshots. Run `npm run dev` in `apps/desktop`, then open `http://localhost:1420/?view=alert&mock=alert`. Views: `button`, `panel`, `alert`, `main`. Mock scenarios: `idle`, `alert`, `sender`, `firstrun`, `amber`, `red`.

## Changing the alarm sound

The built-in sounds are in `sounds/` in the repository: `alarm-a.wav`, `alarm-b.wav`, `alarm-c.wav` and `drill-chime.wav`. To use a different sound on a computer, put a WAV file with the same name in the `sounds` folder of Muster's data folder. On Linux that is `~/.local/share/muster/sounds`; Settings shows the exact folder. An alarm file is looped until someone acts, so make it one short cycle. If the file cannot be read, Muster plays the built-in sound instead and logs why.

## Wayland

On Wayland the app cannot place windows or keep them on top; the compositor decides. The status tab says so (`spec/05-desktop-agent.md`, Platform notes). On niri, a window rule keeps the button floating in the corner and on top:

```kdl
window-rule {
    match app-id="muster-desktop" title="^Muster button$"
    open-floating true
    open-focused false
    default-floating-position x=24 y=24 relative-to="bottom-right"
}
window-rule {
    match app-id="muster-desktop" title="^Muster panel$"
    open-floating true
    open-focused false
    default-floating-position x=24 y=110 relative-to="bottom-right"
}
```

## Not done yet

- Join by approval code (the app joins with an enrolment code only).
- The global shortcut, "Move button" position memory, and the per-OS work in R32: Windows audio endpoint and named pipe, session detection, macOS Local Network.
- Practice settings and drill schedules in Admin.
- Installers (M7).
