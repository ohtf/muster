// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Sounds, synthesised at run time (`spec/05-desktop-agent.md`, Hold cue
//! and Alarm sounds), so there are no audio files to license. One thread
//! owns the output device, because a device stream is not `Send` on every
//! platform.

use std::f32::consts::TAU;
use std::num::NonZero;
use std::sync::mpsc;
use std::time::Duration;

use muster_core::io::AudioCmd;
use muster_proto::{CueMode, SoundPattern};
use rodio::{Player, Source};

const RATE: u32 = 48_000;

/// What to play.
#[derive(Debug, Clone, Copy)]
pub enum Sound {
    /// The soft hold cue: 500 Hz rising to 800 Hz over the hold, with a
    /// 4 Hz pulse, -30 dBFS rising to -20 dBFS.
    Cue { hold_ms: u32 },
    /// The alarm pattern, looping; 50% rising to 100% over 8 s.
    Alarm(SoundPattern),
    /// The pattern for `seconds`, for "Play test sound".
    Test(SoundPattern, u32),
    /// The 2 s drill chime.
    Chime,
}

/// One tone segment of an alarm pattern: frequency in Hz (0 = silence) and
/// length in ms.
fn pattern(p: SoundPattern) -> &'static [(f32, u32)] {
    match p {
        // B: three quick high beeps, then a pause.
        SoundPattern::B => &[
            (1000.0, 120),
            (0.0, 80),
            (1000.0, 120),
            (0.0, 80),
            (1000.0, 120),
            (0.0, 700),
        ],
        // C: a slow two-note call, low then high.
        SoundPattern::C => &[(520.0, 450), (740.0, 450), (0.0, 500)],
        // A (default): 880 and 660 Hz, 200 ms each, three times, then
        // 600 ms of silence.
        _ => &[
            (880.0, 200),
            (660.0, 200),
            (880.0, 200),
            (660.0, 200),
            (880.0, 200),
            (660.0, 200),
            (0.0, 600),
        ],
    }
}

/// A mono synthesiser for one [`Sound`].
#[derive(Debug)]
struct Synth {
    sound: Sound,
    n: u64,
    phase: f32,
    len: Option<u64>,
}

impl Synth {
    fn new(sound: Sound) -> Self {
        let len = match sound {
            Sound::Cue { hold_ms } => Some(u64::from(hold_ms) * u64::from(RATE) / 1000),
            Sound::Test(_, s) => Some(u64::from(s) * u64::from(RATE)),
            Sound::Chime => Some(2 * u64::from(RATE)),
            Sound::Alarm(_) => None,
        };
        Synth {
            sound,
            n: 0,
            phase: 0.0,
            len,
        }
    }

    fn alarm_sample(&mut self, p: SoundPattern, t_ms: f32) -> (f32, f32) {
        let segs = pattern(p);
        let period: u32 = segs.iter().map(|s| s.1).sum();
        let mut t = t_ms % period as f32;
        for (f, d) in segs {
            let d = *d as f32;
            if t < d {
                // 5 ms ramps at both ends of each tone.
                let ramp = (t / 5.0).min((d - t) / 5.0).clamp(0.0, 1.0);
                return (*f, if *f > 0.0 { ramp } else { 0.0 });
            }
            t -= d;
        }
        (0.0, 0.0)
    }
}

impl Iterator for Synth {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.len.is_some_and(|l| self.n >= l) {
            return None;
        }
        let t = self.n as f32 / RATE as f32;
        let t_ms = t * 1000.0;
        self.n += 1;
        let (freq, amp) = match self.sound {
            Sound::Cue { hold_ms } => {
                let x = (t_ms / hold_ms.max(1) as f32).min(1.0);
                let f = 500.0 + 300.0 * x;
                // -30 dBFS to -20 dBFS, with a gentle 4 Hz pulse.
                let level = 10f32.powf((-30.0 + 10.0 * x) / 20.0);
                let pulse = 0.8 + 0.2 * (TAU * 4.0 * t).sin();
                let fade = (t_ms / 10.0).min(1.0);
                (f, level * pulse * fade)
            }
            Sound::Alarm(p) => {
                let (f, a) = self.alarm_sample(p, t_ms);
                let ramp = 0.5 + 0.5 * (t / 8.0).min(1.0);
                (f, a * ramp * 0.6)
            }
            Sound::Test(p, _) => {
                let (f, a) = self.alarm_sample(p, t_ms);
                (f, a * 0.3)
            }
            Sound::Chime => {
                let decay = (-t * 2.5).exp();
                let f = if t < 0.4 { 660.0 } else { 990.0 };
                (f, 0.12 * decay * (t_ms / 5.0).min(1.0))
            }
        };
        self.phase = (self.phase + TAU * freq / RATE as f32) % TAU;
        Some(amp * self.phase.sin())
    }
}

