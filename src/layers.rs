//! Two bounded engines, stereo balance and output metering. Default layer A is
//! unity/centre and B is dormant, preserving the single-layer signal exactly.
use crate::effects::{Effects, EffectsParameters};
use crate::meter::Meter;
use crate::smoothing::Ramp;
use crate::{Engine, PlaybackInfo};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerMix {
    pub enabled: i32,
    pub gain: f32,
    pub pan: f32,
}
impl Default for LayerMix {
    fn default() -> Self {
        Self {
            enabled: 1,
            gain: 1.,
            pan: 0.,
        }
    }
}
#[derive(Clone, Copy)]
struct MixState {
    settings: LayerMix,
    gain: Ramp,
    pan: Ramp,
    gate: Ramp,
}
impl MixState {
    fn new(settings: LayerMix) -> Self {
        Self {
            settings,
            gain: Ramp::new(settings.gain as f64),
            pan: Ramp::new(settings.pan as f64),
            gate: Ramp::new(f64::from(settings.enabled != 0)),
        }
    }
    fn active(&self) -> bool {
        self.settings.enabled != 0 || self.gate.current > 0.
    }
}
pub struct LayerMixer {
    mix: [MixState; 2],
    meter: Meter,
    effects: Effects,
    rate: f64,
    running_b: bool,
    sounding: bool,
}
impl Default for LayerMixer {
    fn default() -> Self {
        Self {
            mix: [
                MixState::new(LayerMix::default()),
                MixState::new(LayerMix {
                    enabled: 0,
                    gain: 0.5,
                    pan: 0.,
                }),
            ],
            meter: Meter::default(),
            effects: Effects::default(),
            rate: 44100.,
            running_b: false,
            sounding: false,
        }
    }
}
impl LayerMixer {
    pub fn prepare(&mut self, rate: f64) {
        self.rate = if rate.is_finite() {
            rate.clamp(8000., 384000.)
        } else {
            44100.
        };
        self.meter.prepare(self.rate);
        self.effects.prepare(self.rate);
        self.reset();
    }
    pub fn reset(&mut self) {
        self.meter.reset();
        self.effects.reset();
        self.running_b = false;
        self.sounding = false;
        for state in &mut self.mix {
            *state = MixState::new(state.settings);
        }
    }
    pub fn set_effects(&mut self, parameters: EffectsParameters) {
        self.effects.set_parameters(parameters, self.sounding);
    }
    pub fn clear_effects_history(&mut self) {
        self.effects.clear_history();
    }
    pub fn set_mix(&mut self, layer: usize, mut settings: LayerMix) {
        let Some(state) = self.mix.get_mut(layer) else {
            return;
        };
        settings.enabled = i32::from(settings.enabled != 0);
        settings.gain = if settings.gain.is_finite() {
            settings.gain.clamp(0., 1.)
        } else {
            0.
        };
        settings.pan = if settings.pan.is_finite() {
            settings.pan.clamp(-1., 1.)
        } else {
            0.
        };
        // Host normalization of a stepped -1..1 parameter can round nominal
        // centre to a tiny nonzero value. Keep legacy centre stereo bit-exact.
        if settings.pan.abs() < 1e-6 {
            settings.pan = 0.;
        }
        if (settings.pan.abs() - 1.).abs() < 1e-6 {
            settings.pan = settings.pan.signum();
        }
        if settings.gain < 1e-6 {
            settings.gain = 0.;
        }
        if 1. - settings.gain < 1e-6 {
            settings.gain = 1.;
        }
        if settings == state.settings {
            return;
        }
        state.settings = settings;
        if !self.sounding {
            *state = MixState::new(settings);
        } else {
            let samples = (self.rate * 0.005).round() as u32;
            state.gain.target(settings.gain as f64, samples);
            state.pan.target(settings.pan as f64, samples);
            state.gate.target(f64::from(settings.enabled != 0), samples);
        }
    }
    pub fn layer_playback(&self, layer: usize, engine: &Engine) -> PlaybackInfo {
        if self.mix.get(layer).is_some_and(MixState::active) {
            engine.playback_info()
        } else {
            PlaybackInfo::default()
        }
    }
    pub fn playback(&self, first: &Engine, second: &Engine) -> PlaybackInfo {
        let a = self.layer_playback(0, first);
        let b = self.layer_playback(1, second);
        PlaybackInfo {
            peak: self.meter.peak,
            held_peak: self.meter.held_peak,
            clips: self.meter.clips,
            active: a.active + b.active,
            held: a.held + b.held,
            released: a.released + b.released,
            fading: a.fading + b.fading,
        }
    }
    pub fn render(
        &mut self,
        first: &mut Engine,
        second: &mut Engine,
        left: &mut [f32],
        right: &mut [f32],
    ) {
        self.render_inner(first, second, left, right, false);
    }
    pub fn render_mono(&mut self, first: &mut Engine, second: &mut Engine, output: &mut [f32]) {
        let mut right = [0.; 128];
        for part in output.chunks_mut(128) {
            self.render_inner(first, second, part, &mut right[..part.len()], true);
        }
    }
    fn render_inner(
        &mut self,
        first: &mut Engine,
        second: &mut Engine,
        left: &mut [f32],
        right: &mut [f32],
        mono: bool,
    ) {
        assert_eq!(left.len(), right.len());
        if self.mix[1].settings.enabled != 0 && !self.running_b {
            second.resume_from(first);
            self.running_b = true;
        }
        let mut a = [0.; 128];
        let mut b = [0.; 128];
        for (l, r) in left.chunks_mut(128).zip(right.chunks_mut(128)) {
            let n = l.len();
            first.render(&mut a[..n]);
            let render_b = self.running_b && self.mix[1].active();
            if render_b {
                second.render(&mut b[..n]);
            } else {
                b[..n].fill(0.);
            }
            for i in 0..n {
                let mut channels = [0_f32; 2];
                for (layer, sample) in [a[i], b[i]].into_iter().enumerate() {
                    let s = &mut self.mix[layer];
                    let gain = (s.gain.next() * s.gate.next()) as f32;
                    let pan = s.pan.next() as f32;
                    // Centre is unity in both channels for legacy patch parity.
                    channels[0] += sample * gain * (1. - pan.max(0.));
                    channels[1] += sample * gain * (1. + pan.min(0.));
                }
                if mono {
                    let sample = (channels[0] + channels[1]) * 0.5;
                    channels = [sample, sample];
                }
                channels = self.effects.process(channels);
                l[i] = channels[0];
                r[i] = channels[1];
                self.meter.observe(l[i].abs().max(r[i].abs()));
            }
            if self.running_b && !self.mix[1].active() {
                second.reset();
                self.running_b = false;
            }
        }
        self.sounding = first.active_voices() > 0
            || (self.running_b && second.active_voices() > 0)
            || self.playback(first, second).fading > 0
            || self.effects.has_tail();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArpParameters, Parameters, Sequence, SequenceBank, TransportInfo};
    fn engines() -> (Engine, Engine, LayerMixer) {
        let a = Engine::new(8000., 1);
        let b = Engine::new(8000., 2);
        let mut m = LayerMixer::default();
        m.prepare(8000.);
        (a, b, m)
    }
    #[test]
    fn default_mix_preserves_single_engine_output_exactly() {
        for osc in 0..3 {
            let (mut a, mut b, mut m) = engines();
            let mut old = Engine::new(8000., 1);
            let p = Parameters {
                osc,
                ..Default::default()
            };
            a.set_parameters(p);
            old.set_parameters(p);
            for note in [48, 60, 67] {
                a.midi(0x90, note, 100);
                old.midi(0x90, note, 100);
            }
            let mut left = [0.; 4096];
            let mut right = [0.; 4096];
            let mut expected = [0.; 4096];
            m.render(&mut a, &mut b, &mut left, &mut right);
            old.render(&mut expected);
            assert_eq!(left, expected);
            assert_eq!(right, expected);
            assert!(!m.running_b);
            assert_eq!(m.layer_playback(1, &b).active, 0);
        }
    }
    #[test]
    fn independent_layers_pan_to_separate_channels_and_meter_combined_output() {
        let (mut a, mut b, mut m) = engines();
        b.set_parameters(Parameters {
            osc: 1,
            ..Default::default()
        });
        b.set_tuning(-12.);
        m.set_mix(
            0,
            LayerMix {
                pan: -1.,
                ..Default::default()
            },
        );
        m.set_mix(
            1,
            LayerMix {
                pan: 1.,
                ..Default::default()
            },
        );
        a.midi(0x90, 0, 127);
        let mut l = [0.; 16];
        let mut r = [0.; 16];
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_eq!(l, [-0.5; 16]);
        assert_ne!(l, r);
        assert_eq!(m.playback(&a, &b).active, 2);
        assert_eq!(
            m.playback(&a, &b).held_peak,
            l.iter().chain(&r).map(|x| x.abs()).fold(0_f32, f32::max)
        );
        assert_eq!(m.playback(&a, &b).clips, 0);
        let (mut a, mut b, mut m) = engines();
        m.set_mix(1, LayerMix::default());
        a.midi(0x90, 0, 127);
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_eq!(l, [-1.; 16]);
        assert_eq!(r, l);
        assert_eq!(m.playback(&a, &b).clips, 16);
    }
    #[test]
    fn mono_output_averages_stereo_and_meter_follows_downmix() {
        let (mut a, mut b, mut m) = engines();
        m.set_mix(
            0,
            LayerMix {
                pan: -1.,
                ..Default::default()
            },
        );
        a.midi(0x90, 0, 127);
        let mut mono = [0.; 32];
        m.render_mono(&mut a, &mut b, &mut mono);
        assert_eq!(mono, [-0.25; 32]);
        assert_eq!(m.playback(&a, &b).held_peak, 0.25);
    }
    #[test]
    fn envelopes_and_all_sequence_lanes_are_independent() {
        let (mut a, mut b, mut m) = engines();
        a.set_parameters(Parameters {
            volume_enabled: 1,
            pitch_enabled: 1,
            duty_enabled: 1,
            ..Default::default()
        });
        b.set_parameters(Parameters {
            volume_enabled: 1,
            pitch_enabled: 1,
            duty_enabled: 1,
            ..Default::default()
        });
        let mut ba = SequenceBank::default();
        ba.0[0] = Sequence::parse("[15,0]|0", 0, 15).unwrap();
        ba.0[1] = Sequence::parse("[0,12]|0", -64, 63).unwrap();
        ba.0[2] = Sequence::parse("[0,2]|0", 0, 2).unwrap();
        let mut bb = SequenceBank::default();
        bb.0[0] = Sequence::parse("[0,15,7]|3,0", 0, 15).unwrap();
        bb.0[1] = Sequence::parse("[7,0,-12]|0", -64, 63).unwrap();
        bb.0[2] = Sequence::parse("[2,1,0]|0", 0, 2).unwrap();
        a.set_sequences(ba);
        b.set_sequences(bb);
        m.set_mix(
            0,
            LayerMix {
                pan: -1.,
                ..Default::default()
            },
        );
        m.set_mix(
            1,
            LayerMix {
                pan: 1.,
                ..Default::default()
            },
        );
        a.midi(0x90, 60, 127);
        let mut l = [0.; 134];
        let mut r = [0.; 134];
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert!(l[..133].iter().any(|x| *x != 0.));
        assert!(r[..133].iter().all(|x| *x == 0.));
        assert_eq!(a.sequence_positions(), [1, 1, 1]);
        assert_eq!(b.sequence_positions(), [1, 1, 1]);
        let mut l = [0.; 134];
        let mut r = [0.; 134];
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert!(l[..132].iter().all(|x| *x == 0.));
        assert!(r[..132].iter().any(|x| *x != 0.));
        assert_eq!(a.sequence_positions(), [0, 0, 0]);
        assert_eq!(b.sequence_positions(), [2, 2, 2]);
        a.midi(0x80, 60, 0);
        b.midi(0x80, 60, 0);
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_eq!(a.active_voices(), 0);
        assert_eq!(b.active_voices(), 1);
    }
    #[test]
    fn layer_b_enable_resumes_held_and_pedal_notes_and_disable_sleeps() {
        for pedal in [0, 64, 66] {
            let (mut a, mut b, mut m) = engines();
            a.midi(0x90, 0, 127);
            if pedal != 0 {
                a.midi(0xb0, pedal, 127);
                a.midi(0x80, 0, 0);
            }
            let mut l = [0.; 80];
            let mut r = [0.; 80];
            m.render(&mut a, &mut b, &mut l, &mut r);
            m.set_mix(
                1,
                LayerMix {
                    enabled: 1,
                    gain: 1.,
                    pan: 1.,
                },
            );
            m.render(&mut a, &mut b, &mut l, &mut r);
            assert_eq!(b.active_voices(), 1);
            assert!(m.running_b);
            if pedal != 0 {
                a.midi(0xb0, pedal, 0);
                b.midi(0xb0, pedal, 0);
            } else {
                a.midi(0x80, 0, 0);
                b.midi(0x80, 0, 0);
            }
            m.render(&mut a, &mut b, &mut l, &mut r);
            assert_eq!(m.playback(&a, &b).active, 0);
            m.set_mix(
                1,
                LayerMix {
                    enabled: 0,
                    ..Default::default()
                },
            );
            m.render(&mut a, &mut b, &mut l, &mut r);
            assert!(!m.running_b);
        }
    }
    #[test]
    fn live_level_pan_and_enable_changes_fade_for_five_ms() {
        let (mut a, mut b, mut m) = engines();
        a.midi(0x90, 0, 127);
        let mut l = [0.; 40];
        let mut r = [0.; 40];
        m.render(&mut a, &mut b, &mut l, &mut r);
        m.set_mix(
            0,
            LayerMix {
                enabled: 0,
                ..Default::default()
            },
        );
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert!((l[0] + 0.4875).abs() < 1e-6);
        assert_eq!(l[39], 0.);
        m.set_mix(
            0,
            LayerMix {
                pan: -1.,
                ..Default::default()
            },
        );
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_ne!(l[39], 0.);
        assert_eq!(r[39], 0.);
        m.set_mix(
            0,
            LayerMix {
                gain: 0.,
                ..Default::default()
            },
        );
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_eq!(l[39], 0.);
    }
    #[test]
    fn shared_arp_keeps_gate_and_transport_alignment_after_late_enable() {
        let (mut a, mut b, mut m) = engines();
        let arp = ArpParameters {
            enabled: 1,
            gate: 0.5,
            ..Default::default()
        };
        a.set_arp_parameters(arp);
        b.set_arp_parameters(arp);
        let t = TransportInfo {
            valid: 1,
            playing: 1,
            ..Default::default()
        };
        a.set_transport(t);
        b.set_transport(t);
        for note in [60, 67] {
            a.midi(0x90, note, 127);
        }
        let mut l = [0.; 100];
        let mut r = [0.; 100];
        m.render(&mut a, &mut b, &mut l, &mut r);
        m.set_mix(
            0,
            LayerMix {
                pan: -1.,
                ..Default::default()
            },
        );
        m.set_mix(
            1,
            LayerMix {
                pan: 1.,
                ..Default::default()
            },
        );
        let mut l = [0.; 950];
        let mut r = [0.; 950];
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert!(r[40..400].iter().any(|x| *x != 0.));
        assert!(r[400..900].iter().all(|x| *x == 0.));
        assert!(r[900..].iter().any(|x| *x != 0.));
        let stop = TransportInfo {
            valid: 1,
            playing: 0,
            ..Default::default()
        };
        a.set_transport(stop);
        b.set_transport(stop);
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert!(l.iter().chain(&r).all(|x| *x == 0.));
    }
    #[test]
    fn layer_limits_and_invalid_mix_are_bounded_and_finite() {
        let (mut a, mut b, mut m) = engines();
        let p = crate::Parameters {
            max_poly: 64,
            ..Default::default()
        };
        a.set_parameters(p);
        b.set_parameters(p);
        m.set_mix(1, LayerMix::default());
        for note in 0..128 {
            a.midi(0x90, note, 127);
        }
        let mut l = [0.; 128];
        let mut r = [0.; 128];
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_eq!(m.playback(&a, &b).active, 128);
        m.set_mix(
            0,
            LayerMix {
                enabled: 99,
                gain: f32::NAN,
                pan: f32::INFINITY,
            },
        );
        m.set_mix(9, LayerMix::default());
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert!(l.iter().chain(&r).all(|x| x.is_finite()));
        for e in [&mut a, &mut b] {
            e.midi(0xb0, 120, 0);
        }
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert!(l.iter().chain(&r).all(|x| *x == 0.));
    }
}

