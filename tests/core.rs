use rebit_rust::sequence::{Cursor, MAX_FRAMES};
use rebit_rust::{Engine, Parameters, Sequence, SequenceBank};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct TrackedAllocator;
thread_local! { static TRACK: Cell<bool> = const { Cell::new(false) }; static ALLOCS: Cell<usize> = const { Cell::new(0) }; }
unsafe impl GlobalAlloc for TrackedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        }
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        }
        System.realloc(ptr, layout, size)
    }
}
#[global_allocator]
static ALLOCATOR: TrackedAllocator = TrackedAllocator;
fn audio(engine: &mut Engine, n: usize) -> Vec<f32> {
    let mut out = vec![0.; n];
    engine.render(&mut out);
    out
}
fn peak(out: &[f32]) -> f32 {
    out.iter().fold(0f32, |a, x| a.max(x.abs()))
}

#[test]
fn strict_sequence_grammar_and_limits() {
    assert_eq!(
        &*Sequence::parse("15 to 0 in 5f", 0, 15).unwrap().values,
        &[15, 11, 8, 4, 0]
    );
    assert_eq!(
        &*Sequence::parse("-5 to 0 in 3", -64, 63).unwrap().values,
        &[-5, -3, 0]
    );
    assert_eq!(
        &*Sequence::parse("2x3,0", 0, 15).unwrap().values,
        &[2, 2, 2, 0]
    );
    assert_eq!(
        Sequence::parse("1x65534", 0, 15).unwrap().values.len(),
        MAX_FRAMES
    );
    assert!(Sequence::parse(" \n ", 0, 15).unwrap().values.is_empty());
    for bad in [
        "15,,10",
        "15,",
        "15to0in1",
        "[15",
        "15|[0]",
        "15x99999999999",
        "16",
        "1.5",
        "+3",
        "5-",
        "15|",
        "15[10]junk",
        "3x0",
        "3x-1",
        "1x65535",
        "1x65534,1",
        "[1][2]",
        "1|2|3",
        "1[]",
        "é",
    ] {
        assert!(Sequence::parse(bad, 0, 15).is_err(), "Accepted {bad}");
    }
}
#[test]
fn sequence_loops_and_release() {
    let seq = Sequence::parse("15[10,5]|3,0", 0, 15).unwrap();
    let mut c = Cursor::default();
    for expected in [15, 10, 5, 10, 5] {
        assert_eq!(c.value(&seq), Some(expected));
        c.advance(&seq);
    }
    c.release(&seq);
    assert_eq!(c.value(&seq), Some(3));
    c.release(&seq);
    c.advance(&seq);
    assert_eq!(c.value(&seq), Some(0));
    c.advance(&seq);
    assert_eq!(c.value(&seq), None);
}
#[test]
fn note_lifecycle_and_velocity_zero() {
    let mut e = Engine::default();
    assert_eq!(peak(&audio(&mut e, 100)), 0.);
    e.midi(0x90, 69, 127);
    assert_eq!(peak(&audio(&mut e, 100)), 0.5);
    e.midi(0x90, 69, 0);
    assert_eq!(peak(&audio(&mut e, 100)), 0.);
    assert_eq!(e.active_voices(), 0);
}
#[test]
fn release_duration_follows_current_level() {
    let mut e = Engine::new(48_000., 1);
    e.set_parameters(Parameters {
        sustain: 0.25,
        release: 0.1,
        ..Parameters::default()
    });
    e.midi(0x90, 60, 127);
    audio(&mut e, 100);
    e.midi(0x80, 60, 0);
    assert!(peak(&audio(&mut e, 4790)) > 0.);
    assert_eq!(e.active_voices(), 1);
    audio(&mut e, 20);
    assert_eq!(e.active_voices(), 0);
}
#[test]
fn sustain_sostenuto_and_reset_controllers() {
    let mut e = Engine::default();
    e.midi(0x90, 60, 127);
    audio(&mut e, 10);
    e.midi(0xb0, 64, 127);
    e.midi(0x80, 60, 0);
    assert!(peak(&audio(&mut e, 100)) > 0.);
    e.midi(0xb0, 64, 0);
    audio(&mut e, 10);
    assert_eq!(e.active_voices(), 0);
    e.midi(0x90, 60, 127);
    audio(&mut e, 10);
    e.midi(0xb0, 66, 127);
    e.midi(0x80, 60, 0);
    assert!(peak(&audio(&mut e, 100)) > 0.);
    e.midi(0xb0, 121, 0);
    audio(&mut e, 10);
    assert_eq!(e.active_voices(), 0);
}
#[test]
fn all_notes_off_respects_pedals_and_all_sound_off_is_channel_local() {
    let mut e = Engine::default();
    e.midi(0x90, 60, 127);
    e.midi(0x91, 64, 127);
    audio(&mut e, 10);
    e.midi(0xb0, 64, 127);
    e.midi(0xb0, 123, 0);
    audio(&mut e, 10);
    assert_eq!(e.active_voices(), 2);
    e.midi(0xb0, 120, 0);
    audio(&mut e, 10);
    assert_eq!(e.active_voices(), 1);
    e.midi(0xb1, 120, 0);
    audio(&mut e, 10);
    assert_eq!(e.active_voices(), 0);
}
#[test]
fn bounded_voice_stealing() {
    let mut e = Engine::default();
    e.set_parameters(Parameters {
        max_poly: 2,
        ..Parameters::default()
    });
    for note in 0..128 {
        e.midi(0x90, note, 127);
    }
    assert_eq!(e.active_voices(), 2);
    assert!(audio(&mut e, 128).iter().all(|v| v.is_finite()));
}
#[test]
fn noise_is_instance_local_and_chunk_invariant() {
    let make = || {
        let mut e = Engine::new(48_000., 123);
        e.set_parameters(Parameters {
            osc: 2,
            ..Parameters::default()
        });
        e.midi(0x90, 60, 127);
        e.midi(0x90, 67, 127);
        e
    };
    let mut a = make();
    let mut b = make();
    let mut other = make();
    let expected = audio(&mut a, 8192);
    audio(&mut other, 5000);
    let mut actual = Vec::new();
    for n in [1, 15, 64, 256, 7856] {
        actual.extend(audio(&mut b, n));
    }
    assert_eq!(expected, actual);
}
#[test]
fn rendering_and_midi_do_not_allocate_or_free() {
    let mut e = Engine::default();
    let mut out = [0.; 128];
    ALLOCS.with(|c| c.set(0));
    TRACK.with(|c| c.set(true));
    for i in 0..512 {
        e.midi(0x90, (i % 128) as u8, 127);
        e.render(&mut out);
        std::hint::black_box(e.sequence_positions());
        e.midi(0xb0, 64, (i % 128) as u8);
        e.midi(0x80, (i % 128) as u8, 0);
    }
    TRACK.with(|c| c.set(false));
    assert_eq!(ALLOCS.with(Cell::get), 0);
}
#[test]
fn extreme_notes_rates_and_nonfinite_parameters_remain_finite() {
    for rate in [
        f64::NAN,
        8000.,
        11025.,
        44100.,
        48000.,
        96000.,
        192000.,
        384000.,
    ] {
        let mut e = Engine::new(rate, 1);
        e.set_parameters(Parameters {
            gain: f32::NAN,
            attack: f32::INFINITY,
            vibrato_depth: 2.,
            vibrato_period: 0.,
            sweep_initial: 24.,
            max_poly: 64,
            ..Parameters::default()
        });
        for note in 0..128 {
            e.midi(0x90, note, 127);
            assert!(audio(&mut e, 64).iter().all(|v| v.is_finite()));
        }
    }
}
#[test]
fn volume_sequence_release_terminates_voice() {
    let mut e = Engine::new(48_000., 1);
    e.set_parameters(Parameters {
        volume_enabled: 1,
        ..Parameters::default()
    });
    let mut bank = SequenceBank::default();
    bank.0[0] = Sequence::parse("15[10,5]|3,0", 0, 15).unwrap();
    e.set_sequences(bank);
    e.midi(0x90, 60, 127);
    audio(&mut e, 1000);
    e.midi(0x80, 60, 0);
    audio(&mut e, 2500);
    assert_eq!(e.active_voices(), 0);
}
#[test]
fn ffi_parameter_layout_is_stable() {
    assert_eq!(std::mem::size_of::<Parameters>(), 96);
    assert_eq!(std::mem::align_of::<Parameters>(), 4);
}

