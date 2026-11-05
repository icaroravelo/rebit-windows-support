//! Polynomial edge/corner correction for Clean pulse and triangle oscillators.
//! Raw oscillators remain in engine.rs. No tables, allocation or lookahead.
//! Background: Esqueda/Välimäki/Bilbao, DAFx-16, "Rounding Corners with BLAMP"
//! https://research.spa.aalto.fi/publications/papers/dafx16-blamp/

fn blep(phase: f64, increment: f64) -> f64 {
    if increment <= 0. {
        return 0.;
    }
    if phase < increment {
        let t = phase / increment;
        2. * t - t * t - 1.
    } else if phase > 1. - increment {
        let t = (phase - 1.) / increment;
        t * t + 2. * t + 1.
    } else {
        0.
    }
}
// Integral of the two-sided quadratic BLEP residual, divided by increment.
fn blamp(phase: f64, increment: f64) -> f64 {
    if increment <= 0. {
        return 0.;
    }
    let t = if phase < increment {
        1. - phase / increment
    } else if phase > 1. - increment {
        1. + (phase - 1.) / increment
    } else {
        return 0.;
    };
    t * t * t / 3.
}
pub fn clean_pulse(phase: f64, increment: f64, width: f64) -> f64 {
    let raw = if phase < width { -1. } else { 1. };
    let edge = if phase >= width {
        phase - width
    } else {
        phase - width + 1.
    };
    raw - blep(phase, increment) + blep(edge, increment)
}
pub fn clean_triangle(phase: f64, increment: f64) -> f64 {
    let opposite = if phase >= 0.5 {
        phase - 0.5
    } else {
        phase + 0.5
    };
    1. - 4. * (phase - 0.5).abs()
        + 4. * increment * (blamp(phase, increment) - blamp(opposite, increment))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edges_and_corners_are_smoothed_without_dc_or_extreme_values() {
        assert_eq!(clean_pulse(0., 0.01, 0.5), 0.);
        assert!(clean_triangle(0., 0.01) > -1.);
        for increment in [0., 0.00001, 0.01, 0.125, 0.25, 0.49] {
            for width in [0.125, 0.25, 0.5] {
                let mut pulse_dc = 0.;
                let mut tri_dc = 0.;
                for i in 0..16384 {
                    let phase = i as f64 / 16384.;
                    let p = clean_pulse(phase, increment, width);
                    let t = clean_triangle(phase, increment);
                    assert!(p.is_finite() && p.abs() <= 1.000001);
                    assert!(t.is_finite() && t.abs() <= 1.000001);
                    pulse_dc += p;
                    tri_dc += t;
                }
                assert!((pulse_dc / 16384. - (1. - 2. * width)).abs() < 1e-6);
                assert!(tri_dc.abs() < 1e-6);
            }
        }
    }
}

#[cfg(test)]
mod spectral_tests {
    use super::*;
    fn alias_ratio(samples: &[f64], bin: usize) -> f64 {
        let n = samples.len();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let total = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        let mut harmonic = 0.;
        for h in 1..=(n / 2 - 1) / bin {
            let (mut real, mut imag) = (0., 0.);
            for (i, x) in samples.iter().enumerate() {
                let phase = std::f64::consts::TAU * (i * h * bin % n) as f64 / n as f64;
                real += x * phase.cos();
                imag += x * phase.sin();
            }
            harmonic += 2. * (real * real + imag * imag) / (n * n) as f64;
        }
        (total - harmonic).max(1e-30) / harmonic
    }
    #[test]
    fn clean_reduces_measured_nonharmonic_alias_energy() {
        const N: usize = 8192;
        for bin in [509, 1365, 2047] {
            let increment = bin as f64 / N as f64;
            for shape in 0..4 {
                let mut raw = vec![0.; N];
                let mut clean = vec![0.; N];
                for i in 0..N {
                    let phase = (i * bin % N) as f64 / N as f64;
                    if shape < 3 {
                        let width = [0.125, 0.25, 0.5][shape];
                        raw[i] = if phase < width { -1. } else { 1. };
                        clean[i] = clean_pulse(phase, increment, width);
                    } else {
                        raw[i] = ((1. - 4. * (phase - 0.5).abs()) * 7.5).round() / 7.5;
                        clean[i] = clean_triangle(phase, increment);
                    }
                }
                let before = alias_ratio(&raw, bin);
                let after = alias_ratio(&clean, bin);
                assert!(
                    after < before * 0.5,
                    "shape {shape}, bin {bin}: raw {} dB, clean {} dB",
                    10. * before.log10(),
                    10. * after.log10()
                );
            }
        }
    }
}
