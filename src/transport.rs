//! Block-provided host position, extrapolated between callbacks without allocation.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockParameters {
    pub host_sync: i32,
    pub sequence_rate: i32,
}
impl Default for ClockParameters {
    fn default() -> Self {
        Self {
            host_sync: 1,
            sequence_rate: 0,
        }
    }
}
impl ClockParameters {
    pub fn sanitized(self) -> Self {
        Self {
            host_sync: i32::from(self.host_sync != 0),
            sequence_rate: self.sequence_rate.clamp(0, 7),
        }
    }
    pub fn sequence_beats(self) -> Option<f64> {
        match self.sequence_rate {
            1 => Some(1.),
            2 => Some(0.5),
            3 => Some(0.25),
            4 => Some(0.125),
            5 => Some(0.0625),
            6 => Some(1. / 3.),
            7 => Some(1. / 6.),
            _ => None,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TransportInfo {
    pub ppq: f64,
    pub bpm: f64,
    pub loop_start: f64,
    pub loop_end: f64,
    pub valid: i32,
    pub playing: i32,
    pub looping: i32,
    pub reserved: i32,
}
impl Default for TransportInfo {
    fn default() -> Self {
        Self {
            ppq: 0.,
            bpm: 120.,
            loop_start: 0.,
            loop_end: 0.,
            valid: 0,
            playing: 0,
            looping: 0,
            reserved: 0,
        }
    }
}
impl TransportInfo {
    pub fn sanitized(mut self) -> Self {
        self.valid = i32::from(
            self.valid != 0
                && self.ppq.is_finite()
                && self.ppq.abs() <= 1e9
                && self.bpm.is_finite()
                && self.bpm > 0.,
        );
        self.playing = i32::from(self.playing != 0);
        self.bpm = if self.bpm.is_finite() {
            self.bpm.clamp(1., 999.)
        } else {
            120.
        };
        self.looping = i32::from(
            self.looping != 0
                && self.loop_start.is_finite()
                && self.loop_end.is_finite()
                && self.loop_start.abs() <= 1e9
                && self.loop_end.abs() <= 1e9
                && self.loop_end - self.loop_start >= 1e-6,
        );
        self.reserved = 0;
        self
    }
}
/// Swing pairs are anchored at PPQ zero, including negative preroll positions.
/// The small tolerance absorbs floating point roundoff, not a whole audio sample.
pub fn swung_step(beat: f64, beats: f64, swing: f64) -> (i64, f64, f64) {
    let pair = ((beat + 1e-9) / (beats * 2.)).floor();
    let pair_start = pair * beats * 2.;
    let middle = pair_start + beats * (1. + swing);
    if beat + 1e-9 >= middle {
        (
            (pair as i64) * 2 + 1,
            (beat - middle).max(0.),
            beats * (1. - swing),
        )
    } else {
        (
            (pair as i64) * 2,
            (beat - pair_start).max(0.),
            beats * (1. + swing),
        )
    }
}
