use rebit_rust::layers::{LayerMix, LayerMixer};
use rebit_rust::sequence::{Cursor, MAX_FRAMES};
use rebit_rust::{
    Engine, LfoParameters, ModRoute, ModulationParameters, Parameters, Sequence, SequenceBank,
};
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

#[test]
fn rapid_live_changes_are_chunk_invariant_and_allocation_free() {
    fn edit(e: &mut Engine, event: usize) {
        e.set_parameters(Parameters {
            max_poly: [1, 8, 2, 64][event % 4],
            behavior: (event % 4) as i32,
            osc: (event % 3) as i32,
            duty: ((event / 3) % 3) as i32,
            noise_algorithm: ((event / 5) % 3) as i32,
            gain: (event % 11) as f32 / 10.,
            sustain: (event % 7) as f32 / 6.,
            attack: if event.is_multiple_of(2) { 0. } else { 0.002 },
            bend_range: (event % 25) as f32,
            sweep_initial: (event % 49) as f32 - 24.,
            vibrato_depth: 1.,
            vibrato_delay: 0.,
            ..Default::default()
        });
        let note = (event % 128) as u8;
        e.midi(0x90, note, 100);
        e.midi(0xe0, (event % 128) as u8, (event % 128) as u8);
        if event > 10 {
            e.midi(0x80, ((event - 10) % 128) as u8, 0);
        }
    }
    let mut a = Engine::new(48000., 123);
    let mut b = Engine::new(48000., 123);
    let mut expected = vec![0.; 128 * 73];
    let mut actual = vec![0.; expected.len()];
    for (event, chunk) in expected.chunks_mut(73).enumerate() {
        edit(&mut a, event);
        a.render(chunk);
    }
    ALLOCS.with(|c| c.set(0));
    TRACK.with(|c| c.set(true));
    for (event, chunk) in actual.chunks_mut(73).enumerate() {
        edit(&mut b, event);
        for part in chunk.chunks_mut(7) {
            b.render(part);
        }
    }
    b.midi(0xb0, 120, 0);
    let mut silence = [1.; 128];
    b.render(&mut silence);
    TRACK.with(|c| c.set(false));
    assert_eq!(ALLOCS.with(Cell::get), 0);
    assert_eq!(actual, expected);
    assert!(actual.iter().all(|x| x.is_finite()));
    assert_eq!(silence, [0.; 128]);
}

#[test]
fn playback_feedback_measures_post_gain_output_and_silent_voices() {
    let mut e = Engine::new(8000., 1);
    assert_eq!(e.playback_info(), rebit_rust::PlaybackInfo::default());
    e.midi(0x90, 0, 127);
    e.midi(0x90, 1, 127);
    let output = audio(&mut e, 32);
    let info = e.playback_info();
    assert_eq!(info.peak, peak(&output));
    assert_eq!(info.held_peak, 1.);
    assert_eq!(info.clips, 32);
    assert_eq!(info.active, 2);
    assert_eq!(info.held, 2);
    assert_eq!(info.released, 0);
    e.reset();
    e.set_parameters(Parameters {
        gain: 0.25,
        ..e.parameters()
    });
    e.midi(0x90, 0, 127);
    e.midi(0x90, 1, 127);
    audio(&mut e, 32);
    assert_eq!(e.playback_info().held_peak, 0.5);
    let clips = e.playback_info().clips;
    e.set_parameters(Parameters {
        volume_enabled: 1,
        ..e.parameters()
    });
    e.reset();
    e.midi(0x90, 0, 127);
    assert_eq!(audio(&mut e, 64), vec![0.; 64]);
    let info = e.playback_info();
    assert_eq!(info.active, 1);
    assert_eq!(info.held, 1);
    assert_eq!(info.peak, 0.);
    assert_eq!(info.held_peak, 0.);
    assert_eq!(info.clips, clips);
}

