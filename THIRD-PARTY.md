# Provenance and dependencies

This edition is derived from the independently reconstructed Rebit C++
implementation, provided under AGPL-3.0-or-later. Its JUCE editor
and parameter layout are adapted here; the DSP and sequence compiler are
implemented in Rust. No original Magical8bitPlug2 source or artwork was used.

The Rust crate uses only Rust's standard library and has no third-party crates.
Rust distributions include their own applicable MIT/Apache-2.0 and third-party
notices; preserve applicable dependency notices when distributing binaries.

Native plugin builds use JUCE 8.0.12, vendored at `vendor/JUCE` from the official
https://github.com/juce-framework/JUCE repository, commit
29396c22c93392d6738e021b83196283d6e4d850. JUCE is used under its AGPLv3 option.
Its complete notices and bundled dependency licenses are in that checkout,
including AudioUnitSDK and the VST3 SDK. Preserve the corresponding dependency
source and notices when redistributing. This stock checkout does not need the
older C++ project's original-preset identity patch; Rebit Rust does not import
original Magical8bitPlug2 presets or state.
