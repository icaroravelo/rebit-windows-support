//! Fixed-capacity synthesis. Rendering and MIDI handling allocate no memory.
use crate::arpeggiator::{ArpParameters, Arpeggiator};
use crate::meter::Meter;
use crate::modulation::{
    evaluate, GlobalModulation, LfoState, Modulation, ModulationParameters, SourceValues,
};
use crate::sequence::{Cursor, SequenceBank};
use crate::smoothing::{Blend, Ramp};
use crate::transport::{ClockParameters, TransportInfo};
use std::f64::consts::TAU;

pub const MAX_VOICES: usize = 64;
/// Audio-thread snapshot. Counts exclude finished voices; holds include pedals.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlaybackInfo {
    pub peak: f32,
    pub held_peak: f32,
    pub clips: u32,
    pub active: u32,
    pub held: u32,
    pub released: u32,
    pub fading: u32,
}
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
    tuning: f64,
    level: f64,
    stage: u8,
    release_step: f64,
    time: f64,
    frame: f64,
    glide: f64,
    glide_step: f64,
    noise: [Noise; 3],
    pitch_offset: Ramp,
    amplitude_offset: Ramp,
    smooth_amplitude: bool,
    fade_in: Ramp,
    last_amplitude: f64,
    last_increment: f64,
    last_duty: [f64; 3],
    cursors: [Cursor; 3],
    arpeggiated: bool,
    sequence_tick: Option<i64>,
    lfo: [LfoState; 2],
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
            tuning: 0.,
            level: 0.,
            stage: 0,
            release_step: 1.,
            time: 0.,
            frame: 0.,
            glide: 0.,
            glide_step: 0.,
            noise: [Noise::default(); 3],
            pitch_offset: Ramp::new(0.),
            amplitude_offset: Ramp::new(0.),
            smooth_amplitude: false,
            fade_in: Ramp::new(1.),
            last_amplitude: 0.,
            last_increment: 0.,
            last_duty: [1., 0., 0.],
            cursors: [Cursor::default(); 3],
            arpeggiated: false,
            sequence_tick: None,
            lfo: [LfoState::default(); 2],
        }
    }
}
#[derive(Clone, Copy)]
struct Noise {
    sample: f32,
    rng: u32,
    lfsr: u32,
}
impl Default for Noise {
    fn default() -> Self {
        Self {
            sample: 0.,
            rng: 1,
            lfsr: 0x7fff,
        }
    }
}
// Frozen oscillator continuations, never full extra envelope/sequence voices.
// Bounded storage makes stealing and voice-count edits safe on the audio thread.
#[derive(Clone, Copy)]
struct Tail {
    phase: f64,
    increment: f64,
    noise: [Noise; 3],
    amplitude: f64,
    remaining: u32,
    length: u32,
    wave: [f64; 3],
    duty: [f64; 3],
    algorithms: [f64; 3],
    clean: f64,
    channel: u8,
    arpeggiated: bool,
}
impl Default for Tail {
    fn default() -> Self {
        Self {
            phase: 0.,
            increment: 0.,
            noise: [Noise::default(); 3],
            amplitude: 0.,
            remaining: 0,
            length: 1,
            wave: [1., 0., 0.],
            duty: [1., 0., 0.],
            algorithms: [1., 0., 0.],
            clean: 0.,
            channel: 0,
            arpeggiated: false,
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
    tuning: f64,
    sound_mode: i32,
    clean: Ramp,
    meter: Meter,
    voices: [Voice; MAX_VOICES],
    tails: [Tail; MAX_VOICES],
    gain: Ramp,
    sustain_level: Ramp,
    wave: Blend,
    duty: Blend,
    algorithms: Blend,
    duty_sequence: Ramp,
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
    modulation: Modulation,
    pressure: [f64; 16],
    poly_pressure: [[f64; 128]; 16],
    timbre: [f64; 16],
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
            tuning: 0.,
            sound_mode: 0,
            clean: Ramp::new(0.),
            meter: Meter::default(),
            voices: [Voice::default(); MAX_VOICES],
            tails: [Tail::default(); MAX_VOICES],
            gain: Ramp::new(0.5),
            sustain_level: Ramp::new(1.),
            wave: Blend::new(0),
            duty: Blend::new(0),
            algorithms: Blend::new(0),
            duty_sequence: Ramp::new(0.),
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
            modulation: Modulation::new(seed),
            pressure: [0.; 16],
            poly_pressure: [[0.; 128]; 16],
            timbre: [0.; 16],
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
        self.meter.prepare(self.sample_rate);
        self.reset();
    }
    pub fn reset(&mut self) {
        self.meter.reset();
        self.clean.reset(self.sound_mode as f64);
        self.voices.fill(Voice::default());
        self.tails.fill(Tail::default());
        self.snap_parameters();
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
        self.modulation.reset(self.seed);
        self.pressure.fill(0.);
        self.poly_pressure.fill([0.; 128]);
        self.timbre.fill(0.);
    }
    fn transition_samples(&self) -> u32 {
        (self.sample_rate * 0.005).round() as u32
    }
    fn tail_samples(&self) -> u32 {
        (self.sample_rate * 0.003).round() as u32
    }
    fn snap_parameters(&mut self) {
        let p = self.params;
        self.gain.reset(p.gain as f64);
        self.sustain_level.reset(p.sustain as f64);
        self.wave = Blend::new(p.osc);
        self.duty = Blend::new(p.duty);
        self.algorithms = Blend::new(p.noise_algorithm);
        self.duty_sequence.reset(f64::from(p.duty_enabled != 0));
    }
    pub fn set_parameters(&mut self, params: Parameters) {
        let p = params.sanitized();
        let old = self.params;
        if p == old {
            return;
        }
        let samples = self.transition_samples();
        let sounding =
            self.voices.iter().any(|v| v.active) || self.tails.iter().any(|t| t.remaining > 0);
        self.params = p;
        if sounding {
            self.gain.target(p.gain as f64, samples);
            self.sustain_level.target(p.sustain as f64, samples);
            self.wave.target(p.osc, samples);
            self.duty.target(p.duty, samples);
            self.algorithms.target(p.noise_algorithm, samples);
            self.duty_sequence
                .target(f64::from(p.duty_enabled != 0), samples);
        } else {
            self.snap_parameters();
        }
        let pitch_changed = p.bend_range != old.bend_range
            || p.vibrato_period != old.vibrato_period
            || p.vibrato_depth != old.vibrato_depth
            || p.vibrato_delay != old.vibrato_delay
            || p.ignores_wheel != old.ignores_wheel
            || p.sweep_initial != old.sweep_initial
            || p.sweep_time != old.sweep_time
            || p.pitch_enabled != old.pitch_enabled
            || p.pitch_mode != old.pitch_mode;
        let amplitude_changed = p.attack != old.attack
            || p.decay != old.decay
            || p.volume_enabled != old.volume_enabled;
        for v in &mut self.voices {
            if !v.active {
                continue;
            }
            if amplitude_changed {
                v.smooth_amplitude = true;
            }
            if p.volume_enabled == 0 && old.volume_enabled != 0 && v.stage != 3 {
                v.stage = 2;
            }
            if p.release != old.release && v.stage == 3 {
                v.release_step = v.level / (self.sample_rate * (p.release as f64).max(0.003));
            }
            if p.portamento != old.portamento && v.glide != 0. {
                v.glide_step = -v.glide / (self.sample_rate * (p.portamento as f64).max(0.003));
            }
            if pitch_changed {
                let increment = pitch(
                    v,
                    &p,
                    &self.sequences,
                    self.sample_rate,
                    &self.bends,
                    &self.wheels,
                );
                smooth_pitch(v, increment, samples);
            }
        }
        if p.max_poly != old.max_poly || (p.max_poly == 1 && p.behavior != old.behavior) {
            self.reconcile_voices(old.max_poly);
        }
    }
    pub fn set_modulation(&mut self, parameters: ModulationParameters) {
        let next = parameters.sanitized();
        if next == self.modulation.parameters {
            return;
        }
        self.modulation.parameters = next;
    }
    pub fn modulation_parameters(&self) -> ModulationParameters {
        self.modulation.parameters
    }
    /// Mixer-thread sample clock for shared balance and effect destinations.
    /// Per-voice routes advance their own LFO copies inside render().
    pub(crate) fn global_modulation_step(&mut self) -> GlobalModulation {
        self.modulation.advance(self.sample_rate);
        let representative = self
            .voices
            .iter()
            .filter(|v| v.active)
            .max_by_key(|v| v.age);
        let (velocity, wheel, aftertouch, timbre) = representative.map_or((0., 0., 0., 0.), |v| {
            let ch = v.channel as usize;
            (
                v.velocity as f64,
                self.wheels[ch],
                self.poly_pressure[ch][v.note as usize].max(self.pressure[ch]),
                self.timbre[ch],
            )
        });
        let values = SourceValues {
            velocity,
            wheel,
            aftertouch,
            timbre,
            lfo: self.modulation.values(),
            macros: self.modulation.parameters.macros.map(f64::from),
        };
        let out = evaluate(&self.modulation.parameters.routes, &values);
        self.modulation.globals = GlobalModulation {
            balance: out.balance,
            drive_db: out.drive_db,
            mix: out.mix,
        };
        self.modulation.globals
    }
    fn retire(&mut self, voice: Voice) {
        if !voice.active || voice.last_amplitude == 0. {
            return;
        }
        let slot = self
            .tails
            .iter()
            .position(|t| t.remaining == 0)
            .unwrap_or_else(|| {
                self.tails
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        (a.amplitude.abs() * a.remaining as f64 / a.length as f64)
                            .total_cmp(&(b.amplitude.abs() * b.remaining as f64 / b.length as f64))
                    })
                    .map_or(0, |(i, _)| i)
            });
        let length = self.tail_samples().max(1);
        self.tails[slot] = Tail {
            phase: voice.phase,
            increment: voice.last_increment,
            noise: voice.noise,
            amplitude: voice.last_amplitude,
            remaining: length,
            length,
            wave: self.wave.current(),
            duty: voice.last_duty,
            algorithms: self.algorithms.current(),
            clean: self.clean.current,
            channel: voice.channel,
            arpeggiated: voice.arpeggiated,
        };
    }
    fn reconcile_voices(&mut self, previous_limit: i32) {
        let limit = self.params.max_poly as usize;
        let old = self.voices;
        let mono_key = if limit == 1 && self.arp.parameters.enabled == 0 {
            self.newest_key()
        } else {
            None
        };
        let mut indices: [usize; MAX_VOICES] = std::array::from_fn(|i| i);
        indices.sort_unstable_by_key(|&i| {
            let v = old[i];
            std::cmp::Reverse((
                v.active,
                self.arp_voice == Some(i) && self.arp.parameters.enabled != 0,
                mono_key.is_some_and(|(c, n, _)| v.channel as usize == c && v.note == n && v.held),
                v.held,
                v.sustained || v.sostenuto,
                v.age,
            ))
        });
        self.voices.fill(Voice::default());
        let previous_arp = self.arp_voice.take();
        let mut count = 0;
        for i in indices {
            if !old[i].active {
                continue;
            }
            if count < limit {
                self.voices[count] = old[i];
                if previous_arp == Some(i) {
                    self.arp_voice = Some(count);
                }
                count += 1;
            } else {
                self.retire(old[i]);
            }
        }
        if self.arp.parameters.enabled != 0 {
            return;
        }
        // A physically held key may have lost its voice to earlier stealing.
        if let Some((ch, note, velocity)) = mono_key {
            if !self.voices[0].active
                || self.voices[0].channel as usize != ch
                || self.voices[0].note != note
            {
                let v = self.voices[0];
                self.retire(v);
                self.voices[0].active = false;
                let slot = self.spawn_voice(ch, note, velocity, false);
                self.fade_restored(slot, self.keys[ch][note as usize].order);
            }
        } else if self.params.max_poly > previous_limit {
            while self.active_voices() < limit {
                let mut candidate = None;
                let mut order = 0;
                for (ch, keys) in self.keys.iter().enumerate() {
                    for (note, key) in keys.iter().enumerate() {
                        if key.velocity > 0
                            && key.order >= order
                            && !self.voices.iter().any(|v| {
                                v.active
                                    && v.held
                                    && v.channel as usize == ch
                                    && v.note as usize == note
                            })
                        {
                            candidate = Some((ch, note as u8, key.velocity));
                            order = key.order;
                        }
                    }
                }
                let Some((ch, note, velocity)) = candidate else {
                    break;
                };
                let slot = self.spawn_voice(ch, note, velocity, false);
                self.fade_restored(slot, order);
            }
        }
        self.arp_elapsed = 0.;
    }
    fn fade_restored(&mut self, slot: usize, order: u64) {
        let samples = self.tail_samples();
        let v = &mut self.voices[slot];
        v.age = order;
        v.fade_in.reset(0.);
        v.fade_in.target(1., samples);
    }
    pub fn parameters(&self) -> Parameters {
        self.params
    }
    /// Raw is the legacy oscillator. Clean fades in polynomial edge correction.
    pub fn set_sound_mode(&mut self, mode: i32) {
        let mode = mode.clamp(0, 1);
        if mode == self.sound_mode {
            return;
        }
        self.sound_mode = mode;
        if self.active_voices() > 0 || self.tails.iter().any(|t| t.remaining > 0) {
            self.clean.target(mode as f64, self.transition_samples());
        } else {
            self.clean.reset(mode as f64);
        }
    }
    pub fn sound_mode(&self) -> i32 {
        self.sound_mode
    }
    pub fn set_tuning(&mut self, semitones: f64) {
        let tuning = if semitones.is_finite() {
            semitones.clamp(-49., 49.)
        } else {
            0.
        };
        if self.tuning == tuning {
            return;
        }
        self.tuning = tuning;
        let samples = self.transition_samples();
        for v in &mut self.voices {
            if !v.active {
                continue;
            }
            v.tuning = tuning;
            let increment = pitch(
                v,
                &self.params,
                &self.sequences,
                self.sample_rate,
                &self.bends,
                &self.wheels,
            );
            smooth_pitch(v, increment, samples);
        }
    }
    /// Restart a previously dormant layer from the shared performance. Sound
    /// parameters and sequence banks stay independent; no heap work occurs.
    pub fn resume_from(&mut self, source: &Self) {
        self.reset();
        self.keys = source.keys;
        self.bends = source.bends;
        self.wheels = source.wheels;
        self.sustain = source.sustain;
        self.age = source.age;
        self.tempo = source.tempo;
        self.clock = source.clock;
        self.transport = source.transport;
        self.transport_frames = source.transport_frames;
        self.arp_elapsed = source.arp_elapsed;
        self.arp = source.arp.clone();
        if self.arp.parameters.enabled != 0 {
            if let Some(slot) = source.arp_voice {
                let v = source.voices[slot];
                if v.active && v.stage != 3 {
                    self.arp_voice = Some(self.spawn_voice(
                        v.channel as usize,
                        v.note,
                        (v.velocity * 127.).round() as u8,
                        true,
                    ));
                }
            }
        } else {
            // Prefer the source's current mono/legacy-arp note, then newest held
            // voices. Pedal-held voices still represented in the source survive.
            for v in source.voices.iter().filter(|v| v.active && v.stage != 3) {
                let slot = self.spawn_voice(
                    v.channel as usize,
                    v.note,
                    (v.velocity * 127.).round() as u8,
                    false,
                );
                self.voices[slot].held = v.held;
                self.voices[slot].sustained = v.sustained;
                self.voices[slot].sostenuto = v.sostenuto;
                self.voices[slot].age = v.age;
            }
            if self.params.max_poly > 1 || self.active_voices() == 0 {
                self.reconcile_voices(0);
            }
        }
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
        for tail in &mut self.tails {
            if tail.arpeggiated {
                tail.remaining = 0;
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
    pub fn playback_info(&self) -> PlaybackInfo {
        let mut info = PlaybackInfo {
            peak: self.meter.peak,
            held_peak: self.meter.held_peak,
            clips: self.meter.clips,
            ..PlaybackInfo::default()
        };
        for voice in &self.voices {
            if voice.active {
                info.active += 1;
                if voice.stage == 3 {
                    info.released += 1;
                } else {
                    info.held += 1;
                }
            }
        }
        info.fading = self.tails.iter().filter(|tail| tail.remaining > 0).count() as u32;
        info
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
                self.retune(ch, true);
            }
            0xd0 => self.pressure[ch] = data1 as f64 / 127.,
            0xa0 => self.poly_pressure[ch][data1 as usize] = data2 as f64 / 127.,
            0xb0 => self.controller(ch, data1, data2),
            _ => (),
        }
        if matches!(status & 0xf0, 0x80 | 0x90) {
            self.retune(ch, false);
        }
    }
    fn retune(&mut self, ch: usize, smooth: bool) {
        let samples = self.transition_samples();
        for v in &mut self.voices {
            if v.active && v.channel as usize == ch {
                let increment = pitch(
                    v,
                    &self.params,
                    &self.sequences,
                    self.sample_rate,
                    &self.bends,
                    &self.wheels,
                );
                if smooth {
                    smooth_pitch(v, increment, samples);
                } else {
                    v.increment = increment;
                }
            }
        }
    }
    fn note_on(&mut self, ch: usize, note: u8, velocity: u8) {
        self.age = self.age.wrapping_add(1);
        self.poly_pressure[ch][note as usize] = 0.;
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
        if !self.voices.iter().any(|voice| voice.active) {
            self.modulation.retrigger();
        }
        let mut v = Voice {
            active: true,
            held: true,
            note,
            channel: ch as u8,
            velocity: velocity as f32 / 127.,
            age: self.age,
            tuning: self.tuning,
            noise: [Noise {
                rng: seed.max(1),
                ..Noise::default()
            }; 3],
            ..Voice::default()
        };
        for (algorithm, noise) in v.noise.iter_mut().enumerate() {
            noise.sample = next_noise(noise, algorithm);
        }
        for (index, lfo) in v.lfo.iter_mut().enumerate() {
            if self.modulation.parameters.lfo[index].retrigger != 0 {
                lfo.reset(seed ^ index as u32);
            } else {
                *lfo = self.modulation.lfo[index];
            }
        }
        v.increment = pitch(
            &v,
            &self.params,
            &self.sequences,
            self.sample_rate,
            &self.bends,
            &self.wheels,
        );
        v.arpeggiated = arpeggiated;
        let old = self.voices[slot];
        // Preserve zero-release arp gates and deliberate note articulation.
        if old.active && (old.stage != 3 || self.params.release > 0.) {
            self.retire(old);
            v.fade_in.reset(0.);
            v.fade_in.target(1., self.tail_samples());
        }
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
        self.poly_pressure[ch][note as usize] = 0.;
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
                self.retune(ch, true);
            }
            74 => self.timbre[ch] = value as f64 / 127.,
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
                self.pressure[ch] = 0.;
                self.poly_pressure[ch].fill(0.);
                self.timbre[ch] = 0.;
                for tail in &mut self.tails {
                    if tail.channel as usize == ch {
                        tail.remaining = 0;
                    }
                }
                for v in &mut self.voices {
                    if v.channel as usize == ch {
                        v.active = false;
                    }
                }
            }
            123 => {
                self.keys[ch].fill(Key::default());
                self.poly_pressure[ch].fill(0.);
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
                self.pressure[ch] = 0.;
                self.poly_pressure[ch].fill(0.);
                self.timbre[ch] = 0.;
                for v in &mut self.voices {
                    if v.active && v.channel as usize == ch {
                        v.sustained = false;
                        v.sostenuto = false;
                        if !v.held {
                            release(v, &self.params, &self.sequences, self.sample_rate);
                        }
                    }
                }
                self.retune(ch, true);
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
        let mut p = self.params;
        let smoothing_samples = self.transition_samples();
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
            let gain = self.gain.next() as f32;
            p.sustain = self.sustain_level.next() as f32;
            let wave = self.wave.next();
            let clean = self.clean.next();
            let manual_duty = self.duty.next();
            let algorithms = self.algorithms.next();
            let duty_sequence = self.duty_sequence.next();
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
                        v.level = 0.;
                        v.active = false;
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
                let sequence_duty = v.cursors[2]
                    .value(&self.sequences.0[2])
                    .unwrap_or(p.duty as i16)
                    .clamp(0, 2) as usize;
                let duty = std::array::from_fn(|i| {
                    manual_duty[i] * (1. - duty_sequence)
                        + f64::from(i == sequence_duty) * duty_sequence
                });
                for (index, lfo) in v.lfo.iter_mut().enumerate() {
                    lfo.advance(
                        self.sample_rate,
                        self.modulation.parameters.lfo[index].rate_hz as f64,
                    );
                }
                let channel = v.channel as usize;
                let values = SourceValues {
                    velocity: v.velocity as f64,
                    wheel: self.wheels[channel],
                    aftertouch: self.poly_pressure[channel][v.note as usize]
                        .max(self.pressure[channel]),
                    timbre: self.timbre[channel],
                    lfo: [
                        v.lfo[0].value(self.modulation.parameters.lfo[0].shape),
                        v.lfo[1].value(self.modulation.parameters.lfo[1].shape),
                    ],
                    macros: self.modulation.parameters.macros.map(f64::from),
                };
                let modulation = evaluate(&self.modulation.parameters.routes, &values);
                let base_amplitude = v.level * v.velocity as f64;
                if v.smooth_amplitude {
                    v.amplitude_offset.reset(v.last_amplitude - base_amplitude);
                    v.amplitude_offset.target(0., smoothing_samples);
                    v.smooth_amplitude = false;
                }
                let amplitude = ((base_amplitude + v.amplitude_offset.next()).clamp(0., 1.)
                    * modulation.level)
                    .clamp(0., 2.)
                    * v.fade_in.next();
                if v.amplitude_offset.running() {
                    v.active = true;
                }
                v.last_amplitude = amplitude;
                v.last_duty = duty;
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
                let correction = v.pitch_offset.next();
                let mut increment = if correction == 0. {
                    v.increment
                } else {
                    (v.increment * (correction / 12.).exp2()).min(0.49)
                };
                if modulation.pitch != 0. {
                    increment = (increment * (modulation.pitch / 12.).exp2()).min(0.49);
                }
                v.last_increment = increment;
                sum += oscillator(
                    v.phase,
                    &v.noise,
                    wave,
                    duty,
                    algorithms,
                    increment,
                    clean,
                    modulation.duty,
                ) * amplitude as f32;
                advance_oscillator(&mut v.phase, &mut v.noise, increment, wave, algorithms);
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
            for tail in &mut self.tails {
                if tail.remaining == 0 {
                    continue;
                }
                sum += oscillator(
                    tail.phase,
                    &tail.noise,
                    tail.wave,
                    tail.duty,
                    tail.algorithms,
                    tail.increment,
                    tail.clean,
                    0.,
                ) * (tail.amplitude * (tail.remaining - 1) as f64 / tail.length as f64)
                    as f32;
                advance_oscillator(
                    &mut tail.phase,
                    &mut tail.noise,
                    tail.increment,
                    tail.wave,
                    tail.algorithms,
                );
                tail.remaining -= 1;
            }
            sum *= gain;
            *out = if sum.is_finite() { sum } else { 0. };
            self.meter.observe(*out);
            if self.transport.valid != 0 && self.transport.playing != 0 {
                self.transport.ppq += beat_increment;
                self.transport_frames = self.transport_frames.saturating_add(1);
            }
        }
    }
}
fn smooth_pitch(v: &mut Voice, increment: f64, samples: u32) {
    if v.increment == increment {
        return;
    }
    let current = (v.increment * (v.pitch_offset.current / 12.).exp2()).min(0.49);
    v.increment = increment;
    v.pitch_offset.reset(12. * (current / increment).log2());
    v.pitch_offset.target(0., samples);
}
#[allow(clippy::too_many_arguments)]
fn oscillator(
    phase: f64,
    noise: &[Noise; 3],
    wave: [f64; 3],
    duty: [f64; 3],
    algorithms: [f64; 3],
    increment: f64,
    clean: f64,
    duty_offset: f64,
) -> f32 {
    let pulse: f64 = duty
        .iter()
        .zip([0.125, 0.25, 0.5])
        .map(|(weight, width)| {
            weight
                * if phase < (width + duty_offset).clamp(0.02, 0.98) {
                    -1.
                } else {
                    1.
                }
        })
        .sum();
    let triangle = ((1. - 4. * (phase - 0.5).abs()) * 7.5).round() as f32 / 7.5;
    let random: f64 = noise
        .iter()
        .zip(algorithms)
        .map(|(n, weight)| n.sample as f64 * weight)
        .sum();
    let raw = (pulse * wave[0] + triangle as f64 * wave[1] + random * wave[2]) as f32;
    if clean == 0. {
        return raw;
    }
    let clean_pulse: f64 = if wave[0] == 0. {
        0.
    } else {
        duty.iter()
            .zip([0.125, 0.25, 0.5])
            .filter(|(weight, _)| **weight != 0.)
            .map(|(weight, width)| {
                weight
                    * crate::oscillator::clean_pulse(
                        phase,
                        increment,
                        (width + duty_offset).clamp(0.02, 0.98),
                    )
            })
            .sum()
    };
    let clean_tri = if wave[1] == 0. {
        0.
    } else {
        crate::oscillator::clean_triangle(phase, increment)
    };
    let corrected = (clean_pulse * wave[0] + clean_tri * wave[1] + random * wave[2]) as f32;
    if clean == 1. {
        corrected
    } else {
        raw + (corrected - raw) * clean as f32
    }
}
fn advance_oscillator(
    phase: &mut f64,
    noise: &mut [Noise; 3],
    increment: f64,
    wave: [f64; 3],
    algorithms: [f64; 3],
) {
    let old = *phase;
    let next = old + increment;
    *phase = next.fract();
    if wave[2] == 0. {
        return;
    }
    for (algorithm, n) in noise.iter_mut().enumerate() {
        if algorithms[algorithm] == 0. {
            continue;
        }
        let buckets = if algorithm == 0 { 50. } else { 16. };
        if (old * buckets) as u32 != (*phase * buckets) as u32 || next >= 1. {
            n.sample = next_noise(n, algorithm);
        }
    }
}
fn release(v: &mut Voice, p: &Parameters, bank: &SequenceBank, sample_rate: f64) {
    if v.stage == 3 {
        return;
    }
    v.stage = 3;
    v.smooth_amplitude = false;
    v.amplitude_offset.reset(0.);
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
        v.note as f64 + v.tuning + offset + v.glide + bends[ch] * p.bend_range as f64 + sweep + vib;
    (440. * ((semitones - 69.) / 12.).exp2() / sample_rate).min(0.49)
}
fn next_noise(v: &mut Noise, algorithm: usize) -> f32 {
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

#[cfg(test)]
mod live_change_tests {
    use super::*;

    fn sounding(rate: f64) -> Engine {
        let mut e = Engine::new(rate, 123);
        e.midi(0x90, 0, 127);
        e.render(&mut [0.; 200]);
        e
    }
    #[test]
    fn gain_and_sustain_live_edits_ramp_without_restarting_a_note() {
        for rate in [8000., 44100., 48000., 192000., 384000.] {
            for gain in [false, true] {
                let mut e = sounding(rate);
                let phase = e.voices[0].phase;
                let time = e.voices[0].time;
                let mut p = e.parameters();
                if gain {
                    p.gain = 0.;
                } else {
                    p.sustain = 0.25;
                }
                e.set_parameters(p);
                assert_eq!(e.voices[0].phase, phase);
                assert_eq!(e.voices[0].time, time);
                let samples = (rate * 0.005).round() as usize;
                let mut output = vec![0.; samples];
                e.render(&mut output);
                let target = if gain { 0. } else { 0.125 };
                assert!((output[0].abs() - 0.5).abs() <= 0.5 / samples as f32 + 1e-6);
                assert!((output[samples - 1].abs() - target).abs() < 1e-6);
                assert!(e.voices[0].held);
            }
        }
    }
    #[test]
    fn oscillator_duty_and_noise_changes_crossfade_from_current_sound() {
        for control in 0..3 {
            let mut e = sounding(8000.);
            if control == 2 {
                let mut p = e.parameters();
                p.osc = 2;
                e.set_parameters(p);
                e.render(&mut [0.; 40]);
                // Freeze pitch for this measurement so native noise edges are excluded.
                e.voices[0].increment = 0.;
            }
            let mut before = [0.; 1];
            e.render(&mut before);
            let phase = e.voices[0].phase;
            let mut p = e.parameters();
            match control {
                0 => p.osc = 1,
                1 => p.duty = 2,
                _ => p.noise_algorithm = 1,
            }
            e.set_parameters(p);
            assert_eq!(e.voices[0].phase, phase);
            let mut after = [0.; 40];
            e.render(&mut after);
            assert!((after[0] - before[0]).abs() < 0.027, "control {control}");
            let weights = match control {
                0 => e.wave.current(),
                1 => e.duty.current(),
                _ => e.algorithms.current(),
            };
            assert_eq!(
                weights,
                if control == 1 {
                    [0., 0., 1.]
                } else {
                    [0., 1., 0.]
                }
            );
        }
        let mut e = sounding(8000.);
        e.set_parameters(Parameters {
            duty: 2,
            ..e.parameters()
        });
        e.render(&mut [0.; 10]);
        let previous = e.duty.current();
        e.set_parameters(Parameters {
            duty: 1,
            ..e.parameters()
        });
        assert_eq!(e.duty.current(), previous);
        e.render(&mut [0.; 40]);
        assert_eq!(e.duty.current(), [0., 1., 0.]);
    }
    #[test]
    fn pitch_bend_and_live_modulation_reach_targets_without_a_frequency_jump() {
        let mut e = sounding(8000.);
        let previous = e.voices[0].last_increment;
        e.midi(0xe0, 127, 127);
        let target = e.voices[0].increment;
        assert!(target > previous);
        e.render(&mut [0.; 1]);
        assert!((e.voices[0].last_increment / previous - 1.).abs() < 0.004);
        e.render(&mut [0.; 39]);
        assert_eq!(e.voices[0].last_increment, target);
        let previous = target;
        e.set_parameters(Parameters {
            bend_range: 24.,
            ..e.parameters()
        });
        e.render(&mut [0.; 1]);
        assert!((e.voices[0].last_increment / previous - 1.).abs() < 0.04);
        e.render(&mut [0.; 39]);
        assert_eq!(e.voices[0].last_increment, e.voices[0].increment);
        // Lane enable fades the tuning change; actual frame boundaries remain stepped.
        let mut bank = SequenceBank::default();
        bank.0[1] = crate::Sequence::parse("12,0", -64, 63).unwrap();
        e.set_sequences(bank);
        let previous = e.voices[0].last_increment;
        e.set_parameters(Parameters {
            pitch_enabled: 1,
            ..e.parameters()
        });
        e.render(&mut [0.; 1]);
        assert!(e.voices[0].last_increment < previous * 1.02);
        e.render(&mut [0.; 39]);
        assert_eq!(e.voices[0].last_increment, e.voices[0].increment);
    }
    #[test]
    fn volume_lane_toggle_fades_and_frame_edges_stay_crisp() {
        let mut e = sounding(8000.);
        let mut bank = SequenceBank::default();
        bank.0[0] = crate::Sequence::parse("[0,15]", 0, 15).unwrap();
        e.set_sequences(bank);
        e.set_parameters(Parameters {
            volume_enabled: 1,
            ..e.parameters()
        });
        let mut output = [0.; 40];
        e.render(&mut output);
        assert!((output[0] - 0.4875).abs() < 1e-6);
        assert_eq!(output[39], 0.);
        let remaining = (8000. / 60. - 40_f64).ceil() as usize;
        e.render(&mut vec![0.; remaining]);
        let mut edge = [0.; 1];
        e.render(&mut edge);
        assert_eq!(edge[0].abs(), 0.5);
        e.set_parameters(Parameters {
            volume_enabled: 0,
            sustain: 0.25,
            ..e.parameters()
        });
        e.render(&mut output);
        assert!((output[39].abs() - 0.125).abs() < 1e-6);
        assert_eq!(e.voices[0].stage, 2);
    }
    #[test]
    fn poly_mono_poly_restores_physical_keys_and_preserves_kept_voice_progress() {
        let mut e = Engine::new(8000., 123);
        let mut bank = SequenceBank::default();
        bank.0[1] = crate::Sequence::parse("[0,4,7,12]", -64, 63).unwrap();
        e.set_sequences(bank);
        e.set_parameters(Parameters {
            pitch_enabled: 1,
            ..e.parameters()
        });
        for note in [60, 64, 67] {
            e.midi(0x90, note, 127);
        }
        e.render(&mut [0.; 200]);
        let latest = e.voices[2];
        let position = e.sequence_positions();
        e.set_parameters(Parameters {
            max_poly: 1,
            ..e.parameters()
        });
        assert_eq!(e.active_voices(), 1);
        assert_eq!(e.voices[0].note, 67);
        assert_eq!(e.voices[0].phase, latest.phase);
        assert_eq!(e.voices[0].time, latest.time);
        assert_eq!(e.sequence_positions(), position);
        for behavior in [1, 2, 0, 3] {
            e.set_parameters(Parameters {
                behavior,
                ..e.parameters()
            });
            assert_eq!(e.voices[0].time, latest.time);
            assert_eq!(e.keys[0][60].velocity, 127);
        }
        e.set_parameters(Parameters {
            max_poly: 3,
            ..e.parameters()
        });
        assert_eq!(e.active_voices(), 3);
        assert_eq!(e.voices[0].phase, latest.phase);
        for note in [60, 64, 67] {
            e.midi(0x80, note, 0);
        }
        e.render(&mut [0.; 40]);
        assert_eq!(e.active_voices(), 0);
        assert!(e.tails.iter().all(|t| t.remaining == 0));
    }
    #[test]
    fn mono_keeps_latest_key_even_if_a_legacy_arp_has_not_played_it() {
        let mut e = Engine::new(8000., 1);
        e.set_parameters(Parameters {
            max_poly: 1,
            behavior: 1,
            ..e.parameters()
        });
        e.midi(0x90, 60, 127);
        e.render(&mut [0.; 10]);
        e.midi(0x90, 67, 127);
        assert_eq!(e.voices[0].note, 60);
        e.set_parameters(Parameters {
            behavior: 0,
            ..e.parameters()
        });
        assert_eq!(e.voices[0].note, 67);
        e.midi(0x80, 67, 0);
        assert_eq!(e.voices[0].note, 60);
        e.midi(0x80, 60, 0);
        e.render(&mut [0.; 40]);
        assert_eq!(e.active_voices(), 0);
    }
    #[test]
    fn pedal_holds_and_controller_state_survive_voice_reconciliation() {
        for cc in [64, 66] {
            let mut e = Engine::new(8000., 1);
            for note in [60, 67] {
                e.midi(0x90, note, 127);
            }
            e.midi(0xb0, cc, 127);
            e.midi(0xe0, 127, 127);
            for note in [60, 67] {
                e.midi(0x80, note, 0);
            }
            e.render(&mut [0.; 80]);
            e.set_parameters(Parameters {
                max_poly: 1,
                ..e.parameters()
            });
            assert_eq!(e.active_voices(), 1);
            assert!(e.bends[0] > 0.99);
            e.set_parameters(Parameters {
                max_poly: 8,
                ..e.parameters()
            });
            assert_eq!(e.active_voices(), 1); // discarded pedal-only keys are not resurrected
            e.midi(0xb0, cc, 0);
            e.render(&mut [0.; 40]);
            assert_eq!(e.active_voices(), 0);
            assert!(e.tails.iter().all(|t| t.remaining == 0));
        }
    }
    #[test]
    fn independent_arp_keeps_current_high_slot_when_voice_count_shrinks() {
        let mut e = Engine::new(8000., 1);
        e.set_parameters(Parameters {
            release: 1.,
            ..e.parameters()
        });
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            rate: 15,
            seconds: 0.01,
            gate: 1.,
            ..Default::default()
        });
        e.midi(0x90, 60, 127);
        e.midi(0x90, 67, 127);
        e.render(&mut [0.; 241]);
        let slot = e.arp_voice.unwrap();
        assert!(slot > 0);
        let current = e.voices[slot];
        e.set_parameters(Parameters {
            max_poly: 1,
            behavior: 2,
            ..e.parameters()
        });
        assert_eq!(e.arp_voice, Some(0));
        assert_eq!(e.voices[0].note, current.note);
        assert_eq!(e.voices[0].time, current.time);
        assert_eq!(e.voices[0].phase, current.phase);
        e.render(&mut [0.; 80]);
        assert_ne!(e.voices[e.arp_voice.unwrap()].note, current.note);
        e.midi(0xb0, 120, 0);
        let mut silence = [1.; 40];
        e.render(&mut silence);
        assert_eq!(silence, [0.; 40]);
    }
    #[test]
    fn removed_and_stolen_voices_fade_for_three_ms_but_panic_is_immediate() {
        let mut e = sounding(8000.);
        e.set_parameters(Parameters {
            behavior: 3,
            ..e.parameters()
        });
        e.midi(0x91, 0, 127);
        e.render(&mut [0.; 200]);
        e.set_parameters(Parameters {
            max_poly: 1,
            ..e.parameters()
        });
        assert_eq!(e.active_voices(), 1);
        assert_eq!(e.tails.iter().filter(|t| t.remaining > 0).count(), 1);
        e.midi(0xb1, 120, 0);
        let mut tail = [0.; 24];
        e.render(&mut tail);
        assert!((tail[0].abs() - 0.5 * 23. / 24.).abs() < 1e-6);
        assert_eq!(tail[23], 0.);
        e.midi(0x90, 0, 127);
        e.render(&mut [0.; 200]);
        e.midi(0x90, 1, 127);
        let mut first = [0.; 1];
        e.render(&mut first);
        assert!((first[0].abs() - 0.5).abs() < 0.05);
        assert!(e.tails.iter().any(|t| t.remaining > 0));
        e.midi(0xb0, 120, 0);
        e.render(&mut first);
        assert_eq!(first, [0.]);
        e.reset();
        assert!(e.tails.iter().all(|t| t.remaining == 0));
    }
    #[test]
    fn zero_release_noteoff_stays_immediate_during_live_fades() {
        let mut e = sounding(8000.);
        e.set_parameters(Parameters {
            gain: 0.1,
            volume_enabled: 1,
            ..e.parameters()
        });
        e.render(&mut [0.; 1]);
        e.midi(0x80, 0, 0);
        let mut output = [1.; 80];
        e.render(&mut output);
        assert!(output.iter().all(|&x| x == 0.));
        assert_eq!(e.active_voices(), 0);
    }
    #[test]
    fn editing_release_to_zero_finishes_with_a_short_fade() {
        let mut e = sounding(8000.);
        e.set_parameters(Parameters {
            release: 1.,
            ..e.parameters()
        });
        e.midi(0x80, 0, 0);
        e.render(&mut [0.; 100]);
        e.set_parameters(Parameters {
            release: 0.,
            ..e.parameters()
        });
        let mut output = [0.; 25];
        e.render(&mut output);
        assert!(output[0].abs() > 0.4);
        assert_eq!(output[24], 0.);
        assert_eq!(e.active_voices(), 0);
    }
}