#[test]
fn sequence_cursor_rebind_clamps_main_and_preserves_release_offset() {
    let old = Sequence::parse("15,14,13,12|3,2,1", 0, 15).unwrap();
    let new = Sequence::parse("15,14|3,2,1", 0, 15).unwrap();
    let mut cursor = Cursor::default();
    cursor.advance(&old);
    cursor.advance(&old);
    cursor.advance(&old);
    cursor.rebind(&old, &new);
    assert_eq!(cursor.position(&new), Some(1));
    cursor.release(&new);
    cursor.advance(&new);
    assert_eq!(cursor.position(&new), Some(3));
    cursor.rebind(&new, &old);
    assert_eq!(cursor.position(&old), Some(5));
    let empty = Sequence::default();
    cursor.rebind(&old, &empty);
    assert_eq!(cursor.position(&empty), None);
}
#[test]
fn telemetry_tracks_latest_active_voice_and_disabled_lanes() {
    let mut e = Engine::new(48000., 1);
    let mut bank = SequenceBank::default();
    bank.0[1] = Sequence::parse("[0,4,7,12]", -64, 63).unwrap();
    e.set_sequences(bank);
    e.set_parameters(Parameters {
        pitch_enabled: 1,
        ..Parameters::default()
    });
    assert_eq!(e.sequence_positions(), [-1; 3]);
    e.midi(0x90, 60, 127);
    audio(&mut e, 1601);
    assert_eq!(e.sequence_positions(), [-1, 2, -1]);
    e.midi(0x90, 67, 127);
    audio(&mut e, 1);
    assert_eq!(e.sequence_positions(), [-1, 0, -1]);
    e.midi(0x80, 67, 0);
    audio(&mut e, 1);
    assert_eq!(e.sequence_positions(), [-1, 2, -1]);
}

