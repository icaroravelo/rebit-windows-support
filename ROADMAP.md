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
4. **Smoother live changes — next**
   - Preserve held notes when voice count/mono mode changes where practical.
   - Smooth audible parameter jumps and define waveform-change transitions.
5. **Playback feedback — planned**
   - Real output peak meter and clipping indication.
   - Voice activity and useful playback feedback alongside parameter previews.
   - Sequence playheads are included in milestone 1.
6. **Two sound layers — planned**
   - Independent oscillator, tuning, gain/pan, envelope, and sequences.
   - Pulse + triangle sub, melodic + noise, detune and stereo combinations.
   - Voice/CPU budget and migration from single-layer patches.
7. **Raw and Clean sound modes — planned**
   - Preserve raw chip sound; offer reduced-aliasing oscillators.
   - Optional saturation, bit-depth and sample-rate reduction, with bypass.
   - Compare CPU and spectra; avoid an unmeasured performance claim.
8. **Modulation and expression — planned**
   - Velocity, wheel, aftertouch, and LFO routing; performance macros.
   - Pulse width, pitch, layer balance, and effects destinations.
   - Add MPE as a later part of this milestone, including the JUCE MIDI bridge.
9. **Presets and musical randomization — planned**
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
