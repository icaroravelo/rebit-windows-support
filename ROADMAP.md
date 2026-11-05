# Rebit Rust roadmap

Each milestone is implemented, checked, and delivered before starting the next.
The original C++ Rebit stays unchanged. Preserve Rust plugin identity, existing
parameter IDs, and version-1 patches; migrate state explicitly when needed.

1. **Visual sequence editor — completed and installed**
   - Draw volume, pitch, and duty steps; keep text editing.
   - Resize patterns, page through long sequences, and edit with the keyboard.
   - Drag loop/release markers; show actual engine playback positions.
   - Hear edits during playback without restarting the current pattern.
2. **Independent arpeggiator — completed and installed**
   - Dedicated enable switch, independent of mono behavior/voice count.
   - Up, down, up/down, random, and played-order patterns; octave range.
   - Latch, gate length, swing; common note lengths, dotted and triplet options.
   - Define envelope retrigger and interaction with the pitch sequence.
3. **Transport synchronization — completed and installed**
   - Align arpeggios and optional sequence clocks with Logic's beat position.
   - Handle start/stop, seek, loops, tempo changes, and free-running standalone.
4. **Smoother live changes — completed and installed**
   - Preserve held notes when voice count/mono mode changes where practical.
   - Smooth audible parameter jumps and define waveform-change transitions.
5. **Playback feedback — completed and installed**
   - Real output peak meter and clipping indication.
   - Voice activity and useful playback feedback alongside parameter previews.
   - Sequence playheads are included in milestone 1.
6. **Two sound layers — completed and installed**
   - Independent oscillator, tuning, gain/pan, envelope, and sequences.
   - Pulse + triangle sub, melodic + noise, detune and stereo combinations.
   - Voice/CPU budget and migration from single-layer patches.
7. **Raw and Clean sound modes — completed and installed**
   - Preserve raw chip sound; offer reduced-aliasing oscillators.
   - Optional saturation, bit-depth and sample-rate reduction, with bypass.
   - Compare CPU and spectra; avoid an unmeasured performance claim.
8. **Modulation and expression — completed and installed**
   - Velocity, wheel, aftertouch, and LFO routing; performance macros.
   - Pulse width, pitch, layer balance, and effects destinations.
   - Add MPE as a later part of this milestone, including the JUCE MIDI bridge.
9. **Presets and musical randomization — completed and installed**
   - Preset browser and useful bass, lead, percussion, and game-effect patches.
   - Section-selective randomization, locks, deterministic seeds, and undo.

Checks per milestone: targeted Rust/native regressions, release builds,
AU/VST3 host checks, actual-editor visual inspection for UI changes, and
installed AU validation. Record remaining limits in VALIDATION.md.

## Milestone 1 delivery

Completed 2026-11-05. Installed the updated Rust AU; retained the previous Rust
component under `dist/au-backups/`. 19 Rust tests and all four CTest groups pass,
Clippy passes with warnings denied, and Apple AU validation succeeds. The
actual-editor preview is `preview-showcase.png`. Restart Logic to load this build.
See README.md for the visual editor's mouse/keyboard controls and VALIDATION.md
for evidence and limits.

## Milestone 2 delivery

Completed 2026-11-05. Installed the independent-arp AU using the staged installer;
the previous Rust AU is backed up. 35 Rust tests and all four CTest groups pass;
Clippy passes, and Apple AU validation succeeds. The default and populated
editor previews are updated; `dist/rebit-rust-arp-demo.wav` demonstrates the new
arp. Existing patches retain legacy behavior. Restart Logic to load this build.
Milestone 3, transport synchronization, is next.

## Milestone 3 delivery

Completed 2026-11-05. Installed transport synchronization and optional musical
sequence clocks. 48 Rust tests and all four CTest groups pass, including actual
AU host PPQ/start-stop/cycle callbacks and VST3 forwarded transport. Clippy
passes and installed Apple AU validation succeeds. Old patches retain Free /
60 Hz timing; turn Host sync on explicitly in an existing patch. New instances
default to Host sync, with standalone fallback. Restart Logic to load this build.

JUCE 8.0.12 is now pinned locally under `vendor/JUCE`; native builds no longer
depend on the missing sibling project folder. Milestone 4 is next.

## Milestone 4 delivery

Completed 2026-11-05. Installed smoother live changes: 5 ms parameter/timbre
transitions, held-key/controller preservation through voice-count and mono-mode
changes, and bounded 3 ms fades for removed/stolen voices. Restored physical keys
start fresh voices; retained voices preserve their progress. Independent-arp
changes preserve the current note and clock. Normal MIDI gates and chip sequence
steps keep their timing. Existing parameter IDs, defaults, ABI and patch schema
are retained.

63 Rust tests, all four CTest groups, and Clippy with warnings denied pass.
The installed AU signature verifies, its executable matches the build, and
Apple AU validation succeeds. The prior Rust AU is backed up. Restart Logic to
load this build. Milestone 5, playback feedback, is next.

## Milestone 5 delivery

Completed 2026-11-05. Installed real output sample-peak metering, a resettable
clip latch, and 64 voice-activity tiles. Held/pedal voices, releases and brief
fade continuations are distinguished; the waveform and ADSR stay parameter
previews. Meter hold/decay follow audio time, and the UI clears stale activity
when the host suspends callbacks. Metering does not limit or modify output.

70 Rust tests, all four CTest groups, and Clippy with warnings denied pass.
Native checks include one-sample overloads between UI polls, actual-editor clip
reset, relatching, and concurrent telemetry reads. Default, showcase and clipping
editor renders were inspected and saved. The installed AU signature verifies,
its executable matches the build, and Apple AU validation succeeds. The prior
Rust AU is backed up. Restart Logic to load it. Milestone 6, two sound layers,
is next.

