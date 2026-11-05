//! Emit coherent actual-engine samples for scripts/analyze_quality.py.
use rebit_rust::{Engine, Parameters};
use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(env::args().nth(1).unwrap_or("dist/quality-analysis".into()));
    fs::create_dir_all(&dir)?;
    let mut index = BufWriter::new(File::create(dir.join("index.csv"))?);
    writeln!(index, "file,shape,mode,bin,rate,frames")?;
    const N: usize = 32768;
    let rate = 48000.;
    for (label, osc, duty) in [
        ("pulse125", 0, 0),
        ("pulse25", 0, 1),
        ("pulse50", 0, 2),
        ("triangle", 1, 0),
    ] {
        for bin in [427, 2047, 5461] {
            for (mode, clean) in [("raw", 0), ("clean", 1)] {
                let mut e = Engine::new(rate, 1);
                e.set_parameters(Parameters {
                    osc,
                    duty,
                    gain: 1.,
                    ..Default::default()
                });
                e.set_sound_mode(clean);
                let hz = bin as f64 * rate / N as f64;
                let note_hz = 440. * (27_f64 / 12.).exp2();
                e.set_tuning(12. * (hz / note_hz).log2());
                e.midi(0x90, 96, 127);
                let mut samples = vec![0.; N];
                e.render(&mut samples);
                let name = format!("{label}-{bin}-{mode}.f32");
                let mut file = BufWriter::new(File::create(dir.join(&name))?);
                for sample in samples {
                    file.write_all(&sample.to_le_bytes())?;
                }
                writeln!(index, "{name},{label},{mode},{bin},{rate},{N}")?;
            }
        }
    }
    println!("Emitted coherent engine data in {}", dir.display());
    Ok(())
}
