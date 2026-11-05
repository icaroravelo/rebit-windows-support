//! Allocation-free modulation sources and routing.
//! Per-voice routes use the voice's own LFO phase; the shared layer balance and
//! effect destinations are stepped once per sample by the layer mixer.
use std::f64::consts::TAU;

pub const MOD_ROUTES: usize = 4;
pub const MOD_SOURCES: i32 = 13;
pub const MOD_DESTINATIONS: i32 = 7;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModRoute {
    pub source: i32,
    pub destination: i32,
    pub amount: f32,
}
impl Default for ModRoute {
    fn default() -> Self {
        Self {
            source: 0,
            destination: 0,
            amount: 0.,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LfoParameters {
    pub shape: i32,
    pub rate_hz: f32,
    pub retrigger: i32,
}
impl Default for LfoParameters {
    fn default() -> Self {
        Self {
            shape: 0,
            rate_hz: 1.,
            retrigger: 0,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModulationParameters {
    pub routes: [ModRoute; MOD_ROUTES],
    pub lfo: [LfoParameters; 2],
    pub macros: [f32; 4],
}
impl Default for ModulationParameters {
    fn default() -> Self {
        Self {
            routes: [ModRoute::default(); MOD_ROUTES],
            lfo: [LfoParameters::default(); 2],
            macros: [0.; 4],
        }
    }
}
fn finite(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
impl ModulationParameters {
    pub fn sanitized(mut self) -> Self {
        for route in &mut self.routes {
            route.source = route.source.clamp(0, MOD_SOURCES - 1);
            route.destination = route.destination.clamp(0, MOD_DESTINATIONS - 1);
            route.amount = finite(route.amount, -1., 1., 0.);
            if route.source == 0 || route.destination == 0 {
                route.amount = 0.;
            }
        }
        for lfo in &mut self.lfo {
            lfo.shape = lfo.shape.clamp(0, 5);
            lfo.rate_hz = finite(lfo.rate_hz, 0.01, 20., 1.);
            lfo.retrigger = i32::from(lfo.retrigger != 0);
        }
        for macro_value in &mut self.macros {
            *macro_value = finite(*macro_value, 0., 1., 0.);
        }
        self
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct LfoState {
    phase: f64,
    hold: f64,
    rng: u32,
}
impl Default for LfoState {
    fn default() -> Self {
        Self {
            phase: 0.,
            hold: 0.,
            rng: 1,
        }
    }
}
impl LfoState {
    pub(crate) fn new(seed: u32) -> Self {
        let mut state = Self {
            phase: 0.,
            hold: 0.,
            rng: seed.max(1),
        };
        state.hold = state.random();
        state
    }
    pub(crate) fn reset(&mut self, seed: u32) {
        self.phase = 0.;
        self.rng = seed.max(1);
        self.hold = self.random();
    }
    fn random(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        ((x >> 8) as f64 / 16_777_215.) * 2. - 1.
    }
    pub(crate) fn advance(&mut self, rate: f64, hz: f64) {
        self.phase += hz / rate.max(1.);
        if self.phase >= 1. {
            self.phase -= self.phase.floor();
            self.hold = self.random();
        }
    }
    pub(crate) fn value(&self, shape: i32) -> f64 {
        match shape {
            1 => 1. - 4. * (self.phase - 0.5).abs(),
            2 => {
                if self.phase < 0.5 {
                    1.
                } else {
                    -1.
                }
            }
            3 => 2. * self.phase - 1.,
            4 => 1. - 2. * self.phase,
            5 => self.hold,
            _ => (self.phase * TAU).sin(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct GlobalModulation {
    pub balance: f64,
    pub drive_db: f64,
    pub mix: f64,
}
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SourceValues {
    pub velocity: f64,
    pub wheel: f64,
    pub aftertouch: f64,
    pub timbre: f64,
    pub lfo: [f64; 2],
    pub macros: [f64; 4],
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct DestinationValues {
    pub pitch: f64,
    pub duty: f64,
    pub level: f64,
    pub balance: f64,
    pub drive_db: f64,
    pub mix: f64,
}
impl DestinationValues {
    fn neutral() -> Self {
        Self {
            pitch: 0.,
            duty: 0.,
            level: 1.,
            ..Self::default()
        }
    }
}
fn source_value(source: i32, values: &SourceValues) -> f64 {
    match source {
        1 => values.velocity,
        2 => values.wheel,
        3 => values.aftertouch,
        4 => values.timbre,
        5 => values.lfo[0],
        6 => values.lfo[1],
        7 => values.macros[0],
        8 => values.macros[1],
        9 => values.macros[2],
        10 => values.macros[3],
        11 => values.lfo[0] * 0.5 + 0.5,
        12 => values.lfo[1] * 0.5 + 0.5,
        _ => 0.,
    }
}
pub(crate) fn evaluate(
    routes: &[ModRoute; MOD_ROUTES],
    values: &SourceValues,
) -> DestinationValues {
    let mut out = DestinationValues::neutral();
    for route in routes {
        let source = source_value(route.source, values);
        let amount = route.amount as f64;
        match route.destination {
            1 => out.pitch += amount * source * 12.,
            2 => out.duty += amount * source * 0.5,
            3 => out.level *= (1. + amount * source).clamp(0., 2.),
            4 => out.balance += amount * source,
            5 => out.drive_db += amount * source * 12.,
            6 => out.mix += amount * source,
            _ => (),
        }
    }
    out.balance = out.balance.clamp(-1., 1.);
    out.mix = out.mix.clamp(-1., 1.);
    out.drive_db = out.drive_db.clamp(-24., 24.);
    out
}
pub(crate) struct Modulation {
    pub parameters: ModulationParameters,
    pub lfo: [LfoState; 2],
    pub globals: GlobalModulation,
}
impl Modulation {
    pub(crate) fn new(seed: u32) -> Self {
        Self {
            parameters: ModulationParameters::default(),
            lfo: [
                LfoState::new(seed ^ 0x4c46_4f31),
                LfoState::new(seed ^ 0x4c46_4f32),
            ],
            globals: GlobalModulation::default(),
        }
    }
    pub(crate) fn reset(&mut self, seed: u32) {
        self.lfo[0].reset(seed ^ 0x4c46_4f31);
        self.lfo[1].reset(seed ^ 0x4c46_4f32);
        self.globals = GlobalModulation::default();
    }
    pub(crate) fn retrigger(&mut self) {
        for (index, lfo) in self.lfo.iter_mut().enumerate() {
            if self.parameters.lfo[index].retrigger != 0 {
                lfo.phase = 0.;
            }
        }
    }
    pub(crate) fn advance(&mut self, rate: f64) {
        for (index, lfo) in self.lfo.iter_mut().enumerate() {
            lfo.advance(rate, self.parameters.lfo[index].rate_hz as f64);
        }
    }
    pub(crate) fn values(&self) -> [f64; 2] {
        [
            self.lfo[0].value(self.parameters.lfo[0].shape),
            self.lfo[1].value(self.parameters.lfo[1].shape),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shapes_are_bounded_and_sample_hold_advances_once_per_cycle() {
        let mut state = LfoState::new(7);
        for shape in 0..6 {
            state.phase = 0.125;
            let first = state.value(shape);
            assert!(first.is_finite() && (-1. ..=1.).contains(&first));
            state.phase = 0.75;
            assert!(state.value(shape).is_finite());
        }
        let mut state = LfoState::new(7);
        let before = state.value(5);
        state.advance(100., 1.);
        assert_eq!(state.value(5), before);
        state.advance(100., 99.);
        assert_ne!(state.value(5), before);
    }
    #[test]
    fn routes_are_bounded_and_off_routes_are_neutral() {
        let parameters = ModulationParameters {
            routes: [
                ModRoute {
                    source: 999,
                    destination: -3,
                    amount: 9.,
                },
                ModRoute {
                    source: 1,
                    destination: 1,
                    amount: 1.,
                },
                ModRoute {
                    source: 200,
                    destination: 2,
                    amount: f32::NAN,
                },
                ModRoute::default(),
            ],
            lfo: [
                LfoParameters {
                    shape: 99,
                    rate_hz: f32::INFINITY,
                    retrigger: 7,
                },
                LfoParameters::default(),
            ],
            macros: [f32::NAN, -1., 2., 0.5],
        }
        .sanitized();
        assert_eq!(parameters.routes[0].amount, 0.);
        assert_eq!(parameters.routes[1].amount, 1.);
        assert_eq!(parameters.routes[2].amount, 0.);
        assert_eq!(parameters.routes[3].amount, 0.);
        assert_eq!(parameters.lfo[0].shape, 5);
        assert_eq!(parameters.lfo[0].rate_hz, 1.);
        assert_eq!(parameters.lfo[0].retrigger, 1);
        assert_eq!(parameters.macros, [0., 0., 1., 0.5]);
        let out = evaluate(&[ModRoute::default(); MOD_ROUTES], &SourceValues::default());
        assert_eq!(out, DestinationValues::neutral());
    }
    #[test]
    fn sources_and_destinations_apply_expected_scales() {
        let values = SourceValues {
            velocity: 0.5,
            wheel: 1.,
            aftertouch: 0.25,
            timbre: 0.75,
            lfo: [-1., 1.],
            macros: [0.5, 0., 0., 0.],
        };
        let routes = [
            ModRoute {
                source: 1,
                destination: 1,
                amount: 1.,
            },
            ModRoute {
                source: 2,
                destination: 2,
                amount: -1.,
            },
            ModRoute {
                source: 3,
                destination: 3,
                amount: 0.5,
            },
            ModRoute {
                source: 5,
                destination: 6,
                amount: 1.,
            },
        ];
        let out = evaluate(&routes, &values);
        assert!((out.pitch - 6.).abs() < 1e-9);
        assert!((out.duty + 0.5).abs() < 1e-9);
        assert!((out.level - 1.125).abs() < 1e-9);
        assert!((out.mix + 1.).abs() < 1e-9);
    }
}
