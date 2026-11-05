# Rebit Rust

A separate Rust edition of Rebit, with its own plugin identity. Builds use the
Rust core and the pinned JUCE source in this folder; the earlier C++ project is
not required.

**Rust owns synthesis, voices, MIDI behavior, modulation, and sequence parsing.**
A small C++/JUCE layer supplies the existing editor, host parameters, saved state,
and AU / VST3 / standalone packaging. This is a hybrid plugin, not an all-Rust UI
or a binary-compatible replacement for the original Rebit.

The development milestones are tracked in [ROADMAP.md](ROADMAP.md).

## Interface

The editor uses a game-console-inspired dark chassis, an original vector pixel
wordmark, lime/cyan accents, glowing waveform and ADSR previews, stepped rotary
rings, and a custom keyboard with octave scrolling. All artwork and glow effects
are drawn with JUCE paths and gradients; no image assets or external fonts are
required. Waveform and envelope displays are parameter previews. The OUTPUT
meter and voice tiles show actual processed playback.

Knob values remain editable, and double-click resets a knob to its default.
The independent arpeggiator works at every voice count. The MONO tab holds the
legacy one-voice modes; glide is available there with Arp off. The MOD tab
holds four modulation routes, while VIB, LFO and MACROS select the shared
sources. Colours separate oscillator, amplitude, performance, modulation, and
sequence controls.

See `preview.png` for the default patch, `preview-showcase.png` for a populated
patch, `preview-layer-b.png` for B's controls, `preview-colour.png` for Clean/FX,
`preview-mod.png` for the route matrix, `preview-lfo.png` for LFO sources,
`preview-macros.png` for macros and MPE, `preview-presets.png` for the preset
browser, `preview-user.png` for a loaded user preset, `preview-random.png` for
seeded randomization with a locked envelope, and `preview-clipping.png` for
output above full scale with a releasing voice.
All are rendered from the actual editor and audio processor; preview patches do
not change defaults.

### Two sound layers

Turn **B on** in SOUND LAYERS to add the second sound. **EDIT A / EDIT B** select
which layer the oscillator, ADSR, pitch units and three sequence lanes edit.
Selecting a layer does not change which layers you hear. Each layer has its own
**LAYER LEVEL**, **PAN**, **TRANSPOSE** (-24..+24 semitones), and **DETUNE**
(-100..+100 cents). Tuning does not change the MIDI key identity or note-off.

A starts on with its previous pulse sound, unity layer level and centered pan.
B starts off, with a triangle oscillator, -12 semitones and 0.50 layer level.
For a sub bass, turn B on and leave it one octave down. For a wide lead, tune
both to the same octave, detune one slightly and pan them apart. Noise and
melodic layers can use different envelopes and volume/pitch/duty patterns.

**MASTER**, Voices, MIDI/pedals, mono/glide, arpeggiator, modulation and FRAME
CLOCK are shared. Voices limits each layer separately: 8 allows eight voices
in A and eight in B. Each voice has one oscillator and its own envelope/sequence
cursors. There are at most 128 regular voices plus 128 short removal fades.

Layer levels, balance and switches fade over 5 ms during playback; tuning edits
use the existing short pitch ramp. Pan uses linear balance: center is unity in
both channels, hard left/right silences the opposite channel. Mono output is the
average of stereo. Layer A continues its performance while muted. Disabling B
stops its synthesis after the fade; enabling it restarts its own envelopes and
sequences from currently held/pedal-retained notes or the current arp step.
Expired release tails and discarded pedal-only voices are not recreated.

The editor is now 1320x1080, with the layer controls in a side panel and a
preset/random panel below the sequencer. Invalid
sequence drafts must be corrected before switching layers. Each layer's bank
and playhead stay separate; editing B does not change A's sequence text.

New patches use state **version 4**. Version-1 Rust patches load into A with
unity layer level, center pan and zero added tuning; B is explicitly disabled
and cleared. Version-2 patches retain both layers. Old formats load in Raw with
effects off and with every modulation route disabled, both LFOs at their
defaults, macros at zero and MPE off. Existing parameter IDs and plugin identity
are retained. New states need this build or later; earlier builds reject
version 4.

