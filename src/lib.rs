#![no_std]

extern crate alloc;

pub mod display;
pub mod e6_display;
mod nibbles_vec;

pub mod prelude {
    pub use crate::e6_display::Display;
    pub use crate::e6_display::E6Color;
    pub use crate::e6_display::E6Display;
    pub use crate::e6_display::PartialUpdate;
}
