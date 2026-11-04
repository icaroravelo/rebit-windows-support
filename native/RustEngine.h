#pragma once
#include <cstddef>
#include <cstdint>
#include <type_traits>
struct RustEngine;
struct RustSequenceBank;
struct RustParameters {
    int32_t osc, duty, noise_algorithm, max_poly, behavior, interval_type, pitch_mode;
    float gain, attack, decay, sustain, release, portamento, bend_range, vibrato_period,
          vibrato_depth, vibrato_delay, sweep_initial, sweep_time, interval;
    int32_t ignores_wheel, volume_enabled, pitch_enabled, duty_enabled;
};
static_assert(sizeof(RustParameters) == 96 && std::is_standard_layout_v<RustParameters>);
struct RustArpParameters {
    int32_t enabled, pattern, rate, octaves, latch, retrigger;
    float seconds, gate, swing;
};
static_assert(sizeof(RustArpParameters)==36 && std::is_standard_layout_v<RustArpParameters>);
struct RustClockParameters { int32_t host_sync, sequence_rate; };
struct RustTransportInfo {
    double ppq,bpm,loop_start,loop_end;
    int32_t valid,playing,looping,reserved;
};
static_assert(sizeof(RustClockParameters)==8 && sizeof(RustTransportInfo)==48);
struct RustSequenceInfo { size_t count; int32_t loop_start, release_start; };
extern "C" {
RustEngine* rebit_engine_new();
void rebit_engine_free(RustEngine*);
void rebit_engine_prepare(RustEngine*, double);
void rebit_engine_reset(RustEngine*);
void rebit_engine_arp_parameters(RustEngine*, const RustArpParameters*);
void rebit_engine_clock_parameters(RustEngine*,const RustClockParameters*);
void rebit_engine_transport(RustEngine*,const RustTransportInfo*);
int32_t rebit_engine_clock_status(const RustEngine*);
void rebit_engine_parameters(RustEngine*, const RustParameters*, double);
void rebit_engine_midi(RustEngine*, uint8_t, uint8_t, uint8_t);
void rebit_engine_render(RustEngine*, float*, size_t);
RustSequenceBank* rebit_bank_new();
RustSequenceBank* rebit_bank_clone(const RustSequenceBank*);
void rebit_bank_free(RustSequenceBank*);
bool rebit_bank_set(RustSequenceBank*, size_t, const uint8_t*, size_t, uint8_t*, size_t);
bool rebit_bank_read(const RustSequenceBank*, size_t, int16_t*, size_t, RustSequenceInfo*);
void rebit_engine_positions(const RustEngine*, int32_t*);
RustSequenceBank* rebit_engine_swap_bank(RustEngine*, RustSequenceBank*);
}
// All pointers crossing this boundary come from Rust allocation functions.
// Engine is audio-thread-only after prepare; canonical is textMutex-only.
// Pending banks transfer between threads under sequenceLock, and are freed
// only by the control thread or during processor teardown.