#[test]
fn independent_arp_render_is_chunk_invariant_and_allocation_free() {
    let make = || {
        let mut e = Engine::new(48000., 123);
        e.set_arp_parameters(rebit_rust::ArpParameters {
            enabled: 1,
            pattern: 3,
            octaves: 3,
            swing: 0.23,
            ..Default::default()
        });
        for n in [48, 60, 67] {
            e.midi(0x90, n, 100);
        }
        e
    };
    let mut a = make();
    let mut b = make();
    let expected = audio(&mut a, 48000);
    let mut actual = vec![0.; 48000];
    ALLOCS.with(|c| c.set(0));
    TRACK.with(|c| c.set(true));
    for chunk in actual.chunks_mut(137) {
        b.render(chunk);
        std::hint::black_box(b.sequence_positions());
    }
    TRACK.with(|c| c.set(false));
    assert_eq!(ALLOCS.with(Cell::get), 0);
    assert_eq!(actual, expected);
    assert_eq!(std::mem::size_of::<rebit_rust::ArpParameters>(), 36);
}

#[test]
fn host_transport_render_is_chunk_invariant_and_allocation_free() {
    use rebit_rust::{ArpParameters, ClockParameters, TransportInfo};
    let make = || {
        let mut e = Engine::new(48000., 123);
        e.set_arp_parameters(ArpParameters {
            enabled: 1,
            pattern: 2,
            swing: 0.23,
            ..Default::default()
        });
        e.set_parameters(Parameters {
            pitch_enabled: 1,
            ..Default::default()
        });
        let mut bank = SequenceBank::default();
        bank.0[1] = Sequence::parse("[0,4,7,12]", -64, 63).unwrap();
        e.set_sequences(bank);
        e.set_clock_parameters(ClockParameters {
            host_sync: 1,
            sequence_rate: 3,
        });
        e.set_transport(TransportInfo {
            valid: 1,
            playing: 1,
            looping: 1,
            loop_end: 2.,
            ..Default::default()
        });
        e.midi(0x90, 60, 100);
        e.midi(0x90, 67, 100);
        e
    };
    let mut a = make();
    let mut b = make();
    let expected = audio(&mut a, 96000);
    let mut actual = vec![0.; 96000];
    ALLOCS.with(|c| c.set(0));
    TRACK.with(|c| c.set(true));
    let mut start = 0;
    for chunk in actual.chunks_mut(137) {
        b.set_transport(TransportInfo {
            ppq: (start as f64 / 24000.).rem_euclid(2.),
            valid: 1,
            playing: 1,
            looping: 1,
            loop_end: 2.,
            ..Default::default()
        });
        b.render(chunk);
        start += chunk.len();
        std::hint::black_box(b.clock_status());
    }
    TRACK.with(|c| c.set(false));
    assert_eq!(ALLOCS.with(Cell::get), 0);
    assert_eq!(actual, expected);
}
