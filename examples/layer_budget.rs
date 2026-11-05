//! Offline throughput check, not a realtime scheduling guarantee.
use rebit_rust::effects::EffectsParameters;
use rebit_rust::{
    layers::{LayerMix, LayerMixer},
    Engine, Parameters,
};
use std::{hint::black_box, time::Instant};
fn main() {
    for (label, voices, dual, dynamic, clean, fx) in [
        ("A Raw: 8 static", 8, false, false, false, false),
        ("A Clean: 8 static", 8, false, false, true, false),
        ("A+B Raw: 8 each", 8, true, false, false, false),
        ("A+B Clean: 8 each", 8, true, false, true, false),
        ("A+B Raw: 64 each + vibrato", 64, true, true, false, false),
        ("A+B Clean: 64 each + vibrato", 64, true, true, true, false),
        ("A+B Clean: 8 each + all FX", 8, true, false, true, true),
    ] {
        let mut times = Vec::new();
        for _ in 0..3 {
            let mut a = Engine::new(48000., 1);
            let mut b = Engine::new(48000., 2);
            let mut m = LayerMixer::default();
            m.prepare(48000.);
            let p = Parameters {
                max_poly: voices,
                gain: 0.01,
                vibrato_depth: if dynamic { 0.5 } else { 0. },
                vibrato_delay: 0.,
                ..Default::default()
            };
            a.set_parameters(p);
            b.set_parameters(p);
            b.set_tuning(-12.07);
            a.set_sound_mode(i32::from(clean));
            b.set_sound_mode(i32::from(clean));
            if fx {
                m.set_effects(EffectsParameters {
                    enabled: 1,
                    drive_db: 12.,
                    bits: 8.,
                    rate_divider: 6.,
                    mix: 0.7,
                });
            }
            if dual {
                m.set_mix(1, LayerMix::default());
            }
            for note in 36..36 + voices as u8 {
                a.midi(0x90, note, 100);
                b.midi(0x90, note, 100);
            }
            let mut left = [0.; 128];
            let mut right = [0.; 128];
            let start = Instant::now();
            for _ in 0..375 {
                m.render(&mut a, &mut b, &mut left, &mut right);
                black_box((&left, &right));
            }
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{label}: {:.2} ms per 1 s audio, {:.2}% of audio duration (median of 3)",
            times[1],
            times[1] / 10.
        );
    }
}