#[cfg(test)]
mod retirement_tests {
    use super::*;
    #[test]
    fn host_stop_clears_retiring_generated_voices_immediately() {
        let mut e = Engine::new(8000., 1);
        e.set_parameters(Parameters {
            release: 1.,
            ..e.parameters()
        });
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            rate: 4,
            gate: 1.,
            ..Default::default()
        });
        e.midi(0x90, 60, 127);
        e.render(&mut [0.; 751]);
        e.set_parameters(Parameters {
            max_poly: 1,
            ..e.parameters()
        });
        assert!(e.tails.iter().any(|t| t.remaining > 0));
        e.set_transport(TransportInfo {
            valid: 1,
            playing: 0,
            ..Default::default()
        });
        let mut silence = [1.; 40];
        e.render(&mut silence);
        assert_eq!(silence, [0.; 40]);
        assert!(e.tails.iter().all(|t| t.remaining == 0));
    }
    #[test]
    fn full_tail_pool_stays_bounded_and_drains_after_rapid_steals() {
        let mut e = Engine::new(384000., 1);
        e.set_parameters(Parameters {
            max_poly: 64,
            behavior: 3,
            release: 1.,
            osc: 2,
            ..e.parameters()
        });
        for note in 0..64 {
            e.midi(0x90, note, 127);
        }
        e.render(&mut [0.; 40]);
        e.set_parameters(Parameters {
            max_poly: 1,
            ..e.parameters()
        });
        for note in 64..128 {
            e.midi(0x90, note, 127);
            let mut sample = [0.; 1];
            e.render(&mut sample);
            assert!(sample[0].is_finite());
        }
        assert_eq!(
            e.tails.iter().filter(|t| t.remaining > 0).count(),
            MAX_VOICES
        );
        assert_eq!(e.active_voices(), 1);
        e.render(&mut [0.; 1152]);
        assert!(e.tails.iter().all(|t| t.remaining == 0));
        e.midi(0xb0, 120, 0);
        let mut silence = [1.; 1];
        e.render(&mut silence);
        assert_eq!(silence, [0.]);
    }
}