An offline stereo example is `dist/rebit-rust-layers-demo.wav`. Reproduce it with:

```
cargo run --release --offline --locked --bin rebit-render -- layers.wav pulse --layers
```

B skips oscillator rendering while off; A remains the shared performance source
even when muted. Both engines and the mixer are preallocated. For a reproducible
offline processing-cost check, use `cargo run --release --offline --locked
--example layer_budget`; local results and limits are recorded in VALIDATION.md.

### Raw / Clean and master effects

Choose **EDIT A / EDIT B**, then **SOUND MODE → Clean** to reduce oscillator
aliasing on that layer. **Raw** keeps the original pulse, stepped triangle and
noise. Clean smooths pulse edges and uses a continuous triangle with corrected
corners; noise remains unchanged. Switching blends over 5 ms without restarting
held voices or sequences. High notes can sound rounder/quieter in Clean.

Clean reduces oscillator aliasing rather than removing all possible aliasing:
discrete sequence changes, note gates, noise and optional effects can still
produce high-frequency content. The waveform display remains an illustrative
parameter preview. Measurement plots and limits are in
`dist/quality-analysis/spectra.svg` and VALIDATION.md.

**FX on** enables the shared output chain for both layers:

- **DRIVE:** 0–24 dB normalized soft saturation; 0 dB is neutral.
- **BITS:** 4–24-bit quantization; 24 bypasses quantization.
- **RATE DIV:** stereo-linked sample-and-hold, 1–32 host samples per capture.
  1x is neutral; 8x at a 48 kHz host gives a 6 kHz capture rate.
- **MIX:** 0–100% processed sound. FX off returns dry audio after a short fade.

The order is saturation → sample hold → quantization → dry/wet blend, after
layer mixing/MASTER and any mono downmix, before the output meter. Controls and
bypass transitions use 5 ms ramps. Quantization and held-sample edges are
intentional. These effects add colour and can add aliasing even with Clean
oscillators; they are not an output limiter. Saturation can raise output level.

Sample hold may retain the last sample for up to 31 host samples after an
ordinary zero-release note-off. STOP, All Sound Off and a host-synced arp stop
clear its history immediately. There is no lookahead or block-buffering delay.
Effects are bypassed by default, and neutral FX settings preserve the dry signal.

`dist/rebit-rust-colour-demo.wav` compares the same phrase in four two-second
sections: Raw, Clean, Clean+saturation, and Clean+8-bit/8x reduction. Reproduce:

```
cargo run --release --offline --locked --example colour_demo
```

### Playback feedback

The top-right **OUTPUT** meter measures the larger absolute stereo sample after
both layers' levels, pans, MASTER and colour effects, before the DAW's fader or
downstream effects. It shows sample peaks in
dBFS, from -60 to +6 dB on the bar, with instantaneous attack and 24 dB/second
falloff. The white marker and numeric value hold the peak for one second before
falling. Silence reads **-INF dBFS**. This is a sample-peak meter, not a true-peak
or loudness meter.

**CLIP** latches red when any output sample reaches 0 dBFS or higher. Click it
to clear; further overload lights it again. Reduce MASTER or layer levels to
make room. The floating-point output is not limited or altered by the meter.
Clip history survives closing the editor and STOP; preparation clears it.

Below the waveform, **A / B** report each layer's regular voices versus the Voices
setting. Each row has 64 tiles: mint held/pedal voices for A, violet for B, and
amber release stages. **+FADE** counts the brief extra continuations from voice removal
or stealing. A silent enabled volume sequence may still have an active voice;
the output meter reflects the resulting silence. **KEYS HELD** refers to input
keys, so a latched arpeggio can keep playing with **NO KEYS HELD**.

The display refreshes at 30 Hz. When callbacks stop, meter/activity clear after
at least 250 ms (or three buffer durations for long buffers) and show **AUDIO
IDLE**; the clip latch remains available. Engine ballistics advance in audio
samples and remain independent of processing-buffer size.

### Modulation and expression

