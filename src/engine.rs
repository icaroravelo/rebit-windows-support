//! Fixed-capacity synthesis. Rendering and MIDI handling allocate no memory.
use crate::arpeggiator::{ArpParameters, Arpeggiator};
use crate::sequence::{Cursor, SequenceBank};
use crate::transport::{ClockParameters, TransportInfo};
use std::f64::consts::TAU;

pub const MAX_VOICES: usize = 64;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub osc: i32,
    pub duty: i32,
    pub noise_algorithm: i32,
    pub max_poly: i32,
    pub behavior: i32,
    pub interval_type: i32,
    pub pitch_mode: i32,
    pub gain: f32,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub portamento: f32,
    pub bend_range: f32,
    pub vibrato_period: f32,
    pub vibrato_depth: f32,
    pub vibrato_delay: f32,
    pub sweep_initial: f32,
    pub sweep_time: f32,
    pub interval: f32,
    pub ignores_wheel: i32,
    pub volume_enabled: i32,
    pub pitch_enabled: i32,
    pub duty_enabled: i32,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            osc: 0,
            duty: 0,
            noise_algorithm: 0,
            max_poly: 8,
            behavior: 0,
            interval_type: 0,
            pitch_mode: 0,
            gain: 0.5,
            attack: 0.0,
            decay: 0.0,
            sustain: 1.0,
            release: 0.0,
            portamento: 0.0,
            bend_range: 2.0,
            vibrato_period: 0.15,
            vibrato_depth: 0.0,
            vibrato_delay: 0.3,
            sweep_initial: 0.0,
            sweep_time: 0.1,
            interval: 0.1,
            ignores_wheel: 1,
            volume_enabled: 0,
            pitch_enabled: 0,
            duty_enabled: 0,
        }
    }
}
fn bounded(v: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        fallback
    }
}
impl Parameters {
    fn sanitized(mut self) -> Self {
        self.osc = self.osc.clamp(0, 2);
        self.duty = self.duty.clamp(0, 2);
        self.noise_algorithm = self.noise_algorithm.clamp(0, 2);
        self.max_poly = self.max_poly.clamp(1, 64);
        self.behavior = self.behavior.clamp(0, 3);
        self.interval_type = self.interval_type.clamp(0, 8);
        self.pitch_mode = self.pitch_mode.clamp(0, 1);
        self.gain = bounded(self.gain, 0., 1., 0.5);
        self.attack = bounded(self.attack, 0., 5., 0.);
        self.decay = bounded(self.decay, 0., 5., 0.);
        self.sustain = bounded(self.sustain, 0., 1., 1.);
        self.release = bounded(self.release, 0., 5., 0.);
        self.portamento = bounded(self.portamento, 0., 1., 0.);
        self.bend_range = bounded(self.bend_range, 0., 24., 2.);
        self.vibrato_period = bounded(self.vibrato_period, 0.01, 1., 0.15);
        self.vibrato_depth = bounded(self.vibrato_depth, 0., 2., 0.);
        self.vibrato_delay = bounded(self.vibrato_delay, 0., 1., 0.3);
        self.sweep_initial = bounded(self.sweep_initial, -24., 24., 0.);
        self.sweep_time = bounded(self.sweep_time, 0.01, 5., 0.1);
        self.interval = bounded(self.interval, 0.001, 0.3, 0.1);
        self
    }
}
#[derive(Clone, Copy)]
struct Voice {
    active: bool,
    held: bool,
    sustained: bool,
    sostenuto: bool,
    note: u8,
    channel: u8,
    velocity: f32,
    age: u64,
    phase: f64,
    increment: f64,
    level: f64,
    stage: u8,
    release_step: f64,
    time: f64,
    frame: f64,
    glide: f64,
    glide_step: f64,
    noise: f32,
    rng: u32,
    lfsr: u32,
    cursors: [Cursor; 3],
    arpeggiated: bool,
    sequence_tick: Option<i64>,
}
impl Default for Voice {
    fn default() -> Self {
        Self {
            active: false,
            held: false,
            sustained: false,
            sostenuto: false,
            note: 0,
            channel: 0,
            velocity: 0.,
            age: 0,
            phase: 0.,
            increment: 0.,
            level: 0.,
            stage: 0,
            release_step: 1.,
            time: 0.,
            frame: 0.,
            glide: 0.,
            glide_step: 0.,
            noise: 0.,
            rng: 1,
            lfsr: 0x7fff,
            cursors: [Cursor::default(); 3],
            arpeggiated: false,
            sequence_tick: None,
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Key {
    order: u64,
    velocity: u8,
}
pub struct Engine {
    params: Parameters,
    voices: [Voice; MAX_VOICES],
    keys: [[Key; 128]; 16],
    bends: [f64; 16],
    wheels: [f64; 16],
    sustain: [bool; 16],
    sample_rate: f64,
    tempo: f64,
    age: u64,
    seed: u32,
    arp_elapsed: f64,
    arp: Arpeggiator,
    arp_voice: Option<usize>,
    clock: ClockParameters,
    transport: TransportInfo,
    transport_frames: u64,
    pub(crate) sequences: Box<SequenceBank>,
}
impl Default for Engine {
    fn default() -> Self {
        Self::new(44_100., 0x5242_4954)
    }
}
impl Engine {
    pub fn new(sample_rate: f64, seed: u32) -> Self {
        let mut engine = Self {
            params: Parameters::default(),
            voices: [Voice::default(); MAX_VOICES],
            keys: [[Key::default(); 128]; 16],
            bends: [0.; 16],
            wheels: [0.; 16],
            sustain: [false; 16],
            sample_rate: 44_100.,
            tempo: 120.,
            age: 0,
            seed: seed.max(1),
            arp_elapsed: 0.,
            arp: Arpeggiator::default(),
            arp_voice: None,
            clock: ClockParameters::default(),
            transport: TransportInfo::default(),
            transport_frames: 0,
            sequences: Box::default(),
        };
        engine.prepare(sample_rate);
        engine
    }
    pub fn prepare(&mut self, sample_rate: f64) {
        self.sample_rate = if sample_rate.is_finite() {
            sample_rate.clamp(8000., 384_000.)
        } else {
            44_100.
        };
        self.reset();
    }
    pub fn reset(&mut self) {
        self.voices.fill(Voice::default());
        self.keys.fill([Key::default(); 128]);
        self.bends.fill(0.);
        self.wheels.fill(0.);
        self.sustain.fill(false);
        self.age = 0;
        self.arp_elapsed = 0.;
        self.arp.reset();
        self.arp_voice = None;
        self.transport = TransportInfo::default();
        self.transport_frames = 0;
    }
    pub fn set_parameters(&mut self, params: Parameters) {
        let p = params.sanitized();
        if p == self.params {
            return;
        }
        if p.max_poly != self.params.max_poly || p.behavior != self.params.behavior {
            if self.arp.parameters.enabled == 0 {
                self.reset();
            } else {
                for v in &mut self.voices[p.max_poly as usize..] {
                    v.active = false;
                }
                if self.arp_voice.is_some_and(|i| i >= p.max_poly as usize) {
                    self.arp_voice = None;
                }
            }
        }
        self.params = p;
        for v in &mut self.voices {
            if v.active {
                v.increment = pitch(
                    v,
                    &p,
                    &self.sequences,
                    self.sample_rate,
                    &self.bends,
                    &self.wheels,
                );
            }
        }
    }
    pub fn parameters(&self) -> Parameters {
        self.params
    }
    pub fn set_tempo(&mut self, bpm: f64) {
        if bpm.is_finite() {
            let next = bpm.clamp(1., 999.);
            if next != self.tempo {
                if self.clock.sequence_rate > 0 {
                    let ratio = self.tempo / next;
                    for voice in &mut self.voices {
                        voice.frame *= ratio;
                    }
                }
                self.tempo = next;
                self.arp.clock(self.sample_rate, self.tempo);
            }
        }
    }
    pub fn set_clock_parameters(&mut self, parameters: ClockParameters) {
        let next = parameters.sanitized();
        if self.clock == next {
            return;
        }
        let host_changed = self.clock.host_sync != next.host_sync;
        self.clock = next;
        if host_changed && self.arp.parameters.rate != 15 {
            self.resync_arp();
        }
        for voice in &mut self.voices {
            voice.sequence_tick = None;
            voice.frame = 0.;
        }
    }
    pub fn clock_parameters(&self) -> ClockParameters {
        self.clock
    }
    fn host_arp(&self) -> bool {
        self.clock.host_sync != 0 && self.transport.valid != 0 && self.arp.parameters.rate != 15
    }
    fn resync_arp(&mut self) {
        // Stop all generated tails at a timeline discontinuity. Keep the input chord.
        for voice in &mut self.voices {
            if voice.arpeggiated {
                voice.active = false;
            }
        }
        self.arp_voice = None;
        self.arp.resync_host();
    }
    fn resync_timeline(&mut self) {
        if self.clock.host_sync == 0 {
            return;
        }
        if self.arp.parameters.rate != 15 {
            self.resync_arp();
        }
        if self.clock.sequence_rate > 0 {
            for voice in &mut self.voices {
                voice.sequence_tick = None;
                voice.frame = 0.;
                if voice.active && voice.stage != 3 {
                    voice.cursors.fill(Cursor::default());
                    voice.increment = pitch(
                        voice,
                        &self.params,
                        &self.sequences,
                        self.sample_rate,
                        &self.bends,
                        &self.wheels,
                    );
                }
            }
        }
    }
    /// One call at the beginning of every host callback, before sample-offset MIDI.
    pub fn set_transport(&mut self, info: TransportInfo) {
        let next = info.sanitized();
        let old = self.transport;
        let ramp_allowance =
            (next.bpm - old.bpm).abs() * self.transport_frames as f64 / (60. * self.sample_rate);
        let tolerance = (self.tempo / (60. * self.sample_rate) * 4.).max(1e-6) + ramp_allowance;
        self.transport_frames = 0;
        let discontinuity = old.valid != next.valid
            || (next.valid != 0
                && (old.playing != next.playing
                    || (next.playing != 0 && (next.ppq - old.ppq).abs() > tolerance)));
        self.transport = next;
        self.set_tempo(next.bpm);
        if discontinuity {
            self.resync_timeline();
        }
    }
    pub fn clock_status(&self) -> i32 {
        if self.clock.host_sync == 0
            || (self.arp.parameters.rate == 15 && self.clock.sequence_rate == 0)
        {
            0
        } else if self.transport.valid == 0 {
            3
        } else if self.transport.playing == 0 {
            1
        } else {
            2
        }
    }
    pub fn set_arp_parameters(&mut self, parameters: ArpParameters) {
        let next = parameters.sanitized();
        let old = self.arp.parameters;
        if old == next {
            return;
        }
        if old.enabled != next.enabled {
            for voice in &mut self.voices {
                if voice.active {
                    voice.held = false;
                    voice.sustained = false;
                    voice.sostenuto = false;
                    release(voice, &self.params, &self.sequences, self.sample_rate);
                }
            }
            self.arp_voice = None;
        }
        self.arp.configure(next, self.sample_rate, self.tempo);
        if self.clock.host_sync != 0
            && self.transport.valid != 0
            && (old.rate != next.rate || old.swing != next.swing)
        {
            self.resync_arp();
        }
        if old.enabled == 0 && next.enabled != 0 {
            for ch in 0..16 {
                for note in 0..128 {
                    let key = self.keys[ch][note];
                    if key.velocity > 0 {
                        self.arp.seed(ch, note as u8, key.velocity, key.order);
                    }
                }
            }
        } else if old.enabled != 0 && next.enabled == 0 {
            for ch in 0..16 {
                for note in 0..128 {
                    let velocity = self.keys[ch][note].velocity;
                    if velocity > 0 {
                        self.note_on(ch, note as u8, velocity);
                    }
                }
            }
        }
    }
    pub fn arp_parameters(&self) -> ArpParameters {
        self.arp.parameters
    }
    fn arp_release_voice(&mut self) {
        if let Some(slot) = self.arp_voice.take() {
            let v = &mut self.voices[slot];
            if v.active && v.arpeggiated {
                v.held = false;
                v.sustained = false;
                v.sostenuto = false;
                release(v, &self.params, &self.sequences, self.sample_rate);
            }
        }
    }
    fn independent_arp_tick(&mut self) {
        let action = if self.host_arp() {
            self.arp
                .tick_host(self.transport.ppq, self.transport.playing != 0)
        } else {
            self.arp.tick(self.sample_rate, self.tempo)
        };
        if action.off {
            self.arp_release_voice();
        }
        if let Some(note) = action.on {
            if self.arp.parameters.retrigger == 0 {
                if let Some(slot) = self.arp_voice {
                    let v = &mut self.voices[slot];
                    if v.active && v.stage != 3 {
                        v.note = note.note;
                        v.channel = note.channel;
                        v.velocity = note.velocity as f32 / 127.;
                        v.increment = pitch(
                            v,
                            &self.params,
                            &self.sequences,
                            self.sample_rate,
                            &self.bends,
                            &self.wheels,
                        );
                        return;
                    }
                }
            }
            self.age = self.age.wrapping_add(1);
            self.arp_voice =
                Some(self.spawn_voice(note.channel as usize, note.note, note.velocity, true));
        }
    }
    /// Control-thread convenience. The native callback instead uses the FFI pointer swap,
    /// returning the old bank for destruction on the control thread.
    pub fn set_sequences(&mut self, sequences: SequenceBank) {
        *self.sequences = sequences;
        self.reset_cursors();
    }
    pub(crate) fn reset_cursors(&mut self) {
        for v in &mut self.voices {
            v.cursors.fill(Cursor::default());
            v.sequence_tick = None;
            v.frame = 0.;
            if !v.held && !v.sustained && !v.sostenuto {
                for (c, s) in v.cursors.iter_mut().zip(&self.sequences.0) {
                    c.release(s);
                }
            }
            if v.active {
                v.increment = pitch(
                    v,
                    &self.params,
                    &self.sequences,
                    self.sample_rate,
                    &self.bends,
                    &self.wheels,
                );
            }
        }
    }
    /// Latest active voice is the representative playhead for polyphonic notes.
    pub fn sequence_positions(&self) -> [i32; 3] {
        let mut positions = [-1; 3];
        if let Some(voice) = self
            .voices
            .iter()
            .filter(|v| v.active)
            .max_by_key(|v| v.age)
        {
            let enabled = [
                self.params.volume_enabled,
                self.params.pitch_enabled,
                self.params.duty_enabled,
            ];
            for lane in 0..3 {
                if enabled[lane] != 0 {
                    positions[lane] = voice.cursors[lane]
                        .position(&self.sequences.0[lane])
                        .map_or(-1, |p| p as i32);
                }
            }
        }
        positions
    }
    pub(crate) fn rebind_sequences(&mut self, old: &SequenceBank) {
        for voice in &mut self.voices {
            for lane in 0..3 {
                voice.cursors[lane].rebind(&old.0[lane], &self.sequences.0[lane]);
            }
            if voice.active {
                voice.increment = pitch(
                    voice,
                    &self.params,
                    &self.sequences,
                    self.sample_rate,
                    &self.bends,
                    &self.wheels,
                );
            }
        }
    }
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }
    pub fn midi(&mut self, status: u8, data1: u8, data2: u8) {
        if data1 > 127 || data2 > 127 {
            return;
        }
        let ch = (status & 15) as usize;
        if self.arp.parameters.enabled != 0 {
            match status & 0xf0 {
                0x90 if data2 != 0 => {
                    self.age = self.age.wrapping_add(1);
                    self.keys[ch][data1 as usize] = Key {
                        order: self.age,
                        velocity: data2,
                    };
                    self.arp.note_on(ch, data1, data2);
                    return;
                }
                0x80 | 0x90 => {
                    self.keys[ch][data1 as usize] = Key::default();
                    self.arp.note_off(ch, data1);
                    return;
                }
                0xb0 => self.arp.controller(ch, data1, data2),
                _ => (),
            }
        }
        match status & 0xf0 {
            0x90 if data2 != 0 => self.note_on(ch, data1, data2),
            0x80 | 0x90 => self.note_off(ch, data1),
            0xe0 => {
                self.bends[ch] = ((data1 as i32 | ((data2 as i32) << 7)) - 8192) as f64 / 8192.;
                self.retune(ch);
            }
            0xb0 => self.controller(ch, data1, data2),
            _ => (),
        }
        self.retune(ch);
    }
    fn retune(&mut self, ch: usize) {
        for v in &mut self.voices {
            if v.active && v.channel as usize == ch {
                v.increment = pitch(
                    v,
                    &self.params,
                    &self.sequences,
                    self.sample_rate,
                    &self.bends,
                    &self.wheels,
                );
            }
        }
    }
    fn note_on(&mut self, ch: usize, note: u8, velocity: u8) {
        self.age = self.age.wrapping_add(1);
        self.keys[ch][note as usize] = Key {
            order: self.age,
            velocity,
        };
        if self.params.max_poly == 1
            && self.params.behavior != 3
            && self.voices[0].active
            && self.voices[0].held
        {
            if self.params.behavior == 0 {
                self.retarget(ch, note, velocity);
            }
            return;
        }
        for v in &mut self.voices {
            if v.active && v.channel as usize == ch && v.note == note {
                v.held = false;
                release(v, &self.params, &self.sequences, self.sample_rate);
            }
        }
        self.spawn_voice(ch, note, velocity, false);
        if self.params.max_poly == 1 {
            self.arp_elapsed = 0.;
        }
    }
    fn spawn_voice(&mut self, ch: usize, note: u8, velocity: u8, arpeggiated: bool) -> usize {
        let limit = self.params.max_poly as usize;
        let slot = self.voices[..limit]
            .iter()
            .position(|v| !v.active)
            .unwrap_or_else(|| {
                self.voices[..limit]
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| (v.held || v.sustained || v.sostenuto, v.age))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });
        let seed = self.seed ^ (slot as u32 + 1).wrapping_mul(0x9e37_79b9) ^ self.age as u32;
        let mut v = Voice {
            active: true,
            held: true,
            note,
            channel: ch as u8,
            velocity: velocity as f32 / 127.,
            age: self.age,
            rng: seed.max(1),
            ..Voice::default()
        };
        v.noise = next_noise(&mut v, self.params.noise_algorithm);
        v.increment = pitch(
            &v,
            &self.params,
            &self.sequences,
            self.sample_rate,
            &self.bends,
            &self.wheels,
        );
        v.arpeggiated = arpeggiated;
        self.voices[slot] = v;
        slot
    }
    fn retarget(&mut self, ch: usize, note: u8, velocity: u8) {
        let v = &mut self.voices[0];
        let previous = v.note as f64 + v.glide;
        v.channel = ch as u8;
        v.note = note;
        v.velocity = velocity as f32 / 127.;
        v.held = true;
        v.sustained = false;
        v.sostenuto = false;
        v.glide = if self.params.portamento > 0. {
            previous - note as f64
        } else {
            0.
        };
        v.glide_step = if self.params.portamento > 0. {
            -v.glide / (self.sample_rate * self.params.portamento as f64)
        } else {
            0.
        };
        v.increment = pitch(
            v,
            &self.params,
            &self.sequences,
            self.sample_rate,
            &self.bends,
            &self.wheels,
        );
    }
    fn newest_key(&self) -> Option<(usize, u8, u8)> {
        let mut newest = None;
        let mut order = 0;
        for (ch, keys) in self.keys.iter().enumerate() {
            for (note, key) in keys.iter().enumerate() {
                if key.velocity > 0 && key.order >= order {
                    order = key.order;
                    newest = Some((ch, note as u8, key.velocity));
                }
            }
        }
        newest
    }
    fn note_off(&mut self, ch: usize, note: u8) {
        self.keys[ch][note as usize] = Key::default();
        if self.params.max_poly == 1 && self.params.behavior != 3 {
            let matches = self.voices[0].channel as usize == ch && self.voices[0].note == note;
            if matches || self.params.behavior == 1 || self.params.behavior == 2 {
                if let Some((c, n, vel)) = self.newest_key() {
                    if self.params.behavior == 0 || matches {
                        self.retarget(c, n, vel);
                    }
                    return;
                }
                let v = &mut self.voices[0];
                v.held = false;
                v.sustained = self.sustain[v.channel as usize];
                if !v.sustained && !v.sostenuto {
                    release(v, &self.params, &self.sequences, self.sample_rate);
                }
            }
            return;
        }
        for v in &mut self.voices {
            if v.active && v.channel as usize == ch && v.note == note {
                v.held = false;
                v.sustained = self.sustain[ch];
                if !v.sustained && !v.sostenuto {
                    release(v, &self.params, &self.sequences, self.sample_rate);
                }
            }
        }
    }
    fn controller(&mut self, ch: usize, cc: u8, value: u8) {
        match cc {
            1 => {
                self.wheels[ch] = value as f64 / 127.;
                self.retune(ch);
            }
            64 => {
                self.sustain[ch] = value >= 64;
                if value < 64 {
                    for v in &mut self.voices {
                        if v.active && !v.arpeggiated && v.channel as usize == ch {
                            v.sustained = false;
                            if !v.held && !v.sostenuto {
                                release(v, &self.params, &self.sequences, self.sample_rate);
                            }
                        }
                    }
                }
            }
            66 => {
                for v in &mut self.voices {
                    if v.active && !v.arpeggiated && v.channel as usize == ch {
                        if value >= 64 {
                            if v.held {
                                v.sostenuto = true;
                            }
                        } else {
                            v.sostenuto = false;
                            if !v.held && !v.sustained {
                                release(v, &self.params, &self.sequences, self.sample_rate);
                            }
                        }
                    }
                }
            }
            120 => {
                self.keys[ch].fill(Key::default());
                for v in &mut self.voices {
                    if v.channel as usize == ch {
                        v.active = false;
                    }
                }
            }
            123 => {
                self.keys[ch].fill(Key::default());
                for v in &mut self.voices {
                    if v.active && !v.arpeggiated && v.channel as usize == ch {
                        v.held = false;
                        v.sustained = self.sustain[ch];
                        if !v.sustained && !v.sostenuto {
                            release(v, &self.params, &self.sequences, self.sample_rate);
                        }
                    }
                }
            }
            121 => {
                self.bends[ch] = 0.;
                self.wheels[ch] = 0.;
                self.sustain[ch] = false;
                for v in &mut self.voices {
                    if v.active && v.channel as usize == ch {
                        v.sustained = false;
                        v.sostenuto = false;
                        if !v.held {
                            release(v, &self.params, &self.sequences, self.sample_rate);
                        }
                    }
                }
                self.retune(ch);
            }
            _ => (),
        }
    }
    fn arp_tick(&mut self) {
        if self.params.max_poly != 1
            || !(1..=2).contains(&self.params.behavior)
            || !self.voices[0].held
        {
            return;
        }
        let p = self.params;
        let period = match p.interval_type {
            0..=2 => (p.interval_type + 1) as f64 / 60.,
            8 => p.interval as f64,
            n => 240. / (self.tempo * [96., 64., 48., 32., 24.][(n - 3) as usize]),
        };
        self.arp_elapsed += 1. / self.sample_rate;
        if self.arp_elapsed < period {
            return;
        }
        self.arp_elapsed %= period;
        let current = self.voices[0].note;
        let ascending = p.behavior == 1;
        let mut next: Option<(usize, u8, u8)> = None;
        let mut wrap: Option<(usize, u8, u8)> = None;
        for (ch, keys) in self.keys.iter().enumerate() {
            for (n, key) in keys.iter().enumerate() {
                if key.velocity == 0 {
                    continue;
                }
                let n = n as u8;
                let better = |old: Option<(usize, u8, u8)>| {
                    old.is_none_or(|(_, prev, _)| if ascending { n < prev } else { n > prev })
                };
                if better(wrap) {
                    wrap = Some((ch, n, key.velocity));
                }
                if (if ascending { n > current } else { n < current }) && better(next) {
                    next = Some((ch, n, key.velocity));
                }
            }
        }
        if let Some((ch, note, vel)) = next.or(wrap) {
            self.retarget(ch, note, vel);
        }
    }
    pub fn render(&mut self, output: &mut [f32]) {
        let dt = 1. / self.sample_rate;
        let p = self.params;
        let sequence_beats = self.clock.sequence_beats();
        let sequence_period = sequence_beats.map_or(1. / 60., |b| b * 60. / self.tempo);
        let host_sequences = sequence_beats.is_some()
            && self.clock.host_sync != 0
            && self.transport.valid != 0
            && self.transport.playing != 0;
        let beat_increment = self.tempo / (60. * self.sample_rate);
        for out in output {
            if self.transport.valid != 0
                && self.transport.playing != 0
                && self.transport.looping != 0
                && self.transport.ppq + 1e-9 >= self.transport.loop_end
            {
                let length = self.transport.loop_end - self.transport.loop_start;
                self.transport.ppq = self.transport.loop_start
                    + (self.transport.ppq - self.transport.loop_end)
                        .max(0.)
                        .rem_euclid(length);
                self.resync_timeline();
            }
            let host_step =
                sequence_beats.map(|b| ((self.transport.ppq + 1e-9) / b).floor() as i64);
            if self.arp.parameters.enabled != 0 {
                self.independent_arp_tick();
            } else {
                self.arp_tick();
            }
            let mut sum = 0.;
            for v in &mut self.voices[..p.max_poly as usize] {
                if !v.active {
                    continue;
                }
                let sequence_advance = if host_sequences {
                    let previous = v.sequence_tick;
                    v.sequence_tick = host_step;
                    previous.is_some() && previous != host_step
                } else if sequence_beats.is_some() {
                    v.sequence_tick = None;
                    if v.frame + 1e-12 >= sequence_period {
                        v.frame -= sequence_period;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if sequence_advance {
                    for (cursor, sequence) in v.cursors.iter_mut().zip(&self.sequences.0) {
                        cursor.advance(sequence);
                    }
                    v.increment = pitch(
                        v,
                        &p,
                        &self.sequences,
                        self.sample_rate,
                        &self.bends,
                        &self.wheels,
                    );
                }
                if p.volume_enabled != 0 {
                    v.level = v.cursors[0].value(&self.sequences.0[0]).unwrap_or(0) as f64 / 15.;
                    if v.stage == 3 && v.cursors[0].value(&self.sequences.0[0]).is_none() {
                        v.active = false;
                        continue;
                    }
                } else {
                    match v.stage {
                        0 => {
                            v.level += if p.attack == 0. {
                                1.
                            } else {
                                dt / p.attack as f64
                            };
                            if v.level >= 1. {
                                v.level = 1.;
                                v.stage = 1;
                            }
                        }
                        1 => {
                            v.level -= if p.decay == 0. {
                                1.
                            } else {
                                dt * (1. - p.sustain as f64) / p.decay as f64
                            };
                            if v.level <= p.sustain as f64 {
                                v.level = p.sustain as f64;
                                v.stage = 2;
                            }
                        }
                        2 => v.level = p.sustain as f64,
                        _ => {
                            v.level -= v.release_step;
                            if v.level <= 0. {
                                v.level = 0.;
                                v.active = false;
                            }
                        }
                    }
                }
                let duty = if p.duty_enabled != 0 {
                    v.cursors[2]
                        .value(&self.sequences.0[2])
                        .unwrap_or(p.duty as i16) as usize
                } else {
                    p.duty as usize
                };
                let raw = match p.osc {
                    0 => {
                        if v.phase < [0.125, 0.25, 0.5][duty.min(2)] {
                            -1.
                        } else {
                            1.
                        }
                    }
                    1 => ((1. - 4. * (v.phase - 0.5).abs()) * 7.5).round() as f32 / 7.5,
                    _ => v.noise,
                };
                sum += raw * v.level as f32 * v.velocity * p.gain;
                let dynamic = v.glide != 0.
                    || (p.sweep_initial != 0. && v.time < p.sweep_time as f64)
                    || (p.vibrato_depth > 0. && v.time >= p.vibrato_delay as f64);
                if dynamic {
                    v.increment = pitch(
                        v,
                        &p,
                        &self.sequences,
                        self.sample_rate,
                        &self.bends,
                        &self.wheels,
                    );
                }
                let old_step = (v.phase * if p.noise_algorithm == 0 { 50. } else { 16. }) as u32;
                let next = v.phase + v.increment;
                v.phase = next.fract();
                let new_step = (v.phase * if p.noise_algorithm == 0 { 50. } else { 16. }) as u32;
                if p.osc == 2 && (old_step != new_step || next >= 1.) {
                    v.noise = next_noise(v, p.noise_algorithm);
                }
                let sweep_ended = p.sweep_initial != 0.
                    && v.time < p.sweep_time as f64
                    && v.time + dt >= p.sweep_time as f64;
                v.time += dt;
                if !host_sequences {
                    v.frame += dt;
                }
                let mut glide_ended = false;
                if v.glide != 0. {
                    let next = v.glide + v.glide_step;
                    if next.signum() != v.glide.signum() || next == 0. {
                        v.glide = 0.;
                        glide_ended = true;
                    } else {
                        v.glide = next;
                    }
                }
                if sweep_ended || glide_ended {
                    v.increment = pitch(
                        v,
                        &p,
                        &self.sequences,
                        self.sample_rate,
                        &self.bends,
                        &self.wheels,
                    );
                }
                if sequence_beats.is_none() && v.frame >= 1. / 60. {
                    v.frame -= 1. / 60.;
                    for (cursor, sequence) in v.cursors.iter_mut().zip(&self.sequences.0) {
                        cursor.advance(sequence);
                    }
                    v.increment = pitch(
                        v,
                        &p,
                        &self.sequences,
                        self.sample_rate,
                        &self.bends,
                        &self.wheels,
                    );
                }
            }
            *out = if sum.is_finite() { sum } else { 0. };
            if self.transport.valid != 0 && self.transport.playing != 0 {
                self.transport.ppq += beat_increment;
                self.transport_frames = self.transport_frames.saturating_add(1);
            }
        }
    }
}
fn release(v: &mut Voice, p: &Parameters, bank: &SequenceBank, sample_rate: f64) {
    if v.stage == 3 {
        return;
    }
    v.stage = 3;
    v.sequence_tick = None;
    v.release_step = if p.release == 0. {
        1.
    } else {
        v.level / (sample_rate * p.release as f64)
    };
    for (cursor, seq) in v.cursors.iter_mut().zip(&bank.0) {
        cursor.release(seq);
    }
}
fn pitch(
    v: &Voice,
    p: &Parameters,
    bank: &SequenceBank,
    sample_rate: f64,
    bends: &[f64; 16],
    wheels: &[f64; 16],
) -> f64 {
    let ch = v.channel as usize;
    let sweep = p.sweep_initial as f64 * (1. - v.time / p.sweep_time as f64).max(0.);
    let vib = if v.time >= p.vibrato_delay as f64 {
        ((v.time - p.vibrato_delay as f64) / p.vibrato_period as f64 * TAU).sin()
            * p.vibrato_depth as f64
            * if p.ignores_wheel != 0 { 1. } else { wheels[ch] }
    } else {
        0.
    };
    let offset = if p.pitch_enabled != 0 {
        v.cursors[1].value(&bank.0[1]).unwrap_or(0) as f64
            * if p.pitch_mode == 0 { 1. } else { 0.125 }
    } else {
        0.
    };
    let semitones =
        v.note as f64 + offset + v.glide + bends[ch] * p.bend_range as f64 + sweep + vib;
    (440. * ((semitones - 69.) / 12.).exp2() / sample_rate).min(0.49)
}
fn next_noise(v: &mut Voice, algorithm: i32) -> f32 {
    if algorithm == 0 {
        let mut x = v.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.rng = x;
        ((x & 15) as f32 - 7.5) / 8.
    } else {
        let bit = (v.lfsr ^ (v.lfsr >> if algorithm == 1 { 1 } else { 6 })) & 1;
        v.lfsr = (v.lfsr >> 1) | (bit << 14);
        if v.lfsr & 1 == 0 {
            -0.5
        } else {
            0.5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mono_returns_to_previous_held_note() {
        let mut e = Engine::default();
        e.set_parameters(Parameters {
            max_poly: 1,
            ..Parameters::default()
        });
        e.midi(0x90, 60, 127);
        e.render(&mut [0.; 32]);
        e.midi(0x90, 67, 127);
        assert_eq!(e.voices[0].note, 67);
        e.midi(0x80, 67, 0);
        assert_eq!(e.voices[0].note, 60);
        assert!(e.voices[0].held);
        e.midi(0x80, 60, 0);
        e.render(&mut [0.; 32]);
        assert_eq!(e.active_voices(), 0);
    }
    #[test]
    fn ninety_sixth_arpeggio_clock_and_live_tempo() {
        let mut e = Engine::new(48_000., 1);
        e.set_parameters(Parameters {
            max_poly: 1,
            behavior: 1,
            interval_type: 3,
            ..Parameters::default()
        });
        e.midi(0x90, 60, 127);
        e.midi(0x90, 67, 127);
        e.render(&mut [0.; 1001]);
        assert_eq!(e.voices[0].note, 67);
        e.set_tempo(240.);
        e.render(&mut [0.; 501]);
        assert_eq!(e.voices[0].note, 60);
    }
    #[test]
    fn pitch_and_duty_release_apply_on_note_off() {
        let mut e = Engine::default();
        e.set_parameters(Parameters {
            pitch_enabled: 1,
            duty_enabled: 1,
            release: 1.,
            ..Parameters::default()
        });
        let mut bank = SequenceBank::default();
        bank.0[1] = crate::Sequence::parse("0|12", -64, 63).unwrap();
        bank.0[2] = crate::Sequence::parse("0|2", 0, 2).unwrap();
        e.set_sequences(bank);
        e.midi(0x90, 60, 127);
        e.render(&mut [0.; 32]);
        let before = e.voices[0].increment;
        e.midi(0x80, 60, 0);
        assert!((e.voices[0].increment - before * 2.).abs() < 1e-10);
        assert_eq!(e.voices[0].cursors[2].value(&e.sequences.0[2]), Some(2));
    }
}

#[cfg(test)]
mod independent_arp_tests {
    use super::*;
    fn make(poly: i32) -> Engine {
        let mut e = Engine::new(8000., 1);
        e.set_parameters(Parameters {
            max_poly: poly,
            ..Parameters::default()
        });
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            ..ArpParameters::default()
        });
        e
    }
    #[test]
    fn independent_of_polyphony_and_legacy_behavior() {
        for poly in [1, 8, 64] {
            for behavior in 0..4 {
                let mut e = make(poly);
                e.set_parameters(Parameters {
                    max_poly: poly,
                    behavior,
                    ..Parameters::default()
                });
                e.midi(0x90, 67, 100);
                e.midi(0x90, 60, 100);
                e.render(&mut [0.; 1]);
                assert_eq!(e.voices[e.arp_voice.unwrap()].note, 60);
                e.render(&mut [0.; 1000]);
                assert_eq!(e.voices[e.arp_voice.unwrap()].note, 67);
                e.midi(0x80, 60, 0);
                e.midi(0x80, 67, 0);
                e.render(&mut [0.; 1]);
                assert_eq!(e.active_voices(), 0);
            }
        }
    }
    #[test]
    fn legato_keeps_sequence_progress_and_retrigger_restarts() {
        for retrigger in [0, 1] {
            let mut e = make(8);
            e.set_parameters(Parameters {
                pitch_enabled: 1,
                ..Parameters::default()
            });
            let mut bank = SequenceBank::default();
            bank.0[1] = crate::Sequence::parse("[0,1,2,3,4,5,6,7]", -64, 63).unwrap();
            e.set_sequences(bank);
            e.set_arp_parameters(ArpParameters {
                enabled: 1,
                gate: 1.,
                retrigger,
                ..ArpParameters::default()
            });
            e.midi(0x90, 60, 100);
            e.midi(0x90, 67, 100);
            e.render(&mut [0.; 1001]);
            assert_eq!(
                e.sequence_positions()[1],
                if retrigger == 0 { 7 } else { 0 }
            );
        }
    }
    #[test]
    fn pedal_does_not_override_generated_gate() {
        let mut e = make(8);
        e.midi(0x90, 60, 100);
        e.midi(0xb0, 64, 127);
        e.midi(0x80, 60, 0);
        e.render(&mut [0.; 701]);
        assert_eq!(e.active_voices(), 0);
        e.render(&mut [0.; 300]);
        assert_eq!(e.active_voices(), 1);
        e.midi(0xb0, 64, 0);
        e.render(&mut [0.; 1]);
        assert_eq!(e.active_voices(), 0);
    }
    #[test]
    fn enabling_and_disabling_preserve_physically_held_notes() {
        let mut e = Engine::new(8000., 1);
        e.midi(0x90, 60, 100);
        e.render(&mut [0.; 10]);
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            ..ArpParameters::default()
        });
        e.render(&mut [0.; 10]);
        assert_eq!(e.active_voices(), 1);
        e.set_arp_parameters(ArpParameters::default());
        e.render(&mut [0.; 10]);
        assert_eq!(e.active_voices(), 1);
        e.midi(0x80, 60, 0);
        e.render(&mut [0.; 1]);
        assert_eq!(e.active_voices(), 0);
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    fn info(ppq: f64, playing: bool) -> TransportInfo {
        TransportInfo {
            ppq,
            playing: i32::from(playing),
            valid: 1,
            ..Default::default()
        }
    }
    fn arp() -> Engine {
        let mut e = Engine::new(8000., 1);
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            gate: 0.5,
            ..Default::default()
        });
        e
    }
    fn block(e: &mut Engine, count: usize) -> Vec<f32> {
        let mut out = vec![0.; count];
        e.render(&mut out);
        out
    }
    fn note(e: &Engine) -> u8 {
        e.voices[e.arp_voice.expect("No generated voice")].note
    }
    #[test]
    fn waits_for_grid_and_closes_gate_at_host_phase() {
        let mut e = arp();
        e.set_transport(info(0.1, true));
        e.midi(0x90, 60, 127);
        e.midi(0x90, 64, 127);
        let out = block(&mut e, 1200);
        assert!(out[..600].iter().all(|v| *v == 0.));
        assert!(out[600..1100].iter().all(|v| v.abs() == 0.5));
        assert!(out[1100..].iter().all(|v| *v == 0.));
    }
    #[test]
    fn stopped_transport_retains_latch_and_restarts_on_grid() {
        let mut e = arp();
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            latch: 1,
            ..Default::default()
        });
        e.set_transport(info(0., false));
        e.midi(0x90, 60, 127);
        e.midi(0x80, 60, 0);
        assert!(block(&mut e, 1000).iter().all(|v| *v == 0.));
        e.set_transport(info(4., true));
        block(&mut e, 1);
        assert_eq!(note(&e), 60);
        e.set_transport(info(4.01, false));
        assert_eq!(e.active_voices(), 0);
        block(&mut e, 200);
        e.set_transport(info(8., true));
        block(&mut e, 1);
        assert_eq!(note(&e), 60);
    }
    #[test]
    fn seeks_and_in_block_loops_restart_pattern() {
        let mut e = arp();
        e.set_transport(info(0., true));
        e.midi(0x90, 60, 127);
        e.midi(0x90, 64, 127);
        e.midi(0x90, 67, 127);
        block(&mut e, 1001);
        assert_eq!(note(&e), 64);
        e.set_transport(info(8., true));
        block(&mut e, 1);
        assert_eq!(note(&e), 60);
        e.set_transport(TransportInfo {
            looping: 1,
            loop_start: 0.,
            loop_end: 0.5,
            ..info(0., true)
        });
        block(&mut e, 1);
        assert_eq!(note(&e), 60);
        block(&mut e, 1000);
        assert_eq!(note(&e), 64);
        block(&mut e, 1000);
        assert_eq!(note(&e), 60);
        block(&mut e, 1000);
        assert_eq!(note(&e), 64);
    }
    #[test]
    fn tempo_change_preserves_pattern_and_grid() {
        let mut e = arp();
        e.set_transport(info(0., true));
        e.midi(0x90, 60, 127);
        e.midi(0x90, 64, 127);
        block(&mut e, 500);
        e.set_transport(TransportInfo {
            bpm: 240.,
            ..info(0.125, true)
        });
        block(&mut e, 250);
        assert_eq!(e.active_voices(), 0);
        block(&mut e, 1);
        assert_eq!(note(&e), 64);
    }
    #[test]
    fn free_seconds_and_missing_host_allow_stopped_audition() {
        for kind in 0..3 {
            let mut e = arp();
            if kind == 0 {
                e.set_clock_parameters(ClockParameters {
                    host_sync: 0,
                    ..Default::default()
                });
            }
            if kind == 1 {
                e.set_arp_parameters(ArpParameters {
                    enabled: 1,
                    rate: 15,
                    ..Default::default()
                });
            }
            e.set_transport(if kind == 2 {
                TransportInfo::default()
            } else {
                info(0., false)
            });
            e.midi(0x90, 60, 127);
            assert_eq!(block(&mut e, 1)[0], -0.5);
        }
    }
    #[test]
    fn negative_preroll_and_swung_grid() {
        let mut e = arp();
        e.set_transport(info(-0.1, true));
        e.midi(0x90, 60, 127);
        let out = block(&mut e, 401);
        assert!(out[..400].iter().all(|v| *v == 0.));
        assert_eq!(out[400], -0.5);
        let mut e = arp();
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            swing: 0.25,
            ..Default::default()
        });
        e.set_transport(info(0., true));
        e.midi(0x90, 60, 127);
        e.midi(0x90, 64, 127);
        block(&mut e, 1250);
        block(&mut e, 1);
        assert_eq!(note(&e), 64);
        e.set_transport(info(0.4, true));
        assert!(block(&mut e, 400).iter().all(|v| *v == 0.));
        block(&mut e, 1);
        assert_eq!(note(&e), 60);
    }
    #[test]
    fn musical_sequences_follow_grid_and_seek_but_60hz_stays_note_relative() {
        for rate in [0, 3] {
            let mut e = Engine::new(8000., 1);
            e.set_parameters(Parameters {
                pitch_enabled: 1,
                ..Default::default()
            });
            e.set_clock_parameters(ClockParameters {
                host_sync: 1,
                sequence_rate: rate,
            });
            let mut bank = SequenceBank::default();
            bank.0[1] = crate::Sequence::parse("[0,4,7,12,7]", -64, 63).unwrap();
            e.set_sequences(bank);
            e.set_transport(info(0.1, true));
            e.midi(0x90, 60, 127);
            block(&mut e, 601);
            let before = e.sequence_positions()[1];
            assert_eq!(before, if rate == 3 { 1 } else { 4 });
            e.set_transport(info(8., true));
            block(&mut e, 1);
            assert_eq!(
                e.sequence_positions()[1],
                if rate == 3 { 0 } else { before }
            );
        }
    }
    #[test]
    fn musical_sequence_release_finishes_while_transport_is_stopped() {
        let mut e = Engine::new(8000., 1);
        e.set_parameters(Parameters {
            volume_enabled: 1,
            ..Default::default()
        });
        e.set_clock_parameters(ClockParameters {
            host_sync: 1,
            sequence_rate: 5,
        });
        let mut bank = SequenceBank::default();
        bank.0[0] = crate::Sequence::parse("15[10]|3,2,1,0", 0, 15).unwrap();
        e.set_sequences(bank);
        e.set_transport(info(0., true));
        e.midi(0x90, 60, 127);
        block(&mut e, 400);
        e.midi(0x80, 60, 0);
        e.set_transport(info(0.1, false));
        block(&mut e, 1100);
        assert_eq!(e.active_voices(), 0);
    }
    #[test]
    fn partial_invalid_host_info_falls_back_and_abis_are_stable() {
        let mut e = arp();
        e.set_transport(TransportInfo {
            bpm: f64::NAN,
            ..info(0., false)
        });
        e.midi(0x90, 60, 127);
        assert_eq!(e.clock_status(), 3);
        assert_eq!(block(&mut e, 1)[0], -0.5);
        assert_eq!(std::mem::size_of::<ClockParameters>(), 8);
        assert_eq!(std::mem::size_of::<TransportInfo>(), 48);
    }
}

