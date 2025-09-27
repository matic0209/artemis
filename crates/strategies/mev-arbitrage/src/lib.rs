#[cfg(feature = "full")]
mod full;
#[cfg(feature = "full")]
pub use full::*;

#[cfg(not(feature = "full"))]
mod stub;
#[cfg(not(feature = "full"))]
pub use stub::*;