**PERFORMANCE → MOD** contains four independent routes. Each route chooses a
source, a destination, and a bipolar **AMOUNT** from -100% to +100%. Negative
amounts invert the source. **MODULATION → VIB / LFO / MACROS** selects the
shared sources and legacy vibrato controls.

Sources:

- **Velocity, aftertouch and timbre CC74** are per note. Aftertouch combines
  channel pressure and polyphonic aftertouch, so MPE timbre and pressure work
  per member channel.
- **Mod wheel** is the channel's CC1 value.
- **LFO 1 / LFO 2** run at 0.01–20 Hz with sine, triangle, square, ramp up, ramp
  down, or sample-and-hold shapes. **Retrigger** restarts a LFO at phase zero on
  the first note of a chord. Per-note routes use a copy of the LFO, so each voice
  keeps its own phase; shared balance and effect routes use one clock.
- **Macro 1–4** are 0–100% performance controls, saved with the patch.
- **LFO unipolar** versions map -1..+1 to 0..1, useful for one-sided movement.

Destinations:

- **Pitch** spans ±12 semitones at 100% amount.
- **Pulse width** shifts the pulse threshold by ±50%; triangle and noise are
  unaffected.
- **Level** multiplies amplitude from 0 to 2x at 100%.
- **Layer balance** crossfades A and B: positive favours B, negative A. It only
  attenuates, and it is evaluated on layer A's shared source clock. Shared
  destinations use the newest active voice for velocity and aftertouch.
- **FX drive** adds ±12 dB before the colour chain; **FX mix** shifts the dry/wet
  blend. These are shared destinations and require the effects to be on.

Velocity still scales every voice directly and the mod wheel still drives
delayed vibrato when **Independent of mod wheel** is off. Routes are additive
where it makes sense; route off is exactly neutral and preserves the previous
sound. The matrix is sanitized on the audio thread: non-finite or out-of-range
settings fall back to neutral and rendering remains allocation-free.

**MPE** is available on the MACROS page. Per-channel pitch bend, channel
pressure, poly aftertouch and CC74 timbre are forwarded as raw MIDI; no extra
bridge is required. MPE mode uses **MPE BEND** (default 48 semitones) instead of
the normal bend range. Note identity remains channel-plus-note, so each member
channel bends independently while the lower-zone master channel still controls
chord-wide messages. Channels are not limited to the lower zone; the engine
accepts per-note expression on any of the 16 MIDI channels.

`dist/rebit-rust-mod-demo.wav` demonstrates the base sound, an aftertouch swell,
a macro A/B crossfade, and combined LFO motion. Reproduce it with:

```
cargo run --release --offline --locked --example modulation_demo
```

Modulation LFOs are free-running, not host-tempo-synced. Shared balance/effect
routes use layer A's clock; if you only need per-note movement, either layer can
carry the route. The editor previews are `preview-mod.png`, `preview-lfo.png`
and `preview-macros.png`.

### Presets and musical randomization

Section **07 PRESETS / RANDOM** holds the preset browser and randomization
controls. The factory list starts with **Init** and includes bass, lead,
percussion, game-effect and pad patches, for example **Sub Pulse**, **Rubber
Bass**, **Wobble**, **Arcade Lead**, **Glide Lead**, **Noise Hat**, **Chip
Kick**, **Coin**, **Laser**, **Power Up** and **Wide Layers**. Select a patch
with the list or the previous/next buttons.

To save the current sound, type a name in **USER PRESET NAME** and press
**SAVE** (or Return). The complete version-4 state is written to
`~/Library/Application Support/Rebit Audio/Rebit Rust/Presets/<name>.rebitrust`
and appears in the list under **User - <name>**. Saving does not change the
sound; loading is undoable. **DEL** removes the selected user preset, while
factory presets are protected. Existing user presets with the same name are
overwritten.

Loading a preset first returns every parameter and both sequence banks to their
defaults, then applies the patch. This makes recall complete and repeatable;
the previous patch is saved so **UNDO** restores it. Preset loads stop sounding
voices rather than carrying the old performance into the new sound.

