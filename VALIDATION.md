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
