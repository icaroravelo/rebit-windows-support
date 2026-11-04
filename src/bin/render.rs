use rebit_rust::{ArpParameters, Engine, Parameters, Sequence, SequenceBank};
use std::{
    env,
    fs::File,
    io::{self, Write},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--help") {
        println!("Usage: rebit-render [output.wav] [pulse|triangle|noise] [--arp]\nRenders a four-second 48 kHz demo; no audio device required.");
        return Ok(());
    }
    if args.len() > 4 || args.get(3).is_some_and(|a| a != "--arp") {
        return Err("Too many arguments; use --help".into());
    }
    let osc = match args.get(2).map(String::as_str).unwrap_or("pulse") {
        "pulse" => 0,
        "triangle" => 1,
        "noise" => 2,
        _ => return Err("Waveform must be pulse, triangle, or noise".into()),
    };
    let arp_demo = args.get(3).is_some();
    let rate = 48_000u32;
    let mut engine = Engine::new(rate as f64, 42);
    engine.set_parameters(Parameters {
        osc,
        attack: 0.005,
        decay: 0.12,
        sustain: 0.6,
        release: 0.15,
        gain: 0.22,
        pitch_enabled: i32::from(!arp_demo),
        ..Parameters::default()
    });
    let mut bank = SequenceBank::default();
    bank.0[1] = Sequence::parse("[0 x 4,7 x 4,12 x 4,7 x 4]|0", -64, 63)?;
    engine.set_sequences(bank);
    let mut audio = vec![0.; rate as usize * 4];
    if arp_demo {
        engine.set_arp_parameters(ArpParameters {
            enabled: 1,
            pattern: 2,
            octaves: 2,
            swing: 0.18,
            ..ArpParameters::default()
        });
        for (i, chord) in [[60, 63, 67], [53, 56, 60]].iter().enumerate() {
            let start = i * rate as usize * 2;
            let end = (i + 1) * rate as usize * 2;
            let held_end = end - rate as usize / 5;
            for note in chord {
                engine.midi(0x90, *note, 105);
            }
            engine.render(&mut audio[start..held_end]);
            for note in chord {
                engine.midi(0x80, *note, 0);
            }
            engine.render(&mut audio[held_end..end]);
        }
    } else {
        for (i, note) in [60, 64, 67, 72, 67, 64, 60, 55].iter().enumerate() {
            let start = i * rate as usize / 2;
            let held_end = start + rate as usize * 3 / 8;
            let end = start + rate as usize / 2;
            engine.midi(0x90, *note, 105);
            engine.render(&mut audio[start..held_end]);
            engine.midi(0x80, *note, 0);
            engine.render(&mut audio[held_end..end]);
        }
    }
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("rebit-rust-demo.wav");
    write_wav(path, rate, &audio)?;
    println!("Rendered {path} (48 kHz, stereo PCM16, 4 seconds)");
    Ok(())
}
fn write_wav(path: &str, rate: u32, audio: &[f32]) -> io::Result<()> {
    let mut file = io::BufWriter::new(File::create(path)?);
    let size = audio.len() as u32 * 4;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + size).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&rate.to_le_bytes())?;
    file.write_all(&(rate * 4).to_le_bytes())?;
    file.write_all(&4u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&size.to_le_bytes())?;
    for sample in audio {
        let pcm = (sample.clamp(-1., 1.) * i16::MAX as f32).round() as i16;
        file.write_all(&pcm.to_le_bytes())?;
        file.write_all(&pcm.to_le_bytes())?;
    }
    file.flush()
}