#[cfg(test)]
mod playback_tests {
    use super::*;
    #[test]
    fn peak_is_measured_after_voice_cancellation() {
        let mut e = Engine::new(8000., 1);
        e.set_parameters(Parameters {
            duty: 2,
            ..e.parameters()
        });
        e.midi(0x90, 69, 127);
        e.midi(0x91, 69, 127);
        e.voices[1].phase = 0.5;
        let mut output = [0.; 7];
        e.render(&mut output);
        assert_eq!(output, [0.; 7]);
        let info = e.playback_info();
        assert_eq!(info.active, 2);
        assert_eq!(info.peak, 0.);
        assert_eq!(info.clips, 0);
    }
}

#[cfg(test)]
mod tuning_tests {
    use super::*;
    #[test]
    fn transpose_and_cents_apply_to_new_and_held_voices_without_restarting() {
        let mut e = Engine::new(48000., 1);
        e.set_tuning(-12.07);
        e.midi(0x90, 69, 127);
        e.render(&mut [0.; 32]);
        let expected = 440. * (-12.07_f64 / 12.).exp2() / 48000.;
        assert!((e.voices[0].increment - expected).abs() < 1e-12);
        let time = e.voices[0].time;
        let phase = e.voices[0].phase;
        e.set_tuning(7.13);
        assert_eq!(e.voices[0].time, time);
        assert_eq!(e.voices[0].phase, phase);
        e.render(&mut [0.; 240]);
        let expected = 440. * (7.13_f64 / 12.).exp2() / 48000.;
        assert!((e.voices[0].last_increment - expected).abs() < 1e-12);
        e.set_tuning(f64::NAN);
        e.render(&mut [0.; 240]);
        assert!((e.voices[0].last_increment - 440. / 48000.).abs() < 1e-12);
    }
}

