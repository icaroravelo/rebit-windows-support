# Local validation

Checked on 2026-11-04, macOS arm64, Rust 1.97.1, AppleClang 21, JUCE 8.0.12.

- Release builds: AU (`aumu / RbR1 / Rbit`), VST3, standalone app: passed.
- Rust tests: 15 passed. Cover bounded sequence grammar, loops/releases,
  envelopes, mono return, tempo-based arpeggios, pedal handling, channel-local
  MIDI controls, voice limits, instance-independent noise, render chunk sizes,
  extreme notes/rates, nonfinite parameters, and C ABI parameter layout.
- A tracked allocator test confirms Rust rendering and MIDI handling perform
  no heap allocation, reallocation, or destruction during the tested workload.
- `cargo clippy --offline --locked --all-targets -- -D warnings`: passed.
- Native processor test: passed. Checks MIDI on/off at exact sample offsets,
  stereo output, rejected edits preserving previous values, state recall,
  empty sequence clearing, and concurrent control/audio sequence-bank handoff.
- Actual VST3 host: passed discovery, instantiation, MIDI, stereo rendering,
  state save, and restore.
- AU host test: skipped in the restricted session. The library loads and its
  expected factory symbol is present, but AudioComponentFindNext exposes no
  system components, and AudioComponentRegister returns null. `auval -a` also
  exposes no components here. No AU playback or auval conformance claim is made.
  The test is retained in CTest and will run where the system registry is available.
- Offline pulse demo: generated `dist/rebit-rust-demo.wav`, four seconds,
  48 kHz stereo PCM16, non-silent finite samples.
- Actual editor component: rendered offscreen and visually inspected; see
  `preview.png`. No live keyboard/mouse interaction or DAW UI testing was done.

Reproduce:

```
./scripts/build.sh
cargo clippy --offline --locked --all-targets -- -D warnings
./build/rebit_rust_preview /tmp/rebit-rust-preview.png
cargo run --release --offline --locked --bin rebit-render -- demo.wav pulse
```

Remaining work includes AU runtime validation outside this restricted session,
more DAW coverage, Intel/universal builds, notarization, listening tests, and
performance profiling. No measured speedup or sample-identity claim is made.
The original C++ implementation and installed plugins were not modified.

## Installed AU validation after permission update

On 2026-11-04, after unrestricted filesystem/system access was enabled, copied
`Rebit Rust.component` into `~/Library/Audio/Plug-Ins/Components/`, refreshed its
local ad-hoc signature, and verified that signature. Apple's
`auval -v aumu RbR1 Rbit` then completed with **AU VALIDATION SUCCEEDED**,
including render, mono/stereo, parameter scheduling, and MIDI checks. This
supersedes the earlier AU-validation limitation. Interactive testing in Logic
remains for the user. The original Rebit component was not changed.

## Editor redesign, 2026-11-05

Rebuilt AU, VST3, and standalone with the vector pixel/neon editor. The Rust DSP,
C ABI, processor state, and plugin identity are unchanged. All four CTest groups
passed (Rust core, AU host, native processor, VST3 host). After installation,
Apple's `auval -v aumu RbR1 Rbit` again reported AU VALIDATION SUCCEEDED.

Rendered and visually inspected both the default and a populated patch at 1.5x,
including highlighted keys, disabled controls, dense sequence text, and glows.
Also inspected the live standalone via computer-use: the Voices value read back
as 1 and the mono controls enabled; an invalid sequence produced the visible
"Volume: Integer is too large" status; a corrected sequence produced
"All three sequences applied." This supersedes the earlier no-live-UI-check note.
No claim of audible listening or exhaustive mouse/keyboard testing is made.

The installed Rust AU was replaced using a staged, signature-verified bundle;
the previous Rust component was preserved under `dist/au-backups/`. The original
C++ Rebit was not modified. Restart Logic to load the replacement binary.

## Milestone 1: visual sequence editor, 2026-11-05

Added volume/pitch/duty step drawing, strict text round-trips, bounded pattern
length, paging, keyboard fine adjustment, draggable loop/release markers, and
real playheads from the newest active voice. Playheads cross the thread boundary
as lock-free integer atomics; the UI reads a separate control-owned sequence
snapshot, never the audio engine. Audio-side bank exchange preserves valid
main/release cursor progress and continues to return retired banks for
control-thread destruction. No new audio-thread allocation or blocking lock.

