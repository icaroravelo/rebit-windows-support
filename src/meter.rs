//! Audio-clock peak ballistics. This observes output without changing it.
#[derive(Clone, Copy)]
pub(crate) struct Meter {
    pub peak: f32,
    pub held_peak: f32,
    pub clips: u32,
    hold_remaining: u32,
    hold_samples: u32,
    decay: f32,
}
impl Default for Meter {
    fn default() -> Self {
        let mut meter = Self {
            peak: 0.,
            held_peak: 0.,
            clips: 0,
            hold_remaining: 0,
            hold_samples: 1,
            decay: 0.,
        };
        meter.prepare(44100.);
        meter
    }
}
impl Meter {
    pub fn prepare(&mut self, sample_rate: f64) {
        self.hold_samples = sample_rate.round() as u32;
        // 24 dB/second, with instantaneous attack and a one-second peak hold.
        self.decay = 10_f64.powf(-24. / (20. * sample_rate)) as f32;
        self.reset();
    }
    pub fn reset(&mut self) {
        self.peak = 0.;
        self.held_peak = 0.;
        self.hold_remaining = 0;
        // Keep the revision monotonic across STOP/state reset. Clip history
        // is acknowledged by the UI, independently of audio engine lifetime.
    }
    pub fn observe(&mut self, sample: f32) {
        let level = sample.abs();
        self.peak = level.max(self.peak * self.decay);
        if self.peak < 1e-6 {
            self.peak = 0.;
        }
        if level >= self.held_peak {
            self.held_peak = level;
            self.hold_remaining = self.hold_samples;
        } else if self.hold_remaining > 0 {
            self.hold_remaining -= 1;
        } else {
            self.held_peak = self.peak.max(self.held_peak * self.decay);
            if self.held_peak < 1e-6 {
                self.held_peak = 0.;
            }
        }
        if level >= 1. {
            self.clips = self.clips.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instant_attack_hold_and_decay_follow_audio_time_at_all_rates() {
        for rate in [8000., 44100., 48000., 192000., 384000.] {
            let mut meter = Meter::default();
            meter.prepare(rate);
            meter.observe(0.5);
            assert_eq!(meter.peak, 0.5);
            assert_eq!(meter.held_peak, 0.5);
            for _ in 0..rate as usize {
                meter.observe(0.);
            }
            let db = 20. * (meter.peak / 0.5).log10();
            assert!((db + 24.).abs() < 0.06, "rate {rate}: {db}");
            assert_eq!(meter.held_peak, 0.5);
            for _ in 0..rate as usize {
                meter.observe(0.);
            }
            let db = 20. * (meter.held_peak / 0.5).log10();
            assert!((db + 24.).abs() < 0.06);
        }
    }
    #[test]
    fn clipping_counts_include_both_polarities_and_survive_reset() {
        let mut m = Meter::default();
        for sample in [0.99, -1., 1., 2., -2., 0.] {
            m.observe(sample);
        }
        assert_eq!(m.clips, 4);
        assert_eq!(m.held_peak, 2.);
        m.reset();
        assert_eq!(m.peak, 0.);
        assert_eq!(m.held_peak, 0.);
        assert_eq!(m.clips, 4);
        m.clips = u32::MAX;
        m.observe(1.);
        assert_eq!(m.clips, 0);
    }
}
