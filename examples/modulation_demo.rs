//! Eight-second expression demo: aftertouch level, macro layer crossfade,
//! LFO pitch and pulse-width motion.
use rebit_rust::layers::{LayerMix, LayerMixer};
use rebit_rust::{Engine, LfoParameters, ModRoute, ModulationParameters, Parameters};
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
};
fn route(source: i32, destination: i32, amount: f32) -> ModRoute {
    ModRoute {
        source,
        destination,
        amount,
    }
}
fn modulation(macros: [f32; 4]) -> ModulationParameters {
    ModulationParameters {
        routes: [
            route(5, 1, 0.12),
            route(6, 2, 0.45),
            route(3, 3, 0.7),
            route(7, 4, 0.9),
        ],
        lfo: [
            LfoParameters {
                shape: 0,
                rate_hz: 5.2,
                retrigger: 0,
            },
            LfoParameters {
                shape: 1,
                rate_hz: 2.4,
                retrigger: 0,
            },
        ],
        macros,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .unwrap_or("dist/rebit-rust-mod-demo.wav".into());
    const RATE: usize = 48000;
    const SECTION: usize = RATE * 2;
    let mut a = Engine::new(RATE as f64, 11);
    let mut b = Engine::new(RATE as f64, 22);
    let mut mixer = LayerMixer::default();
    mixer.prepare(RATE as f64);
    a.set_parameters(Parameters {
        gain: 0.13,
        duty: 1,
        attack: 0.008,
        decay: 0.16,
        sustain: 0.7,
        release: 0.18,
        ..Default::default()
    });
    b.set_parameters(Parameters {
        osc: 1,
        gain: 0.13,
        attack: 0.02,
        decay: 0.2,
        sustain: 0.75,
        release: 0.25,
        ..Default::default()
    });
    b.set_tuning(-12.);
    mixer.set_mix(
        0,
        LayerMix {
            enabled: 1,
            gain: 0.85,
            pan: -0.25,
        },
    );
    mixer.set_mix(
        1,
        LayerMix {
            enabled: 1,
            gain: 0.6,
            pan: 0.3,
        },
    );
    let mut left = vec![0.; SECTION * 4];
    let mut right = vec![0.; left.len()];
    for section in 0..4 {
        let start = section * SECTION;
        let end = start + SECTION;
        if section > 0 {
            for note in [48, 55, 60, 64] {
                a.midi(0x80, note, 0);
                b.midi(0x80, note, 0);
            }
            let tail = (start + RATE / 10).min(end);
            mixer.render(
                &mut a,
                &mut b,
                &mut left[start..tail],
                &mut right[start..tail],
            );
            for note in [48, 55, 60, 64] {
                a.midi(0x90, note, 96);
                b.midi(0x90, note, 96);
            }
            let from = tail;
            for chunk in (from..end).collect::<Vec<_>>().chunks(256) {
                let progress = (chunk[0] - start) as f32 / SECTION as f32;
                let (pressure, macro1) = match section {
                    1 => ((progress * 127.) as u8, 0.),
                    2 => (0, progress),
                    _ => ((progress * 127.) as u8, progress),
                };
                a.midi(0xd0, pressure, 0);
                b.midi(0xd0, pressure, 0);
                let parameters = modulation([macro1, 0., 0., 0.]);
                a.set_modulation(parameters);
                b.set_modulation(parameters);
                let stop = (chunk[0] + chunk.len()).min(end);
                mixer.render(
                    &mut a,
                    &mut b,
                    &mut left[chunk[0]..stop],
                    &mut right[chunk[0]..stop],
                );
            }
        } else {
            for note in [48, 55, 60, 64] {
                a.midi(0x90, note, 96);
                b.midi(0x90, note, 96);
            }
            a.set_modulation(modulation([0.; 4]));
            b.set_modulation(modulation([0.; 4]));
            mixer.render(
                &mut a,
                &mut b,
                &mut left[start..end],
                &mut right[start..end],
            );
        }
    }
    let mut out = BufWriter::new(File::create(&path)?);
    let bytes = (left.len() * 4) as u32;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + bytes).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16_u32.to_le_bytes())?;
    out.write_all(&1_u16.to_le_bytes())?;
    out.write_all(&2_u16.to_le_bytes())?;
    out.write_all(&(RATE as u32).to_le_bytes())?;
    out.write_all(&(RATE as u32 * 4).to_le_bytes())?;
    out.write_all(&4_u16.to_le_bytes())?;
    out.write_all(&16_u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    for (l, r) in left.into_iter().zip(right) {
        for x in [l, r] {
            assert!(x.is_finite());
            out.write_all(&((x.clamp(-1., 1.) * 32767.).round() as i16).to_le_bytes())?;
        }
    }
    out.flush()?;
    println!("Rendered {path}: base, aftertouch swell, macro A/B crossfade, combined motion");
    Ok(())
}