Randomization is deterministic. Type a **SEED** or press **NEW**, leave the
sections you want to keep unchanged locked with **OSC / ENV / SEQ / MOD / FX /
MIX**, then press **RANDOMIZE**. The same starting patch, seed and locks produce
the same result. Each randomization and preset load is undoable; the last eight
states are retained.

- **OSC** picks both layers' waveform, pulse width, noise type and Raw/Clean mode.
- **ENV** picks both layers' attack, decay, sustain and release within musical
  ranges.
- **SEQ** generates valid volume, pitch and duty patterns for both layers,
  chooses lane enables and sets the frame clock. Pitch patterns use a diatonic
  set and stay inside the accepted range.
- **MOD** picks one to three routes, LFO shapes/rates/retrigger and macro values.
  MPE settings stay unchanged.
- **FX** chooses bypass or an effects setting with drive, bit depth, sample rate
  and mix.
- **MIX** picks voices, master gain, layer enables, levels, pans, transpose and
  detune. Layer A stays enabled.

The arpeggiator and MPE mode are performance settings, not randomization
targets, so a randomized patch does not surprise you by enabling them. The
seed field is not part of the saved patch or presets; it is an editing aid, while
the resulting patch is saved normally in version-4 state.

## Run it

On this Mac, after building, open:

```
open "build/RebitRust_artefacts/Release/Standalone/Rebit Rust.app"
```

Select MIDI input in the standalone's Options menu or play the on-screen keys.
The AU and VST3 builds are in the sibling `AU` and `VST3` output folders.
Nothing is installed automatically. To use a plugin in a DAW, copy **Rebit
Rust.component** into `~/Library/Audio/Plug-Ins/Components`, or **Rebit Rust.vst3**
into `~/Library/Audio/Plug-Ins/VST3`, then rescan/restart the host. These names and
identities differ from Rebit's, so both versions can coexist.

## Build

Requires Rust/Cargo, CMake 3.22+, and Xcode command-line tools on macOS. No
third-party Rust crates or network access are needed.

```
./scripts/build.sh
```

The default JUCE source is the pinned 8.0.12 checkout in `vendor/JUCE`. Native
builds are independent of the old project. To use another JUCE 8 checkout:

```
./scripts/build.sh -DJUCE_PATH=/absolute/path/to/JUCE
```

Builds target the current Mac architecture (the verified local build is arm64).
Universal/x86_64 cross-build automation is not provided yet; CMake rejects an
architecture override rather than silently linking an incompatible Rust archive.
Builds are locally ad-hoc signed, not notarized.

The Rust core and offline renderer also work independently of JUCE:

```
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
cargo run --release --offline --locked --bin rebit-render -- demo.wav pulse
```

Use `triangle` or `noise` instead of `pulse` for other demos. The renderer writes
four seconds of 48 kHz stereo PCM16 audio without opening an audio device.
It clips only during PCM conversion; the plugin itself has no output limiter.
Use a lower gain when stacking many voices.

## Instrument

- Pulse (12.5%, 25%, 50%), stepped triangle, 4-bit random, and long/short LFSR noise.
- Two independently tuned/panned oscillator layers; 1–64 voices and ADSR per layer.
- Velocity, per-channel pitch bend and mod wheel, channel pressure, poly aftertouch and CC74 timbre.
- Four-route modulation matrix, two LFOs, four macros and MPE note expression.
- Mono legato with glide and return to the previous held note; retrigger mode;
  legacy ascending/descending arpeggios with frame, tempo, or seconds clocks.
- Independent arpeggiator: five patterns, 1–4 octaves, gate, swing, latch,
  retrigger/legato, and straight/dotted/triplet/seconds clocks.
- Delayed vibrato and automatic pitch sweep.
- Sustain (CC64), sostenuto (CC66), reset controllers (CC121), All Notes Off
  (CC123, respecting pedals), and channel-local All Sound Off (CC120).
- Volume, pitch, and pulse-duty sequences, compiled outside audio processing.
- Host automation and versioned state saving/recall.

Select **1 voice** with **Arp off** for the legacy mono/glide modes. Voice-count
and mono-mode changes retain physically held keys and controller state. With
the independent arp on, the held chord and current generated note survive those
changes. Pitch is capped below Nyquist for extreme
notes/modulation. Raw oscillators are not bandlimited; Clean reduces aliasing
with polynomial edge/corner correction.