- 19 Rust tests passed, including snapshot bounds, bank swap/cursor progress,
  release remapping, representative playheads, and the existing allocation test
  extended to include telemetry reads. Clippy passes with warnings denied.
- All four CTest groups passed: Rust, AU host, native processor, VST3 host.
- Native tests cover graphical mouse gestures for LOOP and REL, a drawn value
  reaching the processor, automatic lane enable, one-unit keyboard changes,
  full-range pitch editing, text/snapshot round-trips, and audio playhead progress
  surviving a live sequence replacement.
- Visually inspected the actual default and populated editor renders.
- Live standalone checks: drawing created a 16-step pattern and enabled Volume;
  the expanded text read back the drawn values. Foreground loop/release drags
  moved the brackets/boundary in that text. Background marker drags were not
  reliable with the local computer-use provider, so they were not counted as
  successful checks. No new audible-listening claim is made.

The popup/parameter IDs and version-1 state schema are retained. New graphical
patches serialize through the existing Rust parser. Milestones 2–9 remain planned.

Milestone 1 was installed using the staged, signature-verified installer, which
backs up the previous Rust AU before replacing it. The installed bundle signature
verified, and `auval -v aumu RbR1 Rbit` reported AU VALIDATION SUCCEEDED. The
installer now stages replacements to avoid overwriting a binary loaded by a host.

## Milestone 2: independent arpeggiator, 2026-11-05

Added a separate Rust arpeggiator and C ABI settings block (36 bytes). The original
96-byte engine parameter block, existing parameter IDs, and version-1 patch format
are retained. New arp IDs are appended. Older patches explicitly receive the new
defaults, with the independent arp disabled. Legacy mono arp modes remain available.

- 35 Rust tests pass. New coverage includes all five patterns, octave expansion
  and note bounds, deterministic random selection, exact gate/swing events, clock
  divisions and sample rates, tempo-change phase preservation, latch replacement,
  sustain/sostenuto and channel controls, polyphony/mono independence, retrigger
  versus legato sequence progression, enable/disable with held notes, invalid
  inputs, and buffer partition invariance.
- The independent-arp allocation test renders a randomized, swung three-octave
  pattern in irregular buffers with no Rust allocation, reallocation, or freeing.
- Clippy passes with warnings denied. All four CTest groups pass, including actual
  AU/VST3 hosting. Native tests verify gate and MIDI sample offsets, new-state
  recall, version-1 legacy defaults/behavior, and clearing latched playback with STOP.
- Visually inspected actual default and populated editor renders. The new ARP
  and MONO tabs fit the existing 1120x930 editor. The standalone accessibility
  tree exposed the new controls and expected defaults alongside a restored
  milestone-1 sequence. A live automated toggle write could not be verified,
  after which no on-screen window was available; it is not counted as a passed
  interaction test. Parameter/control states are covered by native tests and
  actual-editor preview attachments. No audible listening claim is made.
- Generated `dist/rebit-rust-arp-demo.wav`: a four-second 48 kHz stereo PCM16
  demo of two chords, Up/Down, two octaves and swing.

The clock follows tempo and starts on chord input; host transport alignment is
not yet implemented. Retrigger restarts envelopes and sequences; Legato at full
gate preserves them. Shorter gates retrigger after the gap. Release tails follow
the ADSR/volume sequence and can overlap, up to Voices.

Milestone 2's installed AU signature verifies and its executable hash matches
the built AU. Apple `auval -v aumu RbR1 Rbit` reports AU VALIDATION SUCCEEDED.
The prior Rust component is backed up under `dist/au-backups/`; the original C++
Rebit remains untouched.

## Milestone 3: transport synchronization, 2026-11-05

Added independent fixed-layout clock (8-byte) and transport (48-byte) C ABI
blocks; the engine and arp parameter blocks are unchanged. Host sync aligns
musical rates to PPQ, retains chord/latch input through transport stops, resets
patterns on play/seek/cycle, and follows swing and block-reported tempo changes.
Seconds and missing tempo/PPQ use the free clock. Musical sequence frames are
optional; 60 Hz is unchanged. Old version-1 patches load with Host sync off and
60 Hz frames. New fields round-trip in the existing version-1 parameter tree.