#[cfg(test)]
mod quality_tests {
    use super::*;
    #[test]
    fn raw_clean_transition_keeps_voice_phase_and_finishes_in_five_ms() {
        let mut e = Engine::new(8000., 1);
        e.midi(0x90, 60, 127);
        e.render(&mut [0.; 100]);
        let phase = e.voices[0].phase;
        let time = e.voices[0].time;
        e.set_sound_mode(1);
        assert_eq!(e.voices[0].phase, phase);
        assert_eq!(e.voices[0].time, time);
        e.render(&mut [0.; 20]);
        assert!((e.clean.current - 0.5).abs() < 1e-12);
        e.render(&mut [0.; 20]);
        assert_eq!(e.clean.current, 1.);
        assert_eq!(e.active_voices(), 1);
        e.midi(0x80, 60, 0);
        let mut output = [1.; 16];
        e.render(&mut output);
        assert_eq!(output, [0.; 16]);
        e.reset();
        assert_eq!(e.clean.current, 1.);
        e.set_sound_mode(0);
        assert_eq!(e.clean.current, 0.);
    }
    #[test]
    fn noise_is_identical_in_both_modes() {
        for algorithm in 0..3 {
            let mut raw = Engine::new(48000., 123);
            let mut clean = Engine::new(48000., 123);
            for e in [&mut raw, &mut clean] {
                e.set_parameters(Parameters {
                    osc: 2,
                    noise_algorithm: algorithm,
                    ..Default::default()
                });
                e.midi(0x90, 60, 100);
            }
            clean.set_sound_mode(1);
            clean.render(&mut [0.; 240]);
            raw.render(&mut [0.; 240]);
            let mut a = [0.; 4096];
            let mut b = [0.; 4096];
            raw.render(&mut a);
            clean.render(&mut b);
            assert_eq!(a, b);
        }
    }
    #[test]
    fn switching_back_to_raw_keeps_the_original_oscillator_phase() {
        for osc in 0..2 {
            let mut raw = Engine::new(48000., 1);
            let mut other = Engine::new(48000., 1);
            for e in [&mut raw, &mut other] {
                e.set_parameters(Parameters {
                    osc,
                    ..Default::default()
                });
                e.midi(0x90, 69, 127);
            }
            other.set_sound_mode(1);
            raw.render(&mut [0.; 500]);
            other.render(&mut [0.; 500]);
            other.set_sound_mode(0);
            raw.render(&mut [0.; 240]);
            other.render(&mut [0.; 240]);
            let mut a = [0.; 256];
            let mut b = [0.; 256];
            raw.render(&mut a);
            other.render(&mut b);
            assert_eq!(a, b);
        }
    }
}
