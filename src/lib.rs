//! Rebit Rust: framework-independent synthesis and sequence compilation.
pub mod engine;
mod ffi;
pub mod sequence;
pub use engine::{Engine, Parameters};
pub use sequence::{Sequence, SequenceBank};

pub mod arpeggiator;
pub use arpeggiator::ArpParameters;

pub mod transport;
pub use transport::{ClockParameters, TransportInfo};