- 48 Rust tests passed. New cases cover mid-grid entry/gates, retained latch
  across stop/start, seeks, in-buffer loop wraps, tempo changes and rapid ramps,
  negative preroll, swung grid boundaries, musical sequence resets, stopped
  sequence release tails, invalid host metadata, free/seconds fallback, and
  sub-sample gates leaving no stuck generated notes.
- A tracked allocator test covers transport updates, cycle resets, sequencing,
  telemetry, and irregular render buffers, with no Rust heap allocation/freeing.
  Chunked and continuous renders are identical in the tested host-loop case.
- Clippy passes with warnings denied. All four CTest groups passed.
- Actual AU hosting now supplies beat/tempo and transport/cycle callbacks and
  checks PPQ/gate alignment, stop/start with retained notes, and cycle resets
  inside a render block. AU parameter test values use the wrapper's published
  normalized range. Actual VST3 hosting verifies forwarded PPQ and play/stop.
- Native JUCE tests additionally cover MIDI sample offsets, tempo changes, free
  mode, missing timing fallback, clock-state telemetry and state migration.
  Host callbacks are queried only while processing, not during preparation.
- Actual default/showcase editor renders were inspected: Host sync, FRAME CLOCK,
  clock status and playheads fit the existing editor. The showcase uses a real
  test playhead and rendered audio, rather than a fabricated playback indicator.
  No new interactive Logic or listening test is claimed.

The former sibling JUCE path was absent. The identical pinned official JUCE
8.0.12 source is now in `vendor/JUCE` and the default build uses it. The old
project is no longer needed for native builds. Only generated CMake caches were
refreshed to switch dependency paths. Existing plugin identity is retained.

Host PPQ/tempo are observed at callback boundaries and extrapolated within a
buffer; intra-buffer tempo automation cannot be inferred beyond what the host
reports. Provided cycle bounds support intra-buffer loop wrapping. Other seeks
resync on the next callback. Manual notes remain available while stopped with
arp off; host-synced generated arp notes wait for Play.

An additional regression confirms Host sync switching does not restart Seconds
mode. The installed milestone-3 AU signature verifies, its executable hash
matches the built AU, and `auval -v aumu RbR1 Rbit` reports AU VALIDATION
SUCCEEDED. The previous Rust AU is preserved by the staged installer.

## Milestone 4: smoother live changes, 2026-11-05

Added sample-counted 5 ms gain/sustain ramps and waveform/manual-duty/noise-type
crossfades. Noise algorithms have separate per-voice state. Pitch bend and live
pitch/modulation settings interpolate tuning; ordinary sequence-frame pitch
changes retain their original timing. ADSR/lane changes retain the voice and
blend amplitude where a live edit would jump. A release-time edit recomputes
the current tail's slope, with a 3 ms minimum for an edited zero release.

Voice-count/mono-mode changes no longer reset keys or controllers. Reconciliation
retains the newest held voices and preserves their phase, envelope, and sequence
progress. Mono chooses the latest physical key; expansion restores physically
held keys with fresh envelopes/sequences and a 3 ms fade-in. Discarded pedal-only
notes are not resurrected. Independent-arp reconciliation prioritizes and remaps
its current voice without resetting its clock or chord.

Removed/stolen sounding voices continue as frozen oscillators for a 3 ms
fade-out. There are at most 64 continuations beyond the 64 regular voices;
overflow replaces the quietest continuation. These add bounded transient CPU
cost; no performance speedup or universal click-free claim is made. Normal
zero-release note-offs and arp gates stay immediate. Reset/STOP, channel-local
All Sound Off, and host-synced arp stop clear applicable continuations immediately.

- **63 Rust tests pass.** New regressions measure fade starts/endpoints across
  8–384 kHz, timbre/duty/noise transitions and rapid retargeting, pitch ramps,
  volume-lane transitions versus crisp frame edges, poly/mono restoration,
  retained cursor/voice progress, pedals/controllers, current arp voices in high
  slots, short retirement tails, immediate panic/host stop, and full tail pools.
- A tracked allocator test applies rapid timbre, envelope, pitch, voice-count,
  mode and MIDI edits during rendering. No Rust heap allocations/reallocations
  or freeing occur. Continuous versus irregular 7-sample partitions are identical
  with matching event positions. Existing transport/allocation tests also pass.