#[test]
fn playback_activity_distinguishes_releases_pedals_and_retirement() {
    let mut e = Engine::new(8000., 1);
    e.set_parameters(Parameters {
        release: 0.1,
        ..e.parameters()
    });
    for note in [0, 1, 2] {
        e.midi(0x90, note, 127);
    }
    audio(&mut e, 32);
    e.midi(0x80, 0, 0);
    audio(&mut e, 16);
    let info = e.playback_info();
    assert_eq!(
        (info.active, info.held, info.released, info.fading),
        (3, 2, 1, 0)
    );
    e.midi(0xb0, 64, 127);
    e.midi(0x80, 1, 0);
    audio(&mut e, 16);
    assert_eq!(e.playback_info().held, 2);
    e.set_parameters(Parameters {
        max_poly: 1,
        ..e.parameters()
    });
    let info = e.playback_info();
    assert_eq!(
        (info.active, info.held, info.released, info.fading),
        (1, 1, 0, 2)
    );
    audio(&mut e, 24);
    assert_eq!(e.playback_info().fading, 0);
    e.midi(0xb0, 120, 0);
    audio(&mut e, 16);
    assert_eq!(e.playback_info().active, 0);
    assert_eq!(e.playback_info().fading, 0);
    assert!(e.playback_info().held_peak > 0.); // Meter history is independent of current voice count.
}

#[test]
fn playback_feedback_is_chunk_invariant_and_allocation_free() {
    let make = || {
        let mut e = Engine::new(48000., 123);
        e.set_parameters(Parameters {
            osc: 2,
            release: 0.01,
            ..e.parameters()
        });
        for note in [48, 60, 67] {
            e.midi(0x90, note, 127);
        }
        e
    };
    let mut a = make();
    let mut b = make();
    let expected = audio(&mut a, 4096);
    a.midi(0xb0, 123, 0);
    let expected_tail = audio(&mut a, 48000);
    let mut actual = [0.; 4096];
    let mut tail = [0.; 48000];
    ALLOCS.with(|c| c.set(0));
    TRACK.with(|c| c.set(true));
    for part in actual.chunks_mut(37) {
        b.render(part);
        std::hint::black_box(b.playback_info());
    }
    b.midi(0xb0, 123, 0);
    for part in tail.chunks_mut(137) {
        b.render(part);
        std::hint::black_box(b.playback_info());
    }
    TRACK.with(|c| c.set(false));
    assert_eq!(ALLOCS.with(Cell::get), 0);
    assert_eq!(actual.as_slice(), expected);
    assert_eq!(tail.as_slice(), expected_tail);
    assert_eq!(a.playback_info(), b.playback_info());
    assert!(b.playback_info().clips > 0);
    assert_eq!(b.playback_info().active, 0);
}

#[test]
fn two_layer_automation_and_rendering_are_allocation_free_and_chunk_invariant() {
    use rebit_rust::layers::{LayerMix, LayerMixer};
    let make = || {
        let mut a = Box::new(Engine::new(48000., 123));
        let mut b = Box::new(Engine::new(48000., 456));
        for e in [&mut a, &mut b] {
            e.set_parameters(Parameters {
                osc: 2,
                max_poly: 16,
                vibrato_depth: 0.2,
                vibrato_delay: 0.,
                release: 0.01,
                ..Default::default()
            });
        }
        let mut mixer = LayerMixer::default();
        mixer.prepare(48000.);
        (a, b, mixer)
    };
    let edit = |a: &mut Engine, b: &mut Engine, m: &mut LayerMixer, i: usize| {
        a.set_tuning((i % 49) as f64 - 24.);
        b.set_tuning((i % 101) as f64 * 0.01 - 12.);
        m.set_mix(
            0,
            LayerMix {
                enabled: 1,
                gain: 0.7,
                pan: (i % 21) as f32 * 0.1 - 1.,
            },
        );
        m.set_mix(
            1,
            LayerMix {
                enabled: i32::from(!i.is_multiple_of(4)),
                gain: 0.3,
                pan: 1. - (i % 21) as f32 * 0.1,
            },
        );
        for e in [a, b] {
            e.midi(0x90, (48 + i % 24) as u8, 100);
            if i > 8 {
                e.midi(0x80, (48 + (i - 8) % 24) as u8, 0);
            }
        }
    };
    let (mut a, mut b, mut m) = make();
    let (mut c, mut d, mut n) = make();
    let mut l = vec![0.; 64 * 137];
    let mut r = vec![0.; l.len()];
    let mut x = vec![0.; l.len()];
    let mut y = vec![0.; l.len()];
    for (i, (left, right)) in l.chunks_mut(137).zip(r.chunks_mut(137)).enumerate() {
        edit(&mut a, &mut b, &mut m, i);
        m.render(&mut a, &mut b, left, right);
    }
    ALLOCS.with(|c| c.set(0));
    TRACK.with(|c| c.set(true));
    for (i, (left, right)) in x.chunks_mut(137).zip(y.chunks_mut(137)).enumerate() {
        edit(&mut c, &mut d, &mut n, i);
        for (left, right) in left.chunks_mut(7).zip(right.chunks_mut(7)) {
            n.render(&mut c, &mut d, left, right);
            std::hint::black_box(n.playback(&c, &d));
        }
    }
    TRACK.with(|c| c.set(false));
    assert_eq!(ALLOCS.with(Cell::get), 0);
    assert_eq!(l, x);
    assert_eq!(r, y);
    assert_eq!(m.playback(&a, &b), n.playback(&c, &d));
}

