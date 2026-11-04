//! The only unsafe code in the Rust library. Opaque handles are exclusively owned
//! by the native processor. Its audio thread owns Engine; textMutex owns the
//! canonical bank, and sequenceLock transfers ownership of pending banks.
#![allow(clippy::missing_safety_doc)]
use crate::{Engine, Parameters, Sequence, SequenceBank};
use std::{ptr, slice};

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
