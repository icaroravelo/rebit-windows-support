//! Four identical phrases: Raw, Clean, Clean+saturation, Clean+crush.
use rebit_rust::{effects::EffectsParameters, layers::LayerMixer, Engine, Parameters};
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .unwrap_or("dist/rebit-rust-colour-demo.wav".into());
    const RATE: usize = 48000;
    let mut left = vec![0.; RATE * 8];
    let mut right = vec![0.; left.len()];
    for section in 0..4 {
        let mut a = Engine::new(RATE as f64, 1);
        let mut b = Engine::new(RATE as f64, 2);
        let mut m = LayerMixer::default();
        m.prepare(RATE as f64);
        a.set_parameters(Parameters {
            gain: 0.16,
            duty: 1,
            max_poly: 1,
            behavior: 3,
            attack: 0.005,
            decay: 0.05,
            sustain: 0.65,
            release: 0.035,
            ..Default::default()
        });
        a.set_sound_mode(i32::from(section > 0));
        if section == 2 {
            m.set_effects(EffectsParameters {
                enabled: 1,
                drive_db: 12.,
                ..Default::default()
            });
        }
        if section == 3 {
            m.set_effects(EffectsParameters {
                enabled: 1,
                bits: 8.,
                rate_divider: 8.,
                ..Default::default()
            });
        }
        for (i, note) in [90, 94, 97, 102, 97, 94, 90, 85].into_iter().enumerate() {
            let start = section * RATE * 2 + i * RATE / 4;
            let held = start + RATE / 5;
            let end = start + RATE / 4;
            a.midi(0x90, note, 110);
            m.render(
                &mut a,
                &mut b,
                &mut left[start..held],
                &mut right[start..held],
            );
            a.midi(0x80, note, 0);
            m.render(&mut a, &mut b, &mut left[held..end], &mut right[held..end]);
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
    println!("Rendered {path}: 0–2 s Raw, 2–4 s Clean, 4–6 s saturation, 6–8 s crush");
    Ok(())
}
