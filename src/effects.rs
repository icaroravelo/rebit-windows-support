//! Shared stereo colour chain. Bypass and neutral settings preserve dry samples.
use crate::smoothing::Ramp;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectsParameters {
    pub enabled: i32,
    pub drive_db: f32,
    pub bits: f32,
    pub rate_divider: f32,
    pub mix: f32,
}
impl Default for EffectsParameters {
    fn default() -> Self {
        Self {
            enabled: 0,
            drive_db: 0.,
            bits: 24.,
            rate_divider: 1.,
            mix: 1.,
        }
    }
}
fn finite(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}
impl EffectsParameters {
    fn sanitized(mut self) -> Self {
        self.enabled = i32::from(self.enabled != 0);
        self.drive_db = finite(self.drive_db, 0., 24., 0.);
        self.bits = finite(self.bits, 4., 24., 24.);
        self.rate_divider = finite(self.rate_divider, 1., 32., 1.);
        self.mix = finite(self.mix, 0., 1., 1.);
        if self.drive_db < 1e-6 {
            self.drive_db = 0.;
        }
        if self.bits > 23.99999 {
            self.bits = 24.;
        }
        if self.rate_divider < 1.000001 {
            self.rate_divider = 1.;
        }
        self
    }
}
pub struct Effects {
    params: EffectsParameters,
    gate: Ramp,
    drive: Ramp,
    bits: Ramp,
    divider: Ramp,
    mix: Ramp,
    phase: f64,
    held: [f32; 2],
    last_drive: f64,
    k: f64,
    normalization: f64,
    last_bits: f64,
    levels: f64,
    rate: f64,
}
impl Default for Effects {
    fn default() -> Self {
        Self {
            params: EffectsParameters::default(),
            gate: Ramp::new(0.),
            drive: Ramp::new(0.),
            bits: Ramp::new(24.),
            divider: Ramp::new(1.),
            mix: Ramp::new(1.),
            phase: 1.,
            held: [0.; 2],
            last_drive: -1.,
            k: 0.,
            normalization: 1.,
            last_bits: -1.,
            levels: 1.,
            rate: 44100.,
        }
    }
}
impl Effects {
    pub fn prepare(&mut self, rate: f64) {
        self.rate = if rate.is_finite() {
            rate.clamp(8000., 384000.)
        } else {
            44100.
        };
        self.reset();
    }
    pub fn clear_history(&mut self) {
        self.held = [0.; 2];
        self.phase = 1.;
    }
    pub(crate) fn has_tail(&self) -> bool {
        self.gate.current * self.mix.current > 0.
            && self.divider.current > 1.000001
            && self.held.iter().any(|x| *x != 0.)
    }
    pub fn reset(&mut self) {
        self.clear_history();
        let p = self.params;
        self.gate.reset(f64::from(p.enabled != 0));
        self.drive.reset(p.drive_db as f64);
        self.bits.reset(p.bits as f64);
        self.divider.reset(p.rate_divider as f64);
        self.mix.reset(p.mix as f64);
        self.last_drive = -1.;
        self.last_bits = -1.;
    }
    pub fn set_parameters(&mut self, p: EffectsParameters, sounding: bool) {
        let p = p.sanitized();
        if self.params == p {
            return;
        }
        self.params = p;
        if !sounding || (p.enabled == 0 && self.gate.current == 0.) {
            self.reset();
            return;
        }
        let samples = (self.rate * 0.005).round() as u32;
        self.gate.target(f64::from(p.enabled != 0), samples);
        self.drive.target(p.drive_db as f64, samples);
        self.bits.target(p.bits as f64, samples);
        self.divider.target(p.rate_divider as f64, samples);
        self.mix.target(p.mix as f64, samples);
    }
    pub fn process(&mut self, dry: [f32; 2]) -> [f32; 2] {
        self.process_with(dry, 0., 0.)
    }
    /// Per-sample offsets from the shared modulation matrix. Zero offsets are
    /// bit-identical to `process`.
    pub fn process_with(
        &mut self,
        dry: [f32; 2],
        drive_offset_db: f64,
        mix_offset: f64,
    ) -> [f32; 2] {
        if self.params.enabled == 0 && self.gate.current == 0. && !self.gate.running() {
            return dry;
        }
        let wet = self.gate.next() * (self.mix.next() + mix_offset).clamp(0., 1.);
        let db = (self.drive.next() + drive_offset_db).clamp(0., 24.);
        let bits = self.bits.next();
        let divider = self.divider.next();
        if wet == 0. {
            if self.params.enabled == 0 {
                self.reset();
            } else {
                self.clear_history();
            }
            return dry;
        }
        if db != self.last_drive {
            self.last_drive = db;
            self.k = 10_f64.powf(db / 20.) - 1.;
            self.normalization = if self.k < 1e-6 { 1. } else { self.k.tanh() };
        }
        if bits != self.last_bits {
            self.last_bits = bits;
            self.levels = 2_f64.powf(bits - 1.);
        }
        let mut sample = dry;
        if self.k >= 1e-6 {
            for x in &mut sample {
                *x = ((self.k * *x as f64).tanh() / self.normalization) as f32;
            }
        }
        if divider > 1.000001 {
            if self.phase >= 1. {
                self.held = sample;
                self.phase -= 1.;
            }
            self.phase += 1. / divider;
            sample = self.held;
        } else {
            self.phase = 1.;
            self.held = sample;
        }
        if bits < 23.99999 {
            for x in &mut sample {
                *x = ((*x as f64 * self.levels).round() / self.levels) as f32;
            }
        }
        if wet == 1. {
            return sample;
        }
        std::array::from_fn(|i| dry[i] + (sample[i] - dry[i]) * wet as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bypass_and_neutral_colour_are_exact_even_above_full_scale() {
        for enabled in [0, 1] {
            let mut fx = Effects::default();
            fx.set_parameters(
                EffectsParameters {
                    enabled,
                    ..Default::default()
                },
                false,
            );
            for x in [-32., -1., -0.12345, 0., 0.12345, 1., 32.] {
                assert_eq!(fx.process([x, -x]), [x, -x]);
            }
        }
    }
    #[test]
    fn quantizer_and_stereo_linked_hold_have_the_requested_steps() {
        let mut fx = Effects::default();
        fx.set_parameters(
            EffectsParameters {
                enabled: 1,
                bits: 4.,
                rate_divider: 4.,
                ..Default::default()
            },
            false,
        );
        for i in 0..16 {
            let x = i as f32 * 0.03125;
            let actual = fx.process([x, -x]);
            let held = (i / 4) as f32 * 0.125;
            assert_eq!(actual, [held, -held]);
        }
        fx.clear_history();
        assert_eq!(fx.process([0., 0.]), [0., 0.]);
    }
    #[test]
    fn drive_is_odd_finite_and_continuous_at_zero() {
        let mut fx = Effects::default();
        for drive in [0., 0.0001, 1., 6., 12., 24.] {
            fx.set_parameters(
                EffectsParameters {
                    enabled: 1,
                    drive_db: drive,
                    ..Default::default()
                },
                false,
            );
            let out = fx.process([0.25, -0.25]);
            assert!(out[0].is_finite());
            assert_eq!(out[0], -out[1]);
            if drive < 0.001 {
                assert!((out[0] - 0.25).abs() < 1e-6);
            } else {
                assert!(out[0] >= 0.25);
            }
            assert!(fx.process([32., -32.]).iter().all(|x| x.is_finite()));
        }
    }
    #[test]
    fn invalid_settings_remain_finite_and_dry_mix_is_exact() {
        let mut fx = Effects::default();
        fx.set_parameters(
            EffectsParameters {
                enabled: 1,
                drive_db: f32::NAN,
                bits: f32::INFINITY,
                rate_divider: f32::NAN,
                mix: 0.,
            },
            false,
        );
        assert_eq!(fx.process([2., -2.]), [2., -2.]);
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    #[test]
    fn bypass_reaches_exact_dry_after_five_ms_and_discards_held_history() {
        for rate in [8000., 44100., 48000., 192000., 384000.] {
            let mut fx = Effects::default();
            fx.prepare(rate);
            let p = EffectsParameters {
                enabled: 1,
                drive_db: 12.,
                bits: 8.,
                rate_divider: 8.,
                ..Default::default()
            };
            fx.set_parameters(p, true);
            let first = fx.process([0.3, -0.3]);
            assert!((first[0] - 0.3).abs() < 0.01);
            let n = (rate * 0.005).round() as usize;
            for _ in 1..n {
                fx.process([0.3, -0.3]);
            }
            let wet = fx.process([0.3, -0.3]);
            assert!(wet[0] > 0.3);
            fx.set_parameters(EffectsParameters { enabled: 0, ..p }, true);
            let first = fx.process([0.3, -0.3]);
            assert!((first[0] - wet[0]).abs() < 0.02);
            for _ in 1..n {
                fx.process([0.3, -0.3]);
            }
            assert_eq!(fx.process([0., 0.]), [0., 0.]);
            fx.set_parameters(p, true);
            assert_eq!(fx.process([0., 0.]), [0., 0.]);
        }
    }
    #[test]
    fn stereo_hold_does_not_leak_channels_and_clear_is_immediate() {
        let mut fx = Effects::default();
        fx.set_parameters(
            EffectsParameters {
                enabled: 1,
                drive_db: 24.,
                bits: 4.,
                rate_divider: 32.,
                ..Default::default()
            },
            false,
        );
        for _ in 0..10 {
            let out = fx.process([0., 0.5]);
            assert_eq!(out[0], 0.);
            assert!(out[1] > 0.);
        }
        fx.clear_history();
        assert_eq!(fx.process([0., 0.]), [0., 0.]);
    }
}