- Clippy passes with warnings denied. All four CTest groups pass: Rust, native
  processor, AU host, VST3 host. Native tests verify audible held-key restoration,
  retirement and gain ramps via actual host parameters. The AU host measures the
  gain fade crossing callbacks and settling exactly at 5 ms.
- Native parameter reads now use cached stable atomic pointers and string views,
  initialized once, avoiding repeated JUCE parameter-ID string construction in
  the callback. State-recall checks continue to pass. The allocation instrument
  covers Rust, not all JUCE or host operations.
- Rendered and visually inspected the actual populated editor; layout is
  unchanged and tooltips describe the live transitions. No new interactive Logic
  or audible listening test is claimed.

Parameter IDs, defaults, the 96-byte engine ABI and version-1 state are unchanged.
Automation is received at callback boundaries; ramps then advance per sample.
Sharp native oscillator edges and deliberate note/sequence articulation remain.
The staged installer backed up the prior Rust AU. The installed signature verifies,
its executable SHA-256 matches the built AU, and Apple `auval -v aumu RbR1 Rbit`
reports **AU VALIDATION SUCCEEDED**. Restart Logic to load the replacement.

## Milestone 5: playback feedback, 2026-11-05

The Rust core observes the final finite output sample after summing, fades and
gain. Peak attack is immediate; falloff is 24 dB/second, with a separate one-second
peak hold followed by the same falloff. This is sample-peak feedback, not RMS,
loudness or inter-sample true-peak measurement. Samples at or above full scale
increment a wrapping revision, so a short overload cannot be missed between UI
polls. Observation does not limit or change output.

A new 28-byte read-only playback C ABI snapshot supplies peak/held peak, clip
revision, active/held/released voice counts and retirement-continuation count.
Only the audio thread reads the Rust engine. The processor publishes lock-free
float/integer atomics once per callback; activity counts share one packed atomic
to preserve their relationship across concurrent reads. The display polls at
30 Hz. Clip acknowledgement reads native atomics and never mutates Rust from
the UI. No new parameters or saved fields are introduced; existing engine ABI,
IDs, defaults, plugin identity and version-1 patches remain compatible.

- **70 Rust tests pass.** Added checks for meter calibration/hold/decay at
  8–384 kHz, both clip polarities and the full-scale threshold, reset/revision
  wrapping, post-gain and post-cancellation measurements, silent active voices,
  pedal/release/retirement counts, and playback ABI size/alignment/null handling.
- A tracked allocator test covers metering, telemetry reads, release tails and
  clipped noise playback. There is no Rust heap allocation/reallocation/freeing;
  irregular partitions produce identical audio and final telemetry. Existing
  MIDI, automation and host-clock allocation regressions continue to pass.
- Clippy passes with warnings denied; all four CTest groups pass (Rust core,
  native processor, actual AU hosting, actual VST3 hosting). Native checks verify
  a one-sample clipped burst at MIDI offsets 17/18 survives later silence,
  correct final voice counts, clip history when the real editor opens, its actual
  clear callback, silence not relatching old events, further clipping relatching,
  STOP resetting meter/activity, and concurrent UI reads/clip acknowledgement.
- Rendered and visually inspected the default, populated and clipped editor.
  The clipped case uses actual three-voice output with one voice released,
  showing +3.5 dBFS, a red latch, and amber release activity. Images are saved as
  `preview.png`, `preview-showcase.png`, and `preview-clipping.png`. Tooltips
  explain the meter and clearing CLIP. No new interactive Logic or listening
  test is claimed.

The meter is unified for the current mono source duplicated to output channels.
It reports plugin output before host faders/effects. Held tiles include pedal
retention and generated arp voices; keyboard feedback continues to describe
physical/input keys. Brief retirement tails have a separate count beyond regular
Voices. UI output/activity clear when callbacks cease for 250 ms or three buffer
durations, whichever is longer. The UI timeout is a display precaution rather
than an engine reset; clip history stays latched until acknowledged or prepared.
Ballistics advance in samples, independently of buffer partitioning. UI float
fields are independent atomic observations; voice-count relationships are packed.