impl Source for Synth {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        NonZero::<u16>::MIN
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(RATE).unwrap_or(NonZero::<u32>::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        self.len
            .map(|l| Duration::from_millis(l * 1000 / u64::from(RATE)))
    }
}

/// Commands to the audio thread.
#[derive(Debug, Clone, Copy)]
pub enum Cmd {
    Agent(AudioCmd),
    Test(SoundPattern),
}

/// Handle to the audio thread.
#[derive(Debug, Clone)]
pub struct Audio {
    tx: mpsc::Sender<Cmd>,
}

impl Audio {
    /// Starts the audio thread. Returns the handle and whether an output
    /// device opened (reported to the agent as audio status).
    pub fn start() -> (Audio, bool) {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let (ok_tx, ok_rx) = mpsc::channel::<bool>();
        let spawned = std::thread::Builder::new()
            .name("muster-audio".into())
            .spawn(move || run(rx, ok_tx));
        let ok = spawned.is_ok() && ok_rx.recv().unwrap_or(false);
        (Audio { tx }, ok)
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}

fn run(rx: mpsc::Receiver<Cmd>, ok: mpsc::Sender<bool>) {
    let mut sink = match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("no audio output: {e}");
            let _ = ok.send(false);
            // Keep draining so senders never block.
            while rx.recv().is_ok() {}
            return;
        }
    };
    sink.log_on_drop(false);
    let _ = ok.send(true);
    let cue = Player::connect_new(sink.mixer());
    let alarm = Player::connect_new(sink.mixer());
    let other = Player::connect_new(sink.mixer());
    let play = |p: &Player, s: Sound| {
        p.clear();
        p.append(Synth::new(s));
        p.play();
    };
    while let Ok(c) = rx.recv() {
        match c {
            Cmd::Agent(AudioCmd::StartCue { mode, hold_ms }) => match mode {
                CueMode::Silent => {}
                CueMode::AlarmRamp => play(&cue, Sound::Test(SoundPattern::A, hold_ms / 1000 + 1)),
                _ => play(&cue, Sound::Cue { hold_ms }),
            },
            Cmd::Agent(AudioCmd::StopCue) => cue.clear(),
            Cmd::Agent(AudioCmd::StartAlarm { pattern }) => play(&alarm, Sound::Alarm(pattern)),
            Cmd::Agent(AudioCmd::StopAlarm) => alarm.clear(),
            Cmd::Agent(AudioCmd::Chime) => play(&other, Sound::Chime),
            Cmd::Test(p) => play(&other, Sound::Test(p, 3)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(s: Sound, ms: u64) -> f32 {
        Synth::new(s)
            .take((ms * u64::from(RATE) / 1000) as usize)
            .fold(0.0, |m, x| m.max(x.abs()))
    }

    #[test]
    fn cue_is_quiet_and_ends_with_the_hold() {
        let n = Synth::new(Sound::Cue { hold_ms: 1500 }).count() as u64;
        assert_eq!(n, 1500 * u64::from(RATE) / 1000);
        // Never above -20 dBFS.
        assert!(peak(Sound::Cue { hold_ms: 1500 }, 1500) <= 0.101);
    }

    #[test]
    fn alarm_loops_and_ramps_up() {
        let early = peak(Sound::Alarm(SoundPattern::A), 1000);
        let mut s = Synth::new(Sound::Alarm(SoundPattern::A));
        let late = s
            .by_ref()
            .skip(9 * RATE as usize)
            .take(RATE as usize)
            .fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(late > early * 1.5, "{late} > {early}");
        assert!(s.next().is_some(), "alarm loops until stopped");
    }

    #[test]
    fn patterns_differ() {
        assert_ne!(pattern(SoundPattern::A), pattern(SoundPattern::B));
        assert_ne!(pattern(SoundPattern::B), pattern(SoundPattern::C));
    }
}
