<!--
SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Try Panic Button without the desktop app

There is no desktop app yet (M4), but the headless agent and the `panic-button` CLI run the real protocol. You can set up a practice on two or more Linux or macOS machines on the same network, raise an alarm on one and see it arrive on the others.

Not on Windows yet: the agent's control API there needs the named pipe (R32).

## 1. Build

On each machine (or build once and copy `panic-button` and `panic-button-agent`):

```sh
cargo build --release -p panic-button-agent -p panic-button
export PATH="$PWD/target/release:$PATH"
```

Allow UDP and TCP port 47474 through any firewall between the machines.

## 2. Create the practice (first machine)

```sh
mkdir -p ~/panic-button-try && cd ~/panic-button-try
printf 'choose a long passphrase\n' > pass
panic-button --data-dir ./state practice init --name "Test Surgery" \
  --recovery-kit ./recovery-kit --passphrase-file ./pass --label "Reception"
panic-button --data-dir ./state token create --days 1 --out ./token
```

Copy `token` to each other machine. It is a secret: anyone holding it can join until it expires.

## 3. Start an agent on each machine

First machine:

```sh
panic-button-agent --headless --data-dir ./state --control-socket ./ctl.sock --allow-raise
```

Each other machine, with the token in `~/panic-button-try/token`:

```sh
cd ~/panic-button-try
printf '[enrolment]\ntoken_file = "%s/token"\n' "$PWD" > config.toml
panic-button-agent --headless --config ./config.toml --data-dir ./state \
  --control-socket ./ctl.sock --label "Room 3" --allow-raise
```

`--allow-raise` lets the CLI raise alarms, standing in for the button. Add `PANIC_BUTTON_LOG=debug` to see the traffic.

## 4. Try it

In another terminal on any machine (`cd ~/panic-button-try` first):

```sh
panic-button --control-socket ./ctl.sock ctl status      # green once the others are heard
panic-button --control-socket ./ctl.sock ctl peers
panic-button --control-socket ./ctl.sock ctl wait-alarm --timeout 60s   # on a receiver
panic-button --control-socket ./ctl.sock ctl raise                      # on the sender
panic-button --control-socket ./ctl.sock ctl alarms
panic-button --control-socket ./ctl.sock ctl respond <alarm id>         # "I'm coming"
panic-button --control-socket ./ctl.sock ctl resolve <alarm id>
```

Things worth trying:

- Stop an agent and watch the others' status change to amber or red, then start it again.
- Pull a network cable or turn Wi-Fi off during an alarm, then reconnect.
- On the first machine (it is the admin), `ctl revoke <device id>` and see that device turn red.
- Wi-Fi with client isolation, or two VLANs: run a relay (`packaging/relay/README.md`) and add `[network]` `relay_urls = ["wss://<host>/v1/ws"]` to each `config.toml`.

## Starting again

Stop the agents and delete `~/panic-button-try`. The `recovery-kit` and passphrase are the only way to recover the practice, so in a real practice they would be stored safely.