Rebuilt and signed AU/VST3/standalone. The staged installer backed up the previous
Rust AU. Installed code signature verification succeeds and executable SHA-256
matches the built AU. Apple `auval -v aumu RbR1 Rbit` reports **AU VALIDATION
SUCCEEDED**. Restart Logic to load the replacement. Two sound layers is next.

## Milestone 6: two sound layers, 2026-11-05

Added a safe Rust stereo layer mixer around two preallocated engines. Each has
its own oscillator/noise state, ADSR, tuning and three compiled sequence lanes.
Layer enable/level/pan use 5 ms ramps; semitone/cent changes use the existing
tuning ramp without changing MIDI note identity. Center balance is unity in both
channels; hard pan silences the other channel. Nominal center/endpoints from
host normalization are snapped within 1e-6 to preserve legacy center behavior.
Mono outputs average stereo, and the output meter observes the actual mix after
downmix in mono, or the larger absolute channel sample in stereo.

Layer A remains the shared performance source even when muted. MIDI/controller,
Voices/mono/glide, arp, modulation settings and frame clock are shared. Disabling
B fades its contribution and skips B synthesis; enabling copies the current
performance state without heap work and starts fresh B voices/envelopes/cursors.
Current generated arp notes, gate/step phase, latch, keys and pedal holds are
retained where represented by A. Expired release tails and previously discarded
pedal-only voices are not resurrected. Re-enabling during an unfinished fade
retains the running B voices. A muted layer A continues its voices and clock.

Voices limits each engine to 1–64 regular voices; there are at most 128 regular
oscillators plus 128 bounded 3 ms retirement continuations. The B engine is still
preallocated and receives bounded MIDI/parameter updates while off. A remains
active as the performance source, so a B-only patch still has A's processing
cost. This tradeoff avoids a new third clock/voice scheduler in this milestone.

The original 96-byte parameter ABI and existing parameter IDs remain unchanged.
New layer parameters are appended; a separate 12-byte layer mix ABI and seeded
second-engine allocation, tuning, mixer render/telemetry calls are added.
Distinct engines and output buffers are required at the unsafe FFI boundary;
null right output explicitly selects mono. The UI only reads published atomics.
Each layer has a separate packed activity word and three atomic playheads.

New version-2 state stores both banks. Version-1 loading explicitly resets all
new layer controls to defaults and clears B. It preserves A's prior sound and
existing arp/clock fields, with the earlier migrations for fields absent from
older patches. Both banks are validated before changing state, published under
one try-lock handoff, and retired outside the callback. Older binaries reject
version 2. Plugin identity is unchanged.

- **81 Rust tests pass.** Added single-engine output parity for all oscillator
  types, separate stereo routing, mixed peak/clip detection, mono downmix,
  independent sequence/envelope releases, late layer enable with held/pedal
  notes, sleeping B, short switch/level/pan ramps, tuning/cents continuity,
  late arp enable/host stop alignment, 128-voice limits, finite invalid inputs,
  layer ABI/FFI mono/stereo and engine-alias guards.
- The dual-layer allocation regression performs repeated enable/disable, pan,
  level, tuning, MIDI and release changes with no Rust allocation, reallocation
  or freeing. Irregular 7-sample partitions produce identical stereo output and
  final meter/activity telemetry for matching event positions.
- Clippy passes with warnings denied, and all four CTest groups pass. Actual AU
  and VST3 hosts verify A/B panning with different oscillator types. Native tests
  additionally verify per-layer activity, real-editor selector/attachment routing,
  drawing into B and enabling only B's lane, preserving invalid/valid drafts on
  layer switch, version-2 recall, version-1 mix/B reset, rejecting a malformed B
  bank before any partial replacement, and concurrent publication to both banks.
- Rendered and inspected default, A-selected showcase, B-selected showcase, and
  clipping editor states. The new 1320x930 layout retains the earlier controls
  and adds the layer side panel. Previews are `preview.png`,
  `preview-showcase.png`, `preview-layer-b.png`, and `preview-clipping.png`.
  No new interactive Logic or audible listening test is claimed.
- Generated `dist/rebit-rust-layers-demo.wav`: 4 seconds, 48 kHz stereo PCM16,
  pulse plus independently sequenced/detuned triangle sub, separate pan.
  Peaks are 0.2144 left / 0.1979 right; 191,986 of 192,000 frames differ between
  channels. The renderer's `--layers` flag reproduces it.