### Live changes

Gain and held-note sustain edits ramp over **5 ms**. Waveform, manual pulse width,
and noise-type changes crossfade over the same interval without restarting
oscillator phase, envelopes, or sequences. Pitch bend and live pitch/modulation
settings use a short tuning ramp. Changing attack/decay or switching the volume
lane blends from the current amplitude; changing release during a tail recalculates
its remaining slope, with a 3 ms minimum when turning release down to zero.
Changes made while silent take effect immediately for the next note.

Reducing Voices keeps the newest held notes, with the latest physical key chosen
for mono. Kept voices retain their envelope and sequence progress. Raising Voices
restores other physically held keys, newest first, with a **3 ms** fade-in; these
restored voices start fresh envelopes/sequences. Pedal-only notes that lost their
voice are not restored. The retained voices keep sustain/sostenuto and channel
controllers. Independent-arp voice-count changes keep its clock and current note.

Removed or stolen sounding voices get a **3 ms** oscillator fade-out. Up to 64
short continuations per layer can exist beyond the selected Voices count; if full, the
quietest continuation is replaced. These transitions reduce edit/steal clicks;
the raw chip oscillators still contain deliberate sharp edges. Normal note
attacks, zero-release note-offs, arp gates, and sequence-frame steps keep their
timing. STOP, All Sound Off, and a host-synced arp stop silence applicable tails
immediately. Host parameter changes are observed at processing-block boundaries;
the ramps themselves run per sample and do not depend on buffer size.

## Independent arpeggiator

In **PERFORMANCE → ARP**, turn **Arp on** on and hold a chord. Any voice count
works. **Voices** limits overlapping generated notes and their release tails.
The default is Up at 1/16, one octave, 65% Gate, no Swing, and Retrigger.

- **Pattern:** Up, Down, Up/Down (without repeated end notes), Random, Played
  order. Played order keeps key-entry order within each octave.
- **Rate:** quarter through 64th notes, plus dotted (1.5x) and triplet (2/3)
  versions. Rates follow host tempo; standalone uses 120 BPM without a host.
  Choose Seconds to unlock **Step Time**, 0.010–2.000 seconds per unswung step.
- **Octaves:** one to four, expanded upward. MIDI pitches above 127 are skipped.
- **Gate:** 5–100% of each swung step is held. Gate starts the release stage;
  release tails can continue after the gate closes.
- **Swing:** 0–49% alternately lengthens/shortens steps while preserving each
  pair's total duration. It works with musical and seconds rates.
- **Latch:** released notes keep playing. After all physical keys lift, a new
  chord replaces the latched chord. Turn Latch off to discard unheld notes that
  pedals are not retaining. **STOP** stops all voices and clears held/latch and
  controller state; a new chord restarts the enabled arp.
- **Envelope / Retrigger:** every generated step restarts oscillator phase,
  ADSR, sweep/vibrato timing, and all three sequences. Release tails may overlap.
- **Envelope / Legato:** at 100% Gate, pitches change within the same continuous
  voice, so envelope and sequence progress continue. Shorter gates create gaps
  and the next note starts fresh. This articulation does not add glide.

The pitch sequence adds its offset to the generated arpeggio pitch. For a first
arp test, switch the Pitch sequence off. Sustain and sostenuto retain input
chord notes while the arp still observes Gate. All Notes Off clears latch for
that channel (respecting pedals); All Sound Off clears that channel immediately.
Random state is local and deterministic, independent of other plugin instances.

The **MONO** tab retains the old parameter IDs and modes for earlier patches.
Its legacy arp is overridden when the independent **Arp on** switch is enabled.
Version-1 patches from before the independent arp load with it off and its new
settings at defaults. Later version-1 arp settings are preserved. New settings
are saved with normal host state; plugin identity is unchanged.
The arp generates internal synth notes, not MIDI output to other instruments.

### Host transport synchronization

