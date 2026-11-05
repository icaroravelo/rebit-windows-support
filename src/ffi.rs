//! The only unsafe code in the Rust library. Opaque handles are exclusively owned
//! by the native processor. Its audio thread owns Engine; textMutex owns the
//! canonical bank, and sequenceLock transfers ownership of pending banks.
#![allow(clippy::missing_safety_doc)]
use crate::effects::EffectsParameters;
use crate::layers::{LayerMix, LayerMixer};
use crate::{Engine, Parameters, Sequence, SequenceBank};
use std::{ptr, slice};

#[no_mangle]
pub unsafe extern "C" fn rebit_engine_sound_mode(engine: *mut Engine, mode: i32) {
    if let Some(e) = engine.as_mut() {
        e.set_sound_mode(mode);
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_effects(
    mixer: *mut LayerMixer,
    parameters: *const EffectsParameters,
) {
    if let (Some(m), Some(p)) = (mixer.as_mut(), parameters.as_ref()) {
        m.set_effects(*p);
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_clear_effects(mixer: *mut LayerMixer) {
    if let Some(m) = mixer.as_mut() {
        m.clear_effects_history();
    }
}

#[no_mangle]
pub extern "C" fn rebit_engine_new_seeded(seed: u32) -> *mut Engine {
    Box::into_raw(Box::new(Engine::new(44100., seed)))
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_tuning(engine: *mut Engine, semitones: f64) {
    if let Some(engine) = engine.as_mut() {
        engine.set_tuning(semitones);
    }
}
#[no_mangle]
pub extern "C" fn rebit_mixer_new() -> *mut LayerMixer {
    Box::into_raw(Box::default())
}
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_free(mixer: *mut LayerMixer) {
    if !mixer.is_null() {
        drop(Box::from_raw(mixer));
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_prepare(mixer: *mut LayerMixer, rate: f64) {
    if let Some(m) = mixer.as_mut() {
        m.prepare(rate);
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_reset(mixer: *mut LayerMixer) {
    if let Some(m) = mixer.as_mut() {
        m.reset();
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_parameters(
    mixer: *mut LayerMixer,
    layer: usize,
    settings: *const LayerMix,
) {
    if let (Some(m), Some(s)) = (mixer.as_mut(), settings.as_ref()) {
        m.set_mix(layer, *s);
    }
}
/// Engines and output buffers must be distinct and exclusively audio-owned.
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_render(
    mixer: *mut LayerMixer,
    first: *mut Engine,
    second: *mut Engine,
    left: *mut f32,
    right: *mut f32,
    count: usize,
) {
    if count == 0 || left.is_null() || left == right || first == second {
        return;
    }
    if let (Some(m), Some(a), Some(b)) = (mixer.as_mut(), first.as_mut(), second.as_mut()) {
        if right.is_null() {
            m.render_mono(a, b, slice::from_raw_parts_mut(left, count));
        } else {
            m.render(
                a,
                b,
                slice::from_raw_parts_mut(left, count),
                slice::from_raw_parts_mut(right, count),
            );
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_playback(
    mixer: *const LayerMixer,
    first: *const Engine,
    second: *const Engine,
    info: *mut crate::PlaybackInfo,
) {
    if let Some(info) = info.as_mut() {
        *info = match (mixer.as_ref(), first.as_ref(), second.as_ref()) {
            (Some(m), Some(a), Some(b)) => m.playback(a, b),
            _ => crate::PlaybackInfo::default(),
        };
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_mixer_layer_playback(
    mixer: *const LayerMixer,
    layer: usize,
    engine: *const Engine,
    info: *mut crate::PlaybackInfo,
) {
    if let Some(info) = info.as_mut() {
        *info = match (mixer.as_ref(), engine.as_ref()) {
            (Some(m), Some(e)) => m.layer_playback(layer, e),
            _ => crate::PlaybackInfo::default(),
        };
    }
}

#[no_mangle]
pub extern "C" fn rebit_engine_new() -> *mut Engine {
    Box::into_raw(Box::new(Engine::default()))
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_free(engine: *mut Engine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_prepare(engine: *mut Engine, rate: f64) {
    if let Some(e) = engine.as_mut() {
        e.prepare(rate);
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_reset(engine: *mut Engine) {
    if let Some(e) = engine.as_mut() {
        e.reset();
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_parameters(
    engine: *mut Engine,
    parameters: *const Parameters,
    bpm: f64,
) {
    if let (Some(e), Some(p)) = (engine.as_mut(), parameters.as_ref()) {
        e.set_parameters(*p);
        e.set_tempo(bpm);
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_midi(engine: *mut Engine, status: u8, data1: u8, data2: u8) {
    if let Some(e) = engine.as_mut() {
        e.midi(status, data1, data2);
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_render(engine: *mut Engine, output: *mut f32, count: usize) {
    if count == 0 || output.is_null() {
        return;
    }
    if let Some(e) = engine.as_mut() {
        e.render(slice::from_raw_parts_mut(output, count));
    }
}
/// Audio-thread only. UI reads a native atomic copy, never this engine pointer.
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_playback(
    engine: *const Engine,
    info: *mut crate::engine::PlaybackInfo,
) {
    if let Some(info) = info.as_mut() {
        *info = engine.as_ref().map_or(
            crate::engine::PlaybackInfo::default(),
            Engine::playback_info,
        );
    }
}
#[no_mangle]
pub extern "C" fn rebit_bank_new() -> *mut SequenceBank {
    Box::into_raw(Box::default())
}
#[no_mangle]
pub unsafe extern "C" fn rebit_bank_clone(bank: *const SequenceBank) -> *mut SequenceBank {
    bank.as_ref()
        .map_or(ptr::null_mut(), |b| Box::into_raw(Box::new(b.clone())))
}
#[no_mangle]
pub unsafe extern "C" fn rebit_bank_free(bank: *mut SequenceBank) {
    if !bank.is_null() {
        drop(Box::from_raw(bank));
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_bank_set(
    bank: *mut SequenceBank,
    lane: usize,
    text: *const u8,
    length: usize,
    error: *mut u8,
    capacity: usize,
) -> bool {
    let result = (|| {
        let bank = bank.as_mut().ok_or("Missing sequence bank".to_owned())?;
        if lane >= 3 || text.is_null() {
            return Err("Invalid sequence input".into());
        }
        let input = std::str::from_utf8(slice::from_raw_parts(text, length))
            .map_err(|_| "Sequence must be UTF-8".to_owned())?;
        let (min, max) = match lane {
            0 => (0, 15),
            1 => (-64, 63),
            _ => (0, 2),
        };
        let sequence = Sequence::parse(input, min, max).map_err(|e| e.to_string())?;
        bank.0[lane] = sequence;
        Ok::<_, String>(())
    })();
    if !error.is_null() && capacity > 0 {
        let message = result.as_ref().err().map_or("", String::as_str).as_bytes();
        let n = message.len().min(capacity - 1);
        ptr::copy_nonoverlapping(message.as_ptr(), error, n);
        *error.add(n) = 0;
    }
    result.is_ok()
}
/// Swap Box ownership only: no sequence copying, allocation, or destruction.
/// The caller must eventually free the returned bank outside the audio thread.
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_swap_bank(
    engine: *mut Engine,
    bank: *mut SequenceBank,
) -> *mut SequenceBank {
    if engine.is_null() || bank.is_null() {
        return bank;
    }
    let e = &mut *engine;
    let old = std::mem::replace(&mut e.sequences, Box::from_raw(bank));
    e.rebind_sequences(&old);
    Box::into_raw(old)
}

/// Read-only control-thread view of a bank. Engine and UI never share a bank.
#[repr(C)]
pub struct SequenceInfo {
    count: usize,
    loop_start: i32,
    release_start: i32,
}
#[no_mangle]
pub unsafe extern "C" fn rebit_bank_read(
    bank: *const SequenceBank,
    lane: usize,
    values: *mut i16,
    capacity: usize,
    info: *mut SequenceInfo,
) -> bool {
    let Some(bank) = bank.as_ref() else {
        return false;
    };
    let Some(seq) = bank.0.get(lane) else {
        return false;
    };
    let Some(info) = info.as_mut() else {
        return false;
    };
    *info = SequenceInfo {
        count: seq.values.len(),
        loop_start: seq.loop_start.map_or(-1, |v| v as i32),
        release_start: seq.release_start.map_or(-1, |v| v as i32),
    };
    if !values.is_null() && capacity > 0 {
        ptr::copy_nonoverlapping(seq.values.as_ptr(), values, capacity.min(seq.values.len()));
    }
    true
}
/// Caller supplies room for three i32 indices; audio thread only.
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_positions(engine: *const Engine, positions: *mut i32) {
    if let Some(engine) = engine.as_ref() {
        if !positions.is_null() {
            ptr::copy_nonoverlapping(engine.sequence_positions().as_ptr(), positions, 3);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rebit_engine_arp_parameters(
    engine: *mut Engine,
    parameters: *const crate::ArpParameters,
) {
    if let (Some(engine), Some(parameters)) = (engine.as_mut(), parameters.as_ref()) {
        engine.set_arp_parameters(*parameters);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rebit_engine_clock_parameters(
    engine: *mut Engine,
    parameters: *const crate::ClockParameters,
) {
    if let (Some(e), Some(p)) = (engine.as_mut(), parameters.as_ref()) {
        e.set_clock_parameters(*p);
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_transport(
    engine: *mut Engine,
    transport: *const crate::TransportInfo,
) {
    if let (Some(e), Some(t)) = (engine.as_mut(), transport.as_ref()) {
        e.set_transport(*t);
    }
}
#[no_mangle]
pub unsafe extern "C" fn rebit_engine_clock_status(engine: *const Engine) -> i32 {
    engine.as_ref().map_or(0, Engine::clock_status)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colour_abi_and_null_handling() {
        assert_eq!(std::mem::size_of::<EffectsParameters>(), 20);
        assert_eq!(std::mem::align_of::<EffectsParameters>(), 4);
        let mut e = Engine::default();
        let mut mixer = LayerMixer::default();
        unsafe {
            rebit_engine_sound_mode(&mut e, 1);
            assert_eq!(e.sound_mode(), 1);
            rebit_engine_sound_mode(ptr::null_mut(), 1);
            rebit_mixer_effects(ptr::null_mut(), &EffectsParameters::default());
            rebit_mixer_effects(&mut mixer, ptr::null());
            rebit_mixer_clear_effects(ptr::null_mut());
        }
    }
    #[test]
    fn layer_ffi_layout_stereo_mono_and_alias_guards() {
        assert_eq!(std::mem::size_of::<LayerMix>(), 12);
        assert_eq!(std::mem::align_of::<LayerMix>(), 4);
        let mut a = Engine::new(8000., 1);
        let mut b = Engine::new(8000., 2);
        let mut mixer = LayerMixer::default();
        mixer.prepare(8000.);
        a.midi(0x90, 0, 127);
        let mut left = [7.; 8];
        let mut right = [7.; 8];
        unsafe {
            let first = &mut a as *mut Engine;
            rebit_mixer_render(
                &mut mixer,
                first,
                first,
                left.as_mut_ptr(),
                right.as_mut_ptr(),
                8,
            );
            assert_eq!(left, [7.; 8]);
            rebit_mixer_parameters(
                &mut mixer,
                0,
                &LayerMix {
                    pan: -1.,
                    ..LayerMix::default()
                },
            );
            rebit_mixer_render(
                &mut mixer,
                &mut a,
                &mut b,
                left.as_mut_ptr(),
                right.as_mut_ptr(),
                8,
            );
            assert_eq!(left, [-0.5; 8]);
            assert_eq!(right, [0.; 8]);
            rebit_mixer_render(
                &mut mixer,
                &mut a,
                &mut b,
                left.as_mut_ptr(),
                ptr::null_mut(),
                8,
            );
            assert_eq!(left, [-0.25; 8]);
            rebit_mixer_free(ptr::null_mut());
            rebit_engine_tuning(ptr::null_mut(), 0.);
        }
    }
    #[test]
    fn playback_snapshot_layout_and_null_boundary() {
        assert_eq!(std::mem::size_of::<crate::PlaybackInfo>(), 28);
        assert_eq!(std::mem::align_of::<crate::PlaybackInfo>(), 4);
        let mut engine = Engine::default();
        engine.midi(0x90, 60, 127);
        engine.render(&mut [0.; 16]);
        let mut snapshot = crate::PlaybackInfo::default();
        unsafe {
            rebit_engine_playback(&engine, &mut snapshot);
            assert_eq!(snapshot, engine.playback_info());
            rebit_engine_playback(&engine, ptr::null_mut());
            rebit_engine_playback(ptr::null(), &mut snapshot);
            assert_eq!(snapshot, crate::PlaybackInfo::default());
        }
    }
    #[test]
    fn swapping_a_live_bank_preserves_cursor_and_returns_retired_ownership() {
        let mut engine = Engine::new(48000., 1);
        engine.set_parameters(Parameters {
            volume_enabled: 1,
            pitch_enabled: 1,
            duty_enabled: 1,
            release: 1.,
            ..Parameters::default()
        });
        let mut bank = SequenceBank::default();
        bank.0[0] = Sequence::parse("15,14,13,12,11,10,9,8|4,3,2,1", 0, 15).unwrap();
        bank.0[1] = Sequence::parse("0,1,2,3,4,5,6,7|8,9,10,11", -64, 63).unwrap();
        bank.0[2] = Sequence::parse("0,1,2,0,1,2,0,1|2,1,0,1", 0, 2).unwrap();
        engine.set_sequences(bank.clone());
        engine.midi(0x90, 60, 127);
        engine.render(&mut [0.; 1601]);
        assert_eq!(engine.sequence_positions(), [2, 2, 2]);
        bank.0[0].values[2] = 7;
        unsafe {
            let retired =
                rebit_engine_swap_bank(&mut engine, Box::into_raw(Box::new(bank.clone())));
            assert_eq!(engine.sequence_positions(), [2, 2, 2]);
            rebit_bank_free(retired);
            engine.midi(0x80, 60, 0);
            engine.render(&mut [0.; 801]);
            let before = engine.sequence_positions();
            assert_eq!(before, [9, 9, 9]);
            bank.0[1] = Sequence::parse("0,1,2,3|8,9,10,11", -64, 63).unwrap();
            let retired = rebit_engine_swap_bank(&mut engine, Box::into_raw(Box::new(bank)));
            assert_eq!(engine.sequence_positions(), [9, 5, 9]);
            rebit_bank_free(retired);
        }
    }
    #[test]
    fn bank_snapshot_bounds_and_invalid_lane() {
        let mut bank = SequenceBank::default();
        bank.0[1] = Sequence::parse("0[4,7]|12", -64, 63).unwrap();
        let mut info = SequenceInfo {
            count: 0,
            loop_start: -1,
            release_start: -1,
        };
        let mut out = [99i16; 2];
        unsafe {
            assert!(rebit_bank_read(&bank, 1, out.as_mut_ptr(), 2, &mut info));
            assert_eq!(out, [0, 4]);
            assert_eq!((info.count, info.loop_start, info.release_start), (4, 1, 3));
            assert!(!rebit_bank_read(&bank, 3, out.as_mut_ptr(), 2, &mut info));
        }
    }
}