### Local processing-cost check

`cargo run --release --offline --locked --example layer_budget` renders one
second at 48 kHz in 128-frame buffers, taking the median of three runs. On this
Apple M5 Pro, the observed times were:

| Workload | Render wall time / 1 s audio |
| --- | ---: |
| A only, 8 static voices | 3.43 ms |
| A+B, 8 static voices each | 6.58 ms |
| A+B, 64 voices each with vibrato | 88.32 ms |

These are offline throughput measurements, not realtime scheduling or device
latency guarantees; native host/UI overhead and other modulation/retirement
workloads are outside this small benchmark. No speedup over the original C++
plugin is claimed.

Rebuilt/signed AU, VST3 and standalone; installed the AU with the staged installer
and retained the previous Rust component. The installed signature verifies,
its executable SHA-256 matches the built AU, and Apple `auval -v aumu RbR1 Rbit`
reports **AU VALIDATION SUCCEEDED**. Restart Logic to load the replacement.
Milestone 7, Raw/Clean modes and effects, is next.

## Milestone 7: Raw/Clean and effects, 2026-11-05

Raw keeps the prior pulse, stepped-triangle and noise calculation. Clean adds
quadratic polynomial step residuals at pulse edges, and their cubic integrals
at the two corners of an unquantized triangle. This is local edge/corner
correction, with no tables, heap work, lookahead or block delay. Background on
the ramp-correction approach is in [Esqueda, Välimäki and Bilbao, DAFx-16,
Rounding Corners with BLAMP](https://research.spa.aalto.fi/publications/papers/dafx16-blamp/).
The implementation here is a two-sided short polynomial, not a reproduction of
that paper's reported experiments. No external implementation was vendored.

Mode is independent per layer and crossfades over 5 ms without resetting voice
phase, envelope or cursors. Retirement oscillators freeze the current mode blend
and frequency. Noise stays unchanged. Clean does not bandlimit intentional note,
pitch/duty/volume-sequence discontinuities, nor the subsequent colour effects.
High-frequency fundamentals can be attenuated by the short correction; Clean
is not level/spectrum identical to Raw and is not claimed to be alias-free.

The shared post-mix chain is normalized tanh saturation, stereo-linked
sample-and-hold, quantization, then dry/wet. It runs after any mono downmix and
before metering. Controls and bypass ramp for 5 ms. Drive 0 dB, 24 bits and
divider 1 are neutral; fully bypassed effects return dry samples directly and
skip the colour calculation. Quantization does not clamp above-full-scale
values; there is no new output limiter. A hold can extend ordinary zero-release
note-offs by up to 31 host samples. STOP, channel All Sound Off, and host-synced
arp stop clear shared hold history immediately. Other channels are recaptured
from the surviving mix after channel All Sound Off.

Existing parameter IDs and the 96-byte core ABI are retained; sound-mode calls
and a separate 20-byte effects block are added. Version 3 saves the appended
controls. Version-1/2 recall explicitly restores Raw/FX bypass, retaining their
earlier one/two-layer migrations. Older binaries reject version 3. No DSP state
is shared with the UI; parameter atomics and audio-thread ownership are retained.

- **96 Rust tests pass.** Added oscillator bounds/DC and edge/corner checks,
  direct non-harmonic-energy regressions for three pulse widths and triangle,
  held-voice 5 ms mode transitions, return-to-Raw phase parity, unchanged noise,
  exact bypass/neutral effects, quantization steps, linked stereo hold, odd/finite
  saturation including its zero-drive limit, invalid settings, dry blend,
  bypass ramp/history reset across 8–384 kHz, effect tails, post-FX metering,
  effects ABI and null-boundary handling.
- Tracked allocation tests cover rapid Clean/Raw and effect automation during
  dual-layer playback. No Rust allocation/reallocation/freeing occurs; 7-sample
  partitions produce identical stereo audio to continuous renders at matching
  event positions. Clearing effects plus All Sound Off produces immediate zero.
- Clippy passes with warnings denied. All four CTest groups pass. Native checks
  cover real-editor mode attachment switching, FX enable, version-3 recall,
  version-2 fallback preserving B, sample-offset panic with an active hold,
  host stop with FX on, and post-effect output metering. Actual AU and VST3
  hosts verify Clean's initial edge, saturation, and eight-sample hold groups;
  AU additionally checks panic clearing the shared effect history at offset 3.
- Before implementation, retained the milestone-6 renderer and rendered pulse,
  noise and layered four-second demos. After implementation all three WAVs
  compare byte-for-byte identical with Raw/FX bypass; this is a scoped
  regression check rather than an all-patch sample-identity guarantee.
- Rendered and inspected default, populated, B-selected, clipping, and Clean/FX
  editor states. SOUND MODE and the four master FX controls fit the existing
  1320x930 side panel. `preview-colour.png` shows actual processed Clean/FX
  playback. No interactive Logic or audible listening test is claimed.
- `dist/rebit-rust-colour-demo.wav` is 8 seconds at 48 kHz stereo PCM16. Sections
  are Raw (0–2 s), Clean (2–4), Clean+saturation (4–6), and Clean+8-bit/8x hold
  (6–8). Measured peak is 0.3932; no PCM samples clip. Reproduce with
  `cargo run --release --offline --locked --example colour_demo`.

### Actual-engine spectral comparison

`quality_spectrum` exports actual Engine output at 48 kHz in coherent 32,768-frame
records, with FX bypassed and steady envelopes. `scripts/analyze_quality.py`
measures non-harmonic positive-frequency power divided by in-band harmonic
power, excluding DC. Clean lowers that ratio by 15.82–39.53 dB across the tested
12 shape/frequency pairs (about 625.5, 2998.5 and 7999.5 Hz). At 2998.5 Hz:

| Shape | Raw alias/harmonic | Clean alias/harmonic | Improvement |
| --- | ---: | ---: | ---: |
| Pulse 12.5% | -9.03 dB | -27.80 dB | 18.77 dB |
| Pulse 25% | -11.46 dB | -28.16 dB | 16.70 dB |
| Pulse 50% | -12.75 dB | -28.96 dB | 16.21 dB |
| Triangle | -23.65 dB | -47.86 dB | 24.21 dB |

Data, full metrics and inspected plots are in `dist/quality-analysis/`:
`metrics.json`, `spectra.svg`, `spectra.png` and the indexed `.f32` records.
These results apply to the steady tested pitches, not arbitrary modulation,
onsets, noise or nonlinear FX. They are not a subjective listening comparison.

```
cargo run --release --offline --locked --example quality_spectrum
python3 scripts/analyze_quality.py
```

The analysis script needs NumPy/Matplotlib; those were installed only in a
temporary analysis environment, not added as plugin or Rust dependencies.

### Processing cost

Same Apple M5 Pro / 48 kHz / 128-frame offline workload as milestone 6, median of
three one-second renders using `--example layer_budget`:

| Workload | Render wall time / 1 s audio |
| --- | ---: |
| A Raw, 8 static voices | 4.78 ms |
| A Clean, 8 static voices | 4.76 ms |
| A+B Raw, 8 each | 7.41 ms |
| A+B Clean, 8 each | 7.71 ms |
| A+B Raw, 64 each with vibrato | 74.39 ms |
| A+B Clean, 64 each with vibrato | 88.66 ms |
| A+B Clean, 8 each plus all FX | 7.76 ms |

Small-case differences include measurement variability. These are offline
throughput observations, not realtime scheduling, CPU-percentage or device
latency guarantees, and exclude host/UI overhead. No speedup over C++ is claimed.

Rebuilt and signed AU/VST3/standalone. The staged installer preserved the prior
Rust AU; installed signature verification succeeds and its executable SHA-256
matches the built AU. Apple `auval -v aumu RbR1 Rbit` reports **AU VALIDATION
SUCCEEDED**. Restart Logic to load this build. Modulation/expression is next.

## Milestone 8: modulation and expression, 2026-11-05

Added four modulation routes, two LFOs (sine, triangle, square, ramp up, ramp
down, sample and hold), four macros, per-note velocity/aftertouch/timbre
sources, and pitch, pulse-width, level, layer-balance, FX-drive and FX-mix
destinations. Per-note routes run per voice; layer balance and effect offsets
are stepped once per sample in the layer mixer. MPE uses per-channel bend,
channel pressure, poly aftertouch and CC74, with a separate bend range.

- **105 Rust tests pass** (78 unit tests and 27 integration tests). New checks
  cover LFO bounds and sample-and-hold, source/destination scaling, aftertouch
  and mod-wheel routing, layer-balance and FX-drive destinations, deterministic
  LFO output, per-channel bend isolation, chunk invariance, allocation-free
  rendering, C ABI layout and null handling.
- `cargo clippy --offline --locked --all-targets -- -D warnings`: passed.
- All four CTest groups passed: Rust core, AU host, native processor, VST3 host.
- Native checks exercised channel pressure through a real modulation route,
  version-4 state recall, legacy version-3 defaults, and real-editor MOD, LFO,
  MACROS, route and MPE controls.
- Release AU, VST3 and standalone builds succeeded. The installed AU signature
  verifies and `auval -v aumu RbR1 Rbit` reports **AU VALIDATION SUCCEEDED**.
- Actual-editor previews were rendered and inspected at 1.5x:
  `preview-mod.png`, `preview-lfo.png`, `preview-macros.png`; default, showcase,
  layer-B, colour and clipping previews were refreshed.
- `dist/rebit-rust-mod-demo.wav` was rendered from the actual two-layer mixer:
  base sound, aftertouch swell, macro A/B crossfade, then combined LFO motion.

Limits: modulation LFOs are free-running rather than host-tempo-synced. Shared
layer-balance and effect destinations use layer A's LFO clock; per-note routes
use each voice's own LFO copy. Macros are not MIDI-learnable in this milestone.
MPE is tested at the raw MIDI/channel level; no hardware MPE controller or
listening test was performed. The original C++ Rebit remains unchanged.

## Milestone 9: presets and musical randomization, 2026-11-05

Added a factory and user preset browser with 12 built-in patches across Init,
Bass, Lead, Percussion, Game and Pad. Selection loads a complete patch after
resetting all parameters and both sequence banks; previous/next buttons browse
the same list. Preset loads are undoable. The sequence-bearing Noise Hat and
Power Up presets were checked to load valid text and lane enables. User presets
save the complete current state as `.rebitrust` files under
`~/Library/Application Support/Rebit Audio/Rebit Rust/Presets`, appear as
`User - <name>`, can be overwritten by name, and can be deleted while factory
patches remain protected.

Added deterministic seeded randomization with six lock sections: oscillator,
envelope, sequences, modulation, effects and mix. Unlocked sections randomize
both layers' oscillators and envelopes, generate valid volume/pitch/duty
patterns with lane enables and frame clock, pick one to three matrix routes plus
LFOs and macros, optionally configure effects, and choose voices/layers/levels/
pans/tuning/master gain. The arpeggiator and MPE mode are intentionally not
randomized. Preset loads and randomizations share a bounded eight-entry undo
stack. The editor opens at 1320x1080 with a new PRESETS / RANDOM panel.

- **105 Rust tests pass.** No Rust DSP changes were required for this milestone;
  the existing modulation/allocation tests continue to pass.
- `cargo clippy --offline --locked --all-targets -- -D warnings`: passed.
- All four CTest groups passed. Native checks cover factory categories and
  labels, full preset recall, a sequence-bearing preset, user preset
  save/load/overwrite/delete, temporary-directory cleanup, same-seed determinism
  (byte-identical state), different-seed divergence, locked envelope/mix
  sections, randomized sequence validity, undo restoration after preset load
  and randomization, and real-editor preset/seed/lock/randomize/save/delete/
  undo controls.
- Release AU, VST3 and standalone builds succeeded. The installed AU signature
  verifies and `auval -v aumu RbR1 Rbit` reports **AU VALIDATION SUCCEEDED**.
- Actual-editor previews were rendered and inspected: `preview-presets.png`
  (factory browser and save row), `preview-user.png` (a loaded user preset),
  and `preview-random.png` (seeded randomize with ENV locked, status text,
  random sequence and controls). Earlier previews were refreshed for the
  1320x1080 layout.

Limits: user presets are plain files in the standard Application Support folder;
there is no import/export or custom-folder chooser in the editor. The random
seed is an editing aid and is not stored in the plugin state. Randomization
does not touch arpeggiator or MPE settings, and there is no MIDI-learn for
macros. No listening test or DAW UI interaction test was performed. The original
C++ Rebit remains unchanged.