**Host sync**, in the ARP panel, aligns musical independent-arp rates and musical sequence
frames with the host's PPQ grid. It is on for fresh instances. Patches saved
before this feature load with it off, preserving their previous chord-start
behavior. Turn it on explicitly in an existing Logic patch to use transport sync.

- A chord entered between grid boundaries waits for the next boundary.
- Play starts, seeks, and cycle wraps restart the arp pattern while retaining the
  physical/latched chord. Swing pairs are anchored at PPQ zero, including preroll.
- Stop silences generated arp voices, including their tails, while retaining the
  chord for the next Play. To audition the arp while Logic is stopped, switch
  **Host sync off**, or select a Seconds rate.
- **Seconds** remains free-running and independent of host play/stop.
- Without valid tempo and PPQ, the clock falls back to free-running. The
  standalone therefore remains playable. The display reports HOST PLAY,
  HOST STOP, FREE CLOCK, or FREE / NO HOST from the processed clock state.
- Loop points supplied by the host allow cycle wrapping within an audio buffer;
  other discontinuities are detected at the next callback. Host timing and tempo
  are sampled once per callback and extrapolated within it. Tempo ramps are
  followed at that resolution, rather than claiming sample-accurate tempo events.

**FRAME CLOCK** selects 60 Hz or a musical duration for both layers' sequence lanes.
60 Hz keeps the previous note-relative behavior. Musical frames can be quarter,
eighth, sixteenth, 32nd or 64th notes, or eighth/sixteenth triplets. They use the
host grid when Host sync is on and the host is playing; otherwise they use the
selected tempo with a note-relative clock. They do not inherit arp swing.

A new note starts at its first sequence value and advances at the next grid
boundary. Play/seek/cycle resets held voices' musical-sequence cursors; release
cursors are retained. While stopped, manual synth notes and sequence release
tails can finish on their tempo-relative clock. The independent arp itself still
obeys Host sync's stop behavior. Patch recalls reset normally, and drawing a
sequence continues to preserve valid cursor progress.

To render an offline chord/arpeggio demo:

```
cargo run --release --offline --locked --bin rebit-render -- arp.wav pulse --arp
```

## Sequences

Select **EDIT A / EDIT B**, then the **VOLUME**, **PITCH**, or **DUTY** tab and draw in the grid. Drawing
creates a 16-step pattern if the lane is empty and enables that lane. Edits are
heard during playback; the current frame is retained where it remains valid.
The three switches above the grid enable/disable lanes independently of the tab.

- **STEPS** sets the expanded pattern length (0..65,534); press Return to apply.
  Growing fills with 15 for volume or 0 for pitch/duty; shrinking truncates values.
- **LOOP** enables/disables a repeating main section. Drag its marker to move it.
- **RELEASE** assigns the final four steps as a note-off tail (or creates a tail
  for a one-step pattern). Drag REL to choose its starting boundary.
- Arrow buttons page through 16 frames at a time; long patterns remain intact.
- With the grid focused, Left/Right select a step, Up/Down change it by one unit,
  Home/End jump to the first/last step, L sets the loop start, and R sets the
  release boundary at a selected step after the first.
- Pitch drawing has +/-12, +/-24, and FULL range views. The drawing range is
  independent of coarse/fine pitch units. Arrow edits support -64..63.
- The white dot/highlight is an actual Rust engine playhead, following the most
  recently started active voice. Disabled/finished lanes show no playhead.

The **TEXT** field stays available for the selected lane. Press Return, leave
the field, or click **APPLY TEXT**. Invalid text retains the last compiled pattern.
Graphical edits update the text; text edits update the graph. Markers are encoded
in the existing sequence text grammar; version-4 state stores both banks and
the modulation matrix.

Values advance at **FRAME CLOCK**, 60 Hz by default. Examples:

```
Volume: 15,12,8[4,2]|1,0
Pitch:  0,4,7[12,7,4]|0
Duty:   0,1[2,1]|0
Ramp:   15 to 0 in 10
Repeat: 15 x 4,8 x 4,0
```