#[test]
fn clean_and_effect_automation_are_chunk_invariant_and_allocation_free() {
    use rebit_rust::{
        effects::EffectsParameters,
        layers::{LayerMix, LayerMixer},
    };
    let make = || {
        let mut a = Box::new(Engine::new(48000., 123));
        let mut b = Box::new(Engine::new(48000., 456));
        for e in [&mut a, &mut b] {
            e.set_parameters(Parameters {
                gain: 0.2,
                max_poly: 8,
                release: 0.01,
                ..Default::default()
            });
        }
        let mut m = LayerMixer::default();
        m.prepare(48000.);
        m.set_mix(1, LayerMix::default());
        (a, b, m)
    };
    let edit = |a: &mut Engine, b: &mut Engine, m: &mut LayerMixer, i: usize| {
        a.set_sound_mode((i % 2) as i32);
        b.set_sound_mode(((i / 3) % 2) as i32);
        m.set_effects(EffectsParameters {
            enabled: i32::from(!i.is_multiple_of(4)),
            drive_db: (i % 25) as f32,
            bits: 4. + (i % 21) as f32,
            rate_divider: 1. + (i % 32) as f32,
            mix: (i % 11) as f32 / 10.,
        });
        for e in [a, b] {
            e.midi(0x90, (48 + i % 24) as u8, 100);
            if i > 6 {
                e.midi(0x80, (48 + (i - 6) % 24) as u8, 0);
            }
        }
    };
    let (mut a, mut b, mut m) = make();
    let (mut c, mut d, mut n) = make();
    let mut l = vec![0.; 96 * 137];
    let mut r = vec![0.; l.len()];
    let mut x = vec![0.; l.len()];
    let mut y = vec![0.; l.len()];
    for (i, (left, right)) in l.chunks_mut(137).zip(r.chunks_mut(137)).enumerate() {
        edit(&mut a, &mut b, &mut m, i);
        m.render(&mut a, &mut b, left, right);
    }
    ALLOCS.with(|c| c.set(0));
    TRACK.with(|c| c.set(true));
    for (i, (left, right)) in x.chunks_mut(137).zip(y.chunks_mut(137)).enumerate() {
        edit(&mut c, &mut d, &mut n, i);
        for (left, right) in left.chunks_mut(7).zip(right.chunks_mut(7)) {
            n.render(&mut c, &mut d, left, right);
        }
    }
    n.clear_effects_history();
    c.midi(0xb0, 120, 0);
    d.midi(0xb0, 120, 0);
    let mut silence = [1.; 32];
    let mut other = [1.; 32];
    n.render(&mut c, &mut d, &mut silence, &mut other);
    TRACK.with(|c| c.set(false));
    assert_eq!(ALLOCS.with(Cell::get), 0);
    assert_eq!(l, x);
    assert_eq!(r, y);
    assert!(l.iter().chain(&r).all(|x| x.is_finite()));
    assert_eq!(silence, [0.; 32]);
    assert_eq!(other, [0.; 32]);
}