#[cfg(test)]
mod colour_tests {
    use super::*;
    #[test]
    fn bypass_fades_when_only_a_sample_hold_tail_remains() {
        let mut a = Engine::new(8000., 1);
        let mut b = Engine::new(8000., 2);
        let mut m = LayerMixer::default();
        m.prepare(8000.);
        let fx = EffectsParameters {
            enabled: 1,
            rate_divider: 32.,
            ..Default::default()
        };
        m.set_effects(fx);
        a.midi(0x90, 0, 127);
        m.render(&mut a, &mut b, &mut [0.; 10], &mut [0.; 10]);
        a.midi(0x80, 0, 0);
        let mut l = [0.; 1];
        let mut r = [0.; 1];
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_eq!(a.active_voices(), 0);
        assert_ne!(l[0], 0.);
        m.set_effects(EffectsParameters { enabled: 0, ..fx });
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_ne!(l[0], 0.);
        m.render(&mut a, &mut b, &mut [0.; 40], &mut [0.; 40]);
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert_eq!(l, [0.]);
    }
    #[test]
    fn output_meter_observes_effects_and_bypass_preserves_raw_output() {
        let mut a = Engine::new(8000., 1);
        let mut b = Engine::new(8000., 2);
        let mut m = LayerMixer::default();
        m.prepare(8000.);
        m.set_effects(EffectsParameters {
            enabled: 1,
            drive_db: 6.,
            bits: 4.,
            rate_divider: 4.,
            mix: 1.,
        });
        a.midi(0x90, 0, 127);
        let mut l = [0.; 16];
        let mut r = [0.; 16];
        m.render(&mut a, &mut b, &mut l, &mut r);
        let peak = l.iter().chain(&r).map(|x| x.abs()).fold(0_f32, f32::max);
        assert!(peak > 0.5);
        assert_eq!(m.playback(&a, &b).held_peak, peak);
        assert_eq!(l, r);
        m.set_effects(EffectsParameters {
            enabled: 0,
            drive_db: 24.,
            bits: 4.,
            rate_divider: 32.,
            mix: 1.,
        });
        m.render(&mut a, &mut b, &mut [0.; 40], &mut [0.; 40]);
        m.render(&mut a, &mut b, &mut l, &mut r);
        assert!(l.iter().all(|x| x.abs() == 0.5));
    }
}