Volume is 0..15, pitch -64..63, and duty 0..2. Fine pitch units are 1/8 semitone.
A final `[ ... ]` section loops while held. `|` starts a release section on note
off (after pedal holds end). Without a loop, the final main value holds. Clearing
a field clears the lane's data; an enabled empty volume lane produces silence.
After a release lane finishes, volume becomes silent, pitch returns to zero,
and duty returns to its parameter setting. Each lane accepts at most 65,534
expanded frames. Invalid edits keep the last valid sequence.

The grammar deliberately rejects malformed legacy forms, ignored trailing text,
empty release tails, zero/negative repeats, and overflowing integers. Original
Rebit/Magical8bitPlug2 presets and DAW states are not imported. Rust patches use
`RebitRust` version 4 state, with version-1/2/3 migration, and cannot overwrite an
original plugin instance.

## Changes from the C++ edition

- Deterministic random state belongs to each voice/engine. There is no libc
  `rand()` and no interaction with other plugin instances.
- Sequence updates exchange ownership of compiled banks under a try-lock.
  The callback never waits for the producer and never copies the large sequence
  arrays. Superseded/retired banks are freed by the control thread. Active
  sequence cursors retain their progress when a bank is replaced; shortening a
  pattern clamps main positions, and release offsets are retained where valid.
- Static pitches cache their increments; per-sample pitch calculations are
  reserved for active glide, sweep, or vibrato. Sequence ticks and MIDI/parameter
  changes refresh cached pitches.
- ADSR decay and release use the selected duration for the amplitude traversed.
- Mono legato returns to the newest remaining held key. Voice stealing favors
  released voices, then the oldest; extreme notes receive no special protection.
- The 96th-note arpeggio clock uses tempo instead of reproducing the original's
  frame-clock fallback. Tempo changes take effect during playback.
- Inert legacy parameters and original-state import are omitted.

Sound and timing intentionally differ from the reverse-engineered edition.

## Layout and ownership

- `src/engine.rs`: safe Rust, fixed voice/key storage; no render/MIDI allocation.
- `src/arpeggiator.rs`: fixed-storage chord handling, patterns, gate/swing clock.
- `src/transport.rs`: validated host timing and musical frame clock definitions.
- `src/smoothing.rs`: sample-counted ramps and three-way oscillator crossfades.
- `src/meter.rs`: output sample peaks, peak hold/decay, and clip-event revision.
- `src/layers.rs`: preallocated two-engine stereo mixer, layer fades and output metering.
- `src/oscillator.rs`: Clean pulse/triangle polynomial edge/corner correction.
- `src/effects.rs`: bypassable shared saturation, hold and quantization chain.
- `src/sequence.rs`: strict bounded compiler and sequence cursor.
- `src/ffi.rs`: the Rust library's unsafe boundary, opaque C handles.
- `native/RustEngine.h`: matching C ABI, no Rust structs with unstable layout.
- `native/PluginProcessor.*`: host adapter, state, and sequence-bank ownership.
- `native/PluginEditor.*`: adapted JUCE editor.
- `native/SequenceCanvas.*`: graphical sequence editing and atomic playhead display.
- `tests/`: Rust regressions, native processor integration, VST3 host, editor preview.

After preparation, only the audio thread accesses the two engines and mixer.
The control thread owns both canonical sequence banks under `textMutex`. It
publishes cloned banks under `sequenceLock`. The audio callback uses a try-lock
to swap each publication with its engine's bank, leaving retired banks for
control-thread reclamation. State recall publishes both banks under one lock.
Parsing, cloning, and freeing banks never happen in the callback. Processor
teardown requires the host to stop callbacks first. Hosts must call state
serialization/restoration outside realtime processing; these operations allocate.
The allocation test covers Rust rendering and MIDI, not every JUCE/host code path.
Release Rust panics abort; a panic is not a recoverable audio-thread error.

## Verification

```
ctest --test-dir build --output-on-failure
./build/rebit_rust_preview /tmp/rebit-rust-preview.png
```

See `VALIDATION.md` for checks performed on this checkout and remaining limits.

## License

AGPL-3.0-or-later, like the original Rebit implementation; see `LICENSE` and
`THIRD-PARTY.md`. This is an independent instrument, not an official YMCK update.
# rebit
