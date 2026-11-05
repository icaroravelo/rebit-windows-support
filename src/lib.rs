//! Rebit Rust: framework-independent synthesis and sequence compilation.
pub mod effects;
pub mod engine;
mod ffi;
pub mod layers;
mod meter;
pub mod modulation;
pub mod oscillator;
pub mod sequence;
mod smoothing;
pub use engine::{Engine, Parameters, PlaybackInfo};
pub use modulation::{LfoParameters, ModRoute, ModulationParameters};
pub use sequence::{Sequence, SequenceBank};

pub mod arpeggiator;
pub use arpeggiator::ArpParameters;

pub mod transport;
pub use transport::{ClockParameters, TransportInfo};
