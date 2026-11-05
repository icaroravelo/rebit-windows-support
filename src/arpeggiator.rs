//! Allocation-free, sample-clocked chord-to-note arpeggiator.
//! Supports a free sample clock and a host PPQ clock with aligned swing pairs.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArpParameters {
    pub enabled: i32,
    pub pattern: i32,
    pub rate: i32,
    pub octaves: i32,
    pub latch: i32,
    pub retrigger: i32,
    pub seconds: f32,
    pub gate: f32,
    pub swing: f32,
}
impl Default for ArpParameters {
    fn default() -> Self {
        Self {
            enabled: 0,
            pattern: 0,
            rate: 2,
            octaves: 1,
            latch: 0,
            retrigger: 1,
            seconds: 0.125,
            gate: 0.65,
            swing: 0.,
        }
    }
}
fn finite(v: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        default
    }
}
impl ArpParameters {
    pub fn sanitized(mut self) -> Self {
        self.enabled = i32::from(self.enabled != 0);
        self.pattern = self.pattern.clamp(0, 4);
        self.rate = self.rate.clamp(0, 15);
        self.octaves = self.octaves.clamp(1, 4);
        self.latch = i32::from(self.latch != 0);
        self.retrigger = i32::from(self.retrigger != 0);
        self.seconds = finite(self.seconds, 0.01, 2., 0.125);
        self.gate = finite(self.gate, 0.05, 1., 0.65);
        self.swing = finite(self.swing, 0., 0.49, 0.);
        self
    }
    pub fn period_seconds(&self, bpm: f64) -> f64 {
        if self.rate == 15 {
            return self.seconds as f64;
        }
        let rate = self.rate as usize;
        let beats = [1., 0.5, 0.25, 0.125, 0.0625][rate % 5];
        let factor = if rate >= 10 {
            2. / 3.
        } else if rate >= 5 {
            1.5
        } else {
            1.
        };
        beats * factor * 60. / bpm
    }
}
#[derive(Clone, Copy, Default)]
struct Key {
    velocity: u8,
    held: bool,
    sustained: bool,
    sostenuto: bool,
    order: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArpNote {
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
}
#[derive(Clone, Copy, Default, Debug)]
pub struct ArpAction {
    pub off: bool,
    pub on: Option<ArpNote>,
}
#[derive(Clone, Copy)]
struct Candidate {
    note: ArpNote,
    rank: (u64, u64, usize),
}
#[derive(Clone)]
pub struct Arpeggiator {
    pub parameters: ArpParameters,
    keys: [Key; 2048],
    pedals: [bool; 16],
    sostenuto: [bool; 16],
    count: usize,
    order: u64,
    elapsed: f64,
    duration: f64,
    step: u64,
    running: bool,
    gate_open: bool,
    restart: bool,
    last: Option<(u64, u64, usize)>,
    ascending: bool,
    random: u32,
    host_step: Option<i64>,
}
impl Default for Arpeggiator {
    fn default() -> Self {
        Self {
            parameters: ArpParameters::default(),
            keys: [Key::default(); 2048],
            pedals: [false; 16],
            sostenuto: [false; 16],
            count: 0,
            order: 0,
            elapsed: 0.,
            duration: 1.,
            step: 0,
            running: false,
            gate_open: false,
            restart: false,
            last: None,
            ascending: true,
            random: 0x4152_5031,
            host_step: None,
        }
    }
}
impl Arpeggiator {
    pub fn reset(&mut self) {
        self.keys.fill(Key::default());
        self.pedals.fill(false);
        self.sostenuto.fill(false);
        self.count = 0;
        self.order = 0;
        self.elapsed = 0.;
        self.step = 0;
        self.running = false;
        self.gate_open = false;
        self.restart = false;
        self.last = None;
        self.ascending = true;
        self.random = 0x4152_5031;
        self.host_step = None;
    }
    pub fn configure(&mut self, p: ArpParameters, sample_rate: f64, bpm: f64) {
        let p = p.sanitized();
        let old = self.parameters;
        if old.enabled != p.enabled {
            self.reset();
        }
        if old.pattern != p.pattern || old.octaves != p.octaves {
            self.last = None;
            self.ascending = true;
        }
        self.parameters = p;
        if old.latch != 0 && p.latch == 0 {
            for key in &mut self.keys {
                if !key.held && !key.sustained && !key.sostenuto {
                    *key = Key::default();
                }
            }
            self.recount();
        }
        self.clock(sample_rate, bpm);
    }
    pub fn clock(&mut self, sample_rate: f64, bpm: f64) {
        let phase = if self.duration > 0. {
            self.elapsed / self.duration
        } else {
            0.
        };
        let swing = if self.step.is_multiple_of(2) {
            1. + self.parameters.swing as f64
        } else {
            1. - self.parameters.swing as f64
        };
        self.duration = (self.parameters.period_seconds(bpm) * sample_rate * swing).max(1.);
        self.elapsed = phase * self.duration;
    }
    fn recount(&mut self) {
        self.count = self.keys.iter().filter(|k| k.velocity != 0).count();
    }
    pub fn seed(&mut self, channel: usize, note: u8, velocity: u8, order: u64) {
        if channel >= 16 || note > 127 {
            return;
        }

        if self.keys[channel * 128 + note as usize].velocity == 0 && velocity > 0 {
            self.count += 1;
        }
        self.keys[channel * 128 + note as usize] = Key {
            velocity,
            held: true,
            order,
            ..Key::default()
        };
        self.order = self.order.max(order);
        self.restart = true;
    }
    pub fn note_on(&mut self, channel: usize, note: u8, velocity: u8) {
        if channel >= 16 || note > 127 {
            return;
        }

        let replace = self.parameters.latch != 0 && !self.keys.iter().any(|k| k.held);
        if replace {
            self.keys.fill(Key::default());
            self.count = 0;
        }
        if self.count == 0 {
            self.restart = true;
        }
        self.order = self.order.wrapping_add(1);
        self.keys[channel * 128 + note as usize] = Key {
            velocity,
            held: true,
            order: self.order,
            ..Key::default()
        };
        self.recount();
    }
    pub fn note_off(&mut self, channel: usize, note: u8) {
        if channel >= 16 || note > 127 {
            return;
        }

        let key = &mut self.keys[channel * 128 + note as usize];
        key.held = false;
        key.sustained = self.pedals[channel];
        if self.parameters.latch == 0 && !key.sustained && !key.sostenuto {
            *key = Key::default();
        }
        self.recount();
    }
    pub fn controller(&mut self, channel: usize, cc: u8, value: u8) {
        if channel >= 16 {
            return;
        }

        if ![64, 66, 120, 121, 123].contains(&cc) {
            return;
        }
        let keys = &mut self.keys[channel * 128..(channel + 1) * 128];
        match cc {
            64 => {
                self.pedals[channel] = value >= 64;
                if value < 64 {
                    for key in keys {
                        key.sustained = false;
                        if !key.held && !key.sostenuto && self.parameters.latch == 0 {
                            *key = Key::default();
                        }
                    }
                }
            }
            66 => {
                let down = value >= 64;
                if down && !self.sostenuto[channel] {
                    for key in keys {
                        if key.held {
                            key.sostenuto = true;
                        }
                    }
                } else if !down {
                    for key in keys {
                        key.sostenuto = false;
                        if !key.held && !key.sustained && self.parameters.latch == 0 {
                            *key = Key::default();
                        }
                    }
                }
                self.sostenuto[channel] = down;
            }
            120 => keys.fill(Key::default()),
            123 => {
                for key in keys {
                    key.held = false;
                    key.sustained = self.pedals[channel];
                    if !key.sustained && !key.sostenuto {
                        *key = Key::default();
                    }
                }
            }
            121 => {
                self.pedals[channel] = false;
                self.sostenuto[channel] = false;
                for key in keys {
                    key.sustained = false;
                    key.sostenuto = false;
                    if !key.held && self.parameters.latch == 0 {
                        *key = Key::default();
                    }
                }
            }
            _ => (),
        }
        self.recount();
    }
    fn visit(&self, mut f: impl FnMut(Candidate)) {
        for (i, key) in self.keys.iter().enumerate() {
            if key.velocity == 0 {
                continue;
            }
            for octave in 0..self.parameters.octaves as usize {
                let note = i % 128 + octave * 12;
                if note > 127 {
                    continue;
                }
                let id = i * 4 + octave;
                let rank = if self.parameters.pattern == 4 {
                    (octave as u64, key.order, id)
                } else {
                    (note as u64, (i / 128) as u64, id)
                };
                f(Candidate {
                    note: ArpNote {
                        channel: (i / 128) as u8,
                        note: note as u8,
                        velocity: key.velocity,
                    },
                    rank,
                });
            }
        }
    }
    fn select(&mut self) -> Option<ArpNote> {
        if self.parameters.pattern == 3 {
            let mut count = 0;
            self.visit(|_| count += 1);
            if count == 0 {
                return None;
            }
            let mut r = self.random;
            r ^= r << 13;
            r ^= r >> 17;
            r ^= r << 5;
            self.random = r;
            let target = r as usize % count;
            let mut i = 0;
            let mut selected = None;
            self.visit(|c| {
                if i == target {
                    selected = Some(c.note);
                }
                i += 1;
            });
            return selected;
        }
        let ascending = if self.parameters.pattern == 1 {
            false
        } else if self.parameters.pattern == 2 {
            self.ascending
        } else {
            true
        };
        let mut first: Option<Candidate> = None;
        let mut next: Option<Candidate> = None;
        let mut reverse: Option<Candidate> = None;
        self.visit(|c| {
            let better = |old: Option<Candidate>, up: bool| {
                old.is_none_or(|p| if up { c.rank < p.rank } else { c.rank > p.rank })
            };
            if better(first, ascending) {
                first = Some(c);
            }
            if self.last.is_none_or(|last| {
                if ascending {
                    c.rank > last
                } else {
                    c.rank < last
                }
            }) && better(next, ascending)
            {
                next = Some(c);
            }
            if self.last.is_some_and(|last| {
                if ascending {
                    c.rank < last
                } else {
                    c.rank > last
                }
            }) && better(reverse, !ascending)
            {
                reverse = Some(c);
            }
        });
        let selected = if next.is_some() {
            next
        } else if self.parameters.pattern == 2 && reverse.is_some() {
            self.ascending = !ascending;
            reverse
        } else {
            first
        };
        selected.map(|c| {
            self.last = Some(c.rank);
            c.note
        })
    }
    /// Reset clock/pattern only; physical and latched chord state survives.
    pub fn resync_host(&mut self) {
        self.running = false;
        self.gate_open = false;
        self.restart = true;
        self.elapsed = 0.;
        self.step = 0;
        self.last = None;
        self.host_step = None;
        self.ascending = true;
        self.random = 0x4152_5031;
    }
    pub fn tick_host(&mut self, beat: f64, playing: bool) -> ArpAction {
        let mut action = ArpAction::default();
        if !playing || self.count == 0 {
            action.off = self.gate_open;
            self.running = false;
            self.gate_open = false;
            self.host_step = None;
            self.last = None;
            return action;
        }
        if self.restart {
            action.off = self.gate_open;
            self.running = false;
            self.gate_open = false;
            self.last = None;
            self.host_step = None;
            self.ascending = true;
            self.restart = false;
        }
        let beats = self.parameters.period_seconds(60.);
        let (step, elapsed, duration) =
            crate::transport::swung_step(beat, beats, self.parameters.swing as f64);
        let crossed = self.host_step.is_some_and(|last| last != step);
        if !self.running {
            // A chord entered between grid boundaries waits for the next one.
            if crossed || elapsed <= 1e-9 {
                self.running = true;
                action.on = self.select();
                self.gate_open = action.on.is_some();
            }
        } else if crossed {
            action.off =
                self.gate_open && (self.parameters.retrigger != 0 || self.parameters.gate < 1.);
            action.on = self.select();
            self.gate_open = action.on.is_some();
        }
        self.host_step = Some(step);
        if self.gate_open && elapsed + 1e-9 >= duration * self.parameters.gate as f64 {
            // A sub-sample gate already missed by this sample must not leave a new held voice.
            action.on = None;
            action.off = true;
            self.gate_open = false;
        }
        action
    }
    /// Call once before rendering each output sample. All storage is fixed.
    pub fn tick(&mut self, sample_rate: f64, bpm: f64) -> ArpAction {
        let mut action = ArpAction::default();
        if self.count == 0 {
            action.off = self.gate_open;
            self.running = false;
            self.gate_open = false;
            self.last = None;
            self.elapsed = 0.;
            return action;
        }
        if !self.running || self.restart {
            action.off = self.gate_open;
            self.running = true;
            self.restart = false;
            self.elapsed = 0.;
            self.step = 0;
            self.last = None;
            self.ascending = true;
            self.clock(sample_rate, bpm);
            self.elapsed = 0.;
            action.on = self.select();
            self.gate_open = action.on.is_some();
        } else if self.elapsed + 1e-9 >= self.duration {
            self.elapsed -= self.duration;
            self.step = self.step.wrapping_add(1);
            action.off =
                self.gate_open && (self.parameters.retrigger != 0 || self.parameters.gate < 1.);
            // The phase of the next step starts at zero; clock() must not remap the old duration.
            let remainder = self.elapsed;
            self.elapsed = 0.;
            self.clock(sample_rate, bpm);
            self.elapsed = remainder.max(0.);
            action.on = self.select();
            self.gate_open = action.on.is_some();
        } else if self.gate_open
            && self.elapsed + 1e-9 >= self.duration * self.parameters.gate as f64
        {
            action.off = true;
            self.gate_open = false;
        }
        self.elapsed += 1.;
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn arp(pattern: i32) -> Arpeggiator {
        let mut a = Arpeggiator::default();
        a.configure(
            ArpParameters {
                enabled: 1,
                pattern,
                ..ArpParameters::default()
            },
            8000.,
            120.,
        );
        for n in [67, 60, 64] {
            a.note_on(0, n, 100);
        }
        a
    }
    fn notes(a: &mut Arpeggiator, count: usize) -> Vec<u8> {
        let mut result = Vec::new();
        for _ in 0..count * 1000 {
            if let Some(n) = a.tick(8000., 120.).on {
                result.push(n.note);
            }
        }
        result
    }
    #[test]
    fn ordered_patterns() {
        assert_eq!(notes(&mut arp(0), 6), [60, 64, 67, 60, 64, 67]);
        assert_eq!(notes(&mut arp(1), 6), [67, 64, 60, 67, 64, 60]);
        assert_eq!(notes(&mut arp(2), 6), [60, 64, 67, 64, 60, 64]);
        assert_eq!(notes(&mut arp(4), 6), [67, 60, 64, 67, 60, 64]);
    }
    #[test]
    fn octaves_and_extreme_notes() {
        let mut a = arp(0);
        a.configure(
            ArpParameters {
                enabled: 1,
                octaves: 2,
                ..ArpParameters::default()
            },
            8000.,
            120.,
        );
        assert_eq!(notes(&mut a, 7), [60, 64, 67, 72, 76, 79, 60]);
        a.reset();
        a.note_on(15, 127, 127);
        assert_eq!(notes(&mut a, 4), [127; 4]);
    }
    #[test]
    fn random_is_repeatable_and_instance_local() {
        let mut a = arp(3);
        let mut b = arp(3);
        let mut other = arp(3);
        notes(&mut other, 50);
        let left = notes(&mut a, 20);
        assert_eq!(left, notes(&mut b, 20));
        assert!(left.iter().all(|n| [60, 64, 67].contains(n)));
    }
    #[test]
    fn exact_gate_and_swing() {
        let mut a = arp(0);
        a.configure(
            ArpParameters {
                enabled: 1,
                gate: 0.5,
                swing: 0.25,
                ..ArpParameters::default()
            },
            8000.,
            120.,
        );
        let mut on = Vec::new();
        let mut off = Vec::new();
        for i in 0..4000 {
            let action = a.tick(8000., 120.);
            if action.on.is_some() {
                on.push(i);
            }
            if action.off {
                off.push(i);
            }
        }
        assert_eq!(on, [0, 1250, 2000, 3250]);
        assert_eq!(off, [625, 1625, 2625, 3625]);
    }
    #[test]
    fn straight_dotted_triplet_seconds_and_tempo() {
        for (rate, expected) in [(2, 0.125), (7, 0.1875), (12, 1. / 12.), (15, 0.125)] {
            let p = ArpParameters {
                rate,
                ..ArpParameters::default()
            };
            assert!((p.period_seconds(120.) - expected).abs() < 1e-10);
        }
        let mut a = arp(0);
        for _ in 0..500 {
            a.tick(8000., 120.);
        }
        a.clock(8000., 240.);
        for _ in 0..250 {
            assert!(a.tick(8000., 240.).on.is_none());
        }
        assert!(a.tick(8000., 240.).on.is_some());
    }
    #[test]
    fn latch_replaces_a_released_chord_and_clears_when_disabled() {
        let mut a = arp(0);
        let p = ArpParameters {
            enabled: 1,
            latch: 1,
            ..ArpParameters::default()
        };
        a.configure(p, 8000., 120.);
        for n in [60, 64, 67] {
            a.note_off(0, n);
        }
        assert_eq!(notes(&mut a, 3), [60, 64, 67]);
        a.note_on(1, 72, 110);
        assert_eq!(a.tick(8000., 120.).on.unwrap().note, 72);
        a.note_off(1, 72);
        a.configure(ArpParameters { latch: 0, ..p }, 8000., 120.);
        assert!(a.tick(8000., 120.).on.is_none());
        assert_eq!(a.count, 0);
    }
    #[test]
    fn pedals_hold_the_input_chord_but_not_the_gate() {
        let mut a = arp(0);
        for n in [64, 67] {
            a.note_off(0, n);
        }
        a.controller(0, 64, 127);
        a.note_off(0, 60);
        assert_eq!(a.count, 1);
        assert!(a.tick(8000., 120.).on.is_some());
        let mut off = false;
        for _ in 0..700 {
            off |= a.tick(8000., 120.).off;
        }
        assert!(off);
        a.controller(0, 64, 0);
        assert_eq!(a.count, 0);
        a.note_on(0, 60, 100);
        a.controller(0, 66, 127);
        a.note_on(0, 64, 100);
        a.controller(0, 66, 127);
        a.note_off(0, 60);
        a.note_off(0, 64);
        assert_eq!(a.count, 1);
        a.controller(0, 66, 0);
        assert_eq!(a.count, 0);
    }
    #[test]
    fn channel_controls_and_latched_all_notes_off() {
        let mut a = arp(0);
        a.configure(
            ArpParameters {
                enabled: 1,
                latch: 1,
                ..ArpParameters::default()
            },
            8000.,
            120.,
        );
        a.note_on(1, 72, 100);
        a.controller(0, 120, 0);
        assert_eq!(a.count, 1);
        a.controller(1, 123, 0);
        assert_eq!(a.count, 0);
    }
    #[test]
    fn clock_bounds_sanitize_nonfinite_values() {
        let p = ArpParameters {
            rate: 99,
            octaves: 99,
            seconds: f32::NAN,
            gate: f32::INFINITY,
            swing: f32::NAN,
            ..ArpParameters::default()
        }
        .sanitized();
        assert_eq!(p.rate, 15);
        assert_eq!(p.octaves, 4);
        assert_eq!(p.seconds, 0.125);
        assert_eq!(p.gate, 0.65);
        assert_eq!(p.swing, 0.);
    }
}

#[cfg(test)]
mod robustness_tests {
    use super::*;
    #[test]
    fn clocks_at_multiple_sample_rates() {
        for rate in [11025., 44100., 48000., 96000., 192000.] {
            let mut a = Arpeggiator::default();
            a.configure(
                ArpParameters {
                    enabled: 1,
                    ..Default::default()
                },
                rate,
                120.,
            );
            a.note_on(0, 60, 100);
            let mut times = Vec::new();
            for sample in 0..(rate * 0.3) as usize {
                if a.tick(rate, 120.).on.is_some() {
                    times.push(sample);
                }
            }
            assert_eq!(
                times,
                vec![
                    0,
                    (rate * 0.125).ceil() as usize,
                    (rate * 0.25).ceil() as usize
                ]
            );
        }
    }
    #[test]
    fn invalid_public_midi_inputs_are_ignored() {
        let mut a = Arpeggiator::default();
        a.note_on(16, 60, 100);
        a.note_on(0, 255, 100);
        a.seed(99, 60, 100, 1);
        a.note_off(99, 255);
        a.controller(99, 64, 127);
        assert!(a.tick(48000., 120.).on.is_none());
    }
}