## Milestone 6 delivery

Completed 2026-11-05. Added independent A/B oscillator, ADSR and sequence banks,
layer level, stereo pan, semitone transpose and cent detune. Performance/MIDI,
arpeggiator, modulation and master level stay shared. Layer B is off by default
and skips synthesis when off. The 1320x930 editor has an A/B selector and layer
side panel, with separate playheads and activity rows. Combined stereo output
feeds the meter; mono outputs average the stereo mix.

Version-2 state stores both layers. Version-1 Rust patches migrate into A with
unity/center/zero added tuning and B disabled/cleared. Existing parameter IDs,
plugin identity and original parameter ABI remain intact. 81 Rust tests, Clippy
and all four CTest groups pass, including AU/VST3 stereo layers and native state
migration/editor routing. Offline throughput results are in VALIDATION.md; the
stereo demo is `dist/rebit-rust-layers-demo.wav`.

The installed AU signature and executable hash verify, and Apple AU validation
passes. The previous Rust component is backed up. Restart Logic, turn B on, and
use EDIT A / EDIT B to choose the sound/sequence controls. Milestone 7 is next.

## Milestone 7 delivery

Completed 2026-11-05. Raw/Clean is selectable per layer, with 5 ms transitions.
Clean uses polynomial pulse-edge and triangle-corner correction; noise is kept
unchanged. Added shared saturation, bit-depth reduction, stereo-linked sample
hold and dry/wet with bypass. Neutral/bypassed settings preserve dry output.
Three previous-build pulse/noise/layered demo WAVs remain byte-identical.

96 Rust tests, Clippy and all four CTest groups pass, including actual AU/VST3
Clean/effect rendering. Coherent actual-engine spectra show roughly 16–40 dB
lower alias-to-harmonic ratios in the tested cases; results, limitations and
offline throughput comparisons are in VALIDATION.md and `dist/quality-analysis`.
An eight-second comparison is `dist/rebit-rust-colour-demo.wav`.

Version-3 patches save the new controls. Version-1/2 patches migrate to Raw and
FX bypass while preserving their established layer/state migration. The AU is
installed with a verified signature/hash and passes Apple AU validation; the
prior Rust AU is backed up. Restart Logic, select SOUND MODE per layer, and turn
FX on to use the master chain.

## Milestone 8 delivery

Completed 2026-11-05. Added a four-route modulation matrix with velocity,
mod-wheel, channel/poly aftertouch, CC74 timbre, two LFOs, four macros, LFO
unipolar sources, and pitch, pulse-width, level, layer-balance, FX-drive and
FX-mix destinations. Per-note routes use per-voice LFO copies; shared balance
and effects are stepped once per sample by the layer mixer, so rendering stays
buffer-size independent. LFOs support sine, triangle, square, ramp up/down and
sample-and-hold, with optional note retrigger and 0.01–20 Hz rates.

MPE is exposed as a MACROS-page switch with a separate bend range. Raw
per-channel pitch bend, channel pressure, poly aftertouch and CC74 are handled
per member channel without an extra bridge. Version-4 state stores routes, LFOs,
macros and MPE settings; version-1/2/3 patches load with all routes off, default
LFOs, zero macros and MPE off, preserving their previous sound.

105 Rust tests, Clippy with warnings denied, and all four CTest groups pass,
including per-note pressure/bend, route scaling, layer-balance and FX-drive
checks, chunk invariance, allocation checks, editor routing, state recall and
legacy migration. The AU, VST3 and standalone builds pass; Apple AU validation
succeeds after installation. Previews are `preview-mod.png`, `preview-lfo.png`
and `preview-macros.png`; the eight-second demo is
`dist/rebit-rust-mod-demo.wav`.

## Milestone 9 delivery

Completed 2026-11-05. Added a factory and user preset browser with 12 built-in
patches across Init, Bass, Lead, Percussion, Game and Pad categories,
previous/next selection, and undoable loads. Preset loads return all parameters
and both sequence banks to defaults before applying the patch, so recall is
complete and repeatable. **SAVE** writes the complete current state to
`~/Library/Application Support/Rebit Audio/Rebit Rust/Presets`; user patches
appear under **User - <name>**, can be overwritten by name, and **DEL** removes
the selected user file while protecting factory patches.

Added seeded randomization with six independent locks: oscillator, envelope,
sequences, modulation, effects and mix. The same starting patch, seed and locks
produce the same result. Unlocked sections produce musical bounds: both layers'
oscillator/envelope settings, valid volume/pitch/duty patterns and lane enables,
one to three matrix routes with LFO and macro settings, an optional effects
configuration, and layer/voice/level/tuning choices. Arpeggiator and MPE mode
stay under performance control.

Both preset loads and randomizations push a bounded eight-entry undo stack. The
editor gains a 150-pixel PRESETS / RANDOM panel and now opens at 1320x1080; the
keyboard and footer moved down to match. Previews are `preview-presets.png`,
`preview-user.png` and `preview-random.png`.

105 Rust tests, Clippy with warnings denied, and all four CTest groups pass.
Native checks cover preset categories and recall, sequence-bearing presets,
user preset save/load/overwrite/delete, temporary-directory cleanup,
deterministic same-seed output, different-seed output, locked sections, undo,
valid randomized sequence text, editor save/delete controls and undo
restoration. The AU, VST3 and standalone builds pass; the installed AU signature verifies and Apple
AU validation succeeds. The original C++ Rebit remains unchanged.
