#![no_std]

extern crate alloc;
#[cfg(feature = "async")]
pub mod async_e6_display;
pub mod display;

pub mod e6_display;
mod nibbles;

pub mod prelude {
    pub use crate::display::Display;
    pub use crate::e6_display::E6Color;
    pub use crate::nibbles::Nibbles;
    pub use crate::nibbles::NibblesIterator;

    #[cfg(feature = "blocking")]
    pub use crate::e6_display::BlockingDisplay;
    #[cfg(feature = "blocking")]
    pub use crate::e6_display::E6Display;
    #[cfg(feature = "blocking")]
    pub use crate::e6_display::PartialUpdate;

    #[cfg(feature = "async")]
    pub use crate::async_e6_display::AsyncE6Display;
    #[cfg(feature = "async")]
    pub use crate::display::AsyncDisplay;
    #[cfg(feature = "async")]
    pub use crate::display::AsyncPartialUpdate;
}