fn modulation(
    routes: [ModRoute; 4],
    lfo: [LfoParameters; 2],
    macros: [f32; 4],
) -> ModulationParameters {
    ModulationParameters {
        routes,
        lfo,
        macros,
    }
}
fn route(source: i32, destination: i32, amount: f32) -> ModRoute {
    ModRoute {
        source,
        destination,
        amount,
    }
}
#[test]
fn velocity_aftertouch_and_wheel_reach_their_destinations() {
    let routes = [
        route(1, 3, 1.),
        route(2, 2, 1.),
        route(3, 3, 1.),
        ModRoute::default(),
    ];
    let mut quiet = Engine::new(8000., 1);
    quiet.set_modulation(modulation(routes, [LfoParameters::default(); 2], [0.; 4]));
    quiet.midi(0x90, 60, 32);
    let quiet_peak = peak(&audio(&mut quiet, 64));
    let mut loud = Engine::new(8000., 1);
    loud.set_modulation(modulation(routes, [LfoParameters::default(); 2], [0.; 4]));
    loud.midi(0x90, 60, 32);
    loud.midi(0xd0, 127, 0);
    let loud_peak = peak(&audio(&mut loud, 64));
    assert!(
        loud_peak > quiet_peak * 1.4,
        "aftertouch did not lift level"
    );

    let mut plain = Engine::new(8000., 1);
    plain.midi(0x90, 60, 96);
    let plain_out = audio(&mut plain, 64);
    let mut wheel = Engine::new(8000., 1);
    wheel.set_modulation(modulation(routes, [LfoParameters::default(); 2], [0.; 4]));
    wheel.midi(0xb0, 1, 127);
    wheel.midi(0x90, 60, 96);
    let wheel_out = audio(&mut wheel, 64);
    assert_ne!(plain_out, wheel_out, "mod wheel did not reach pulse width");
}
#[test]
fn lfo_routes_are_deterministic_and_bounded() {
    let routes = [
        route(5, 1, 0.5),
        route(6, 3, 0.25),
        ModRoute::default(),
        ModRoute::default(),
    ];
    let lfo = [
        LfoParameters {
            shape: 5,
            rate_hz: 7.,
            retrigger: 1,
        },
        LfoParameters {
            shape: 1,
            rate_hz: 3.,
            retrigger: 0,
        },
    ];
    let mut first = Engine::new(48000., 9);
    first.set_modulation(modulation(routes, lfo, [0.; 4]));
    first.midi(0x90, 60, 100);
    let expected = audio(&mut first, 4096);
    let mut second = Engine::new(48000., 9);
    second.set_modulation(modulation(routes, lfo, [0.; 4]));
    second.midi(0x90, 60, 100);
    let actual = audio(&mut second, 4096);
    assert_eq!(expected, actual, "LFO routing is not deterministic");
    assert!(expected.iter().all(|x| x.is_finite()) && peak(&expected) > 0.);
    let mut neutral = Engine::new(48000., 9);
    neutral.midi(0x90, 60, 100);
    assert_ne!(
        expected,
        audio(&mut neutral, 4096),
        "LFO routes had no effect"
    );
}
#[test]
fn layer_balance_and_fx_destinations_are_shared_and_sample_exact() {
    let mut a = Engine::new(8000., 1);
    let mut b = Engine::new(8000., 2);
    let mut mixer = LayerMixer::default();
    mixer.prepare(8000.);
    mixer.set_mix(
        1,
        LayerMix {
            enabled: 1,
            gain: 0.,
            pan: 0.,
        },
    );
    a.set_modulation(modulation(
        [
            route(7, 4, 1.),
            ModRoute::default(),
            ModRoute::default(),
            ModRoute::default(),
        ],
        [LfoParameters::default(); 2],
        [1., 0., 0., 0.],
    ));
    a.midi(0x90, 0, 127);
    let mut left = [0.; 64];
    let mut right = [0.; 64];
    mixer.render(&mut a, &mut b, &mut left, &mut right);
    assert_eq!(peak(&left), 0., "layer balance did not mute A");
    assert_eq!(left, right);

    let mut a = Engine::new(8000., 1);
    let mut b = Engine::new(8000., 2);
    let mut mixer = LayerMixer::default();
    mixer.prepare(8000.);
    mixer.set_effects(rebit_rust::effects::EffectsParameters {
        enabled: 1,
        drive_db: 0.,
        bits: 24.,
        rate_divider: 1.,
        mix: 1.,
    });
    a.set_modulation(modulation(
        [
            route(7, 5, 1.),
            ModRoute::default(),
            ModRoute::default(),
            ModRoute::default(),
        ],
        [LfoParameters::default(); 2],
        [1., 0., 0., 0.],
    ));
    a.set_parameters(Parameters {
        gain: 0.1,
        ..Parameters::default()
    });
    a.midi(0x90, 60, 127);
    mixer.render(&mut a, &mut b, &mut left, &mut right);
    assert!(left.iter().all(|x| x.is_finite()) && peak(&left) > 0.);
    let mut plain = Engine::new(8000., 1);
    let mut plain_b = Engine::new(8000., 2);
    let mut plain_mixer = LayerMixer::default();
    plain_mixer.prepare(8000.);
    plain_mixer.set_effects(rebit_rust::effects::EffectsParameters {
        enabled: 1,
        drive_db: 0.,
        bits: 24.,
        rate_divider: 1.,
        mix: 1.,
    });
    plain.set_parameters(Parameters {
        gain: 0.1,
        ..Parameters::default()
    });
    plain.midi(0x90, 60, 127);
    let mut plain_left = [0.; 64];
    let mut plain_right = [0.; 64];
    plain_mixer.render(&mut plain, &mut plain_b, &mut plain_left, &mut plain_right);
    assert_ne!(left, plain_left, "FX drive destination had no effect");
}
#[test]
fn modulation_rendering_is_chunk_invariant_and_allocation_free() {
    let routes = [
        route(5, 1, 0.4),
        route(6, 3, 0.2),
        ModRoute::default(),
        ModRoute::default(),
    ];
    let lfo = [
        LfoParameters {
            shape: 0,
            rate_hz: 5.,
            retrigger: 0,
        },
        LfoParameters {
            shape: 4,
            rate_hz: 2.,
            retrigger: 0,
        },
    ];
    let mut expected_a = Engine::new(48000., 3);
    let mut expected_b = Engine::new(48000., 4);
    let mut expected_mixer = LayerMixer::default();
    expected_mixer.prepare(48000.);
    expected_a.set_modulation(modulation(routes, lfo, [0.; 4]));
    expected_a.midi(0x90, 60, 100);
    let mut expected_l = vec![0.; 2048];
    let mut expected_r = vec![0.; 2048];
    expected_mixer.render(
        &mut expected_a,
        &mut expected_b,
        &mut expected_l,
        &mut expected_r,
    );

    let mut actual_a = Engine::new(48000., 3);
    let mut actual_b = Engine::new(48000., 4);
    let mut actual_mixer = LayerMixer::default();
    actual_mixer.prepare(48000.);
    actual_a.set_modulation(modulation(routes, lfo, [0.; 4]));
    actual_a.midi(0x90, 60, 100);
    let mut actual_l = vec![0.; 2048];
    let mut actual_r = vec![0.; 2048];
    for (left, right) in actual_l.chunks_mut(137).zip(actual_r.chunks_mut(137)) {
        for (left, right) in left.chunks_mut(7).zip(right.chunks_mut(7)) {
            actual_mixer.render(&mut actual_a, &mut actual_b, left, right);
        }
    }
    assert_eq!(expected_l, actual_l);
    assert_eq!(expected_r, actual_r);
    TRACK.with(|track| track.set(true));
    let before = ALLOCS.with(Cell::get);
    actual_mixer.render(
        &mut actual_a,
        &mut actual_b,
        &mut actual_l[..128],
        &mut actual_r[..128],
    );
    let after = ALLOCS.with(Cell::get);
    TRACK.with(|track| track.set(false));
    assert_eq!(before, after, "modulation rendering allocated");
}
#[test]
fn per_channel_bend_and_pressure_support_mpe_note_expression() {
    let mut other = Engine::new(8000., 1);
    other.midi(0x90, 60, 127);
    let base = audio(&mut other, 128);
    let mut bent_elsewhere = Engine::new(8000., 1);
    bent_elsewhere.midi(0x90, 60, 127);
    bent_elsewhere.midi(0xe1, 0, 127);
    assert_eq!(
        base,
        audio(&mut bent_elsewhere, 128),
        "a member channel bend leaked into another note"
    );
    let mut bent = Engine::new(8000., 1);
    bent.midi(0x90, 60, 127);
    bent.midi(0xe0, 0, 127);
    assert_ne!(
        base,
        audio(&mut bent, 128),
        "per-note bend did not change pitch"
    );

    let routes = [
        route(3, 3, 1.),
        ModRoute::default(),
        ModRoute::default(),
        ModRoute::default(),
    ];
    let mut pressure = Engine::new(8000., 1);
    pressure.set_modulation(modulation(routes, [LfoParameters::default(); 2], [0.; 4]));
    pressure.midi(0x90, 60, 64);
    pressure.midi(0xd0, 127, 0);
    let pressed = peak(&audio(&mut pressure, 64));
    let mut plain = Engine::new(8000., 1);
    plain.set_modulation(modulation(routes, [LfoParameters::default(); 2], [0.; 4]));
    plain.midi(0x90, 60, 64);
    let released = peak(&audio(&mut plain, 64));
    assert!(
        pressed > released * 1.4,
        "channel pressure did not reach its route"
    );

    let mut poly = Engine::new(8000., 1);
    poly.set_modulation(modulation(routes, [LfoParameters::default(); 2], [0.; 4]));
    poly.midi(0x90, 60, 64);
    poly.midi(0xa0, 60, 127);
    let poly_pressed = peak(&audio(&mut poly, 64));
    assert!(
        poly_pressed > released * 1.4,
        "poly aftertouch did not reach its route"
    );
}