#[cfg(test)]
mod transport_edge_tests {
    use super::*;
    #[test]
    fn rapid_tempo_ramps_do_not_reset_the_chord_pattern_each_block() {
        let mut e = Engine::new(48000., 1);
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            gate: 1.,
            ..Default::default()
        });
        e.set_transport(TransportInfo {
            valid: 1,
            playing: 1,
            ..Default::default()
        });
        for n in [60, 64, 67] {
            e.midi(0x90, n, 127);
        }
        for block in 0..46 {
            let time = block as f64 * 1024. / 48000.;
            let bpm = 120. + 120. * time;
            let beat = (120. * time + 60. * time * time) / 60.;
            e.set_transport(TransportInfo {
                ppq: beat,
                bpm,
                valid: 1,
                playing: 1,
                ..Default::default()
            });
            e.render(&mut [0.; 1024]);
            let last = e.transport.ppq - bpm / (60. * 48000.);
            let step = ((last + 1e-9) / 0.25).floor() as usize;
            assert_eq!(
                e.voices[e.arp_voice.unwrap()].note,
                [60, 64, 67][step % 3],
                "Pattern reset in ramp block {block}"
            );
        }
    }
    #[test]
    fn sub_sample_host_gates_never_leave_generated_notes_held() {
        let mut e = Engine::new(8000., 1);
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            rate: 4,
            gate: 0.05,
            swing: 0.49,
            ..Default::default()
        });
        e.set_transport(TransportInfo {
            bpm: 999.,
            valid: 1,
            playing: 1,
            ..Default::default()
        });
        e.midi(0x90, 60, 127);
        e.render(&mut [0.; 8000]);
        e.midi(0x80, 60, 0);
        e.render(&mut [0.; 2]);
        assert_eq!(e.active_voices(), 0);
    }
}

#[cfg(test)]
mod seconds_transport_test {
    use super::*;
    #[test]
    fn host_mode_switch_does_not_restart_seconds_playback() {
        let mut e = Engine::new(8000., 1);
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            rate: 15,
            gate: 1.,
            retrigger: 0,
            ..Default::default()
        });
        e.midi(0x90, 60, 127);
        e.render(&mut [0.; 200]);
        let voice = e.arp_voice.unwrap();
        let time = e.voices[voice].time;
        e.set_clock_parameters(ClockParameters {
            host_sync: 0,
            sequence_rate: 0,
        });
        e.render(&mut [0.; 1]);
        assert_eq!(e.arp_voice, Some(voice));
        assert!((e.voices[voice].time - time - 1. / 8000.).abs() < 1e-12);
    }
}
