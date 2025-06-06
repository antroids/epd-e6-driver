use core::fmt::Debug;
use core::ops::RangeInclusive;
use defmt::Format;
use embedded_hal::{digital, spi};

#[derive(Debug)]
pub enum Error {
    SpiError(spi::ErrorKind),
    DigitalPinError(digital::ErrorKind),
}

#[derive(Format)]
pub struct Pixel<C: Color> {
    x: u16,
    y: u16,
    color: C,
}

pub trait Display<C: Color> {
    fn initialize(&mut self) -> Result<(), Error>;
    fn update(&mut self, iter: impl IntoIterator<Item = C>) -> Result<(), Error>;
    fn refresh(&mut self) -> Result<(), Error>;
    fn width(&self) -> u16;
    fn height(&self) -> u16;

    fn len(&self) -> usize {
        self.width() as usize * self.height() as usize
    }
}

pub trait PartialUpdate<C: Color> {
    fn partial_update(
        &mut self,
        iter: impl IntoIterator<Item = C>,
        horizontal: RangeInclusive<u16>,
        vertical: RangeInclusive<u16>,
    ) -> Result<(), Error>;
}

impl Error {
    pub fn from_spi_error<ERR: spi::Error>(err: ERR) -> Self {
        Self::SpiError(err.kind())
    }

    pub fn from_digital_pin_error<ERR: digital::Error>(err: ERR) -> Self {
        Self::DigitalPinError(err.kind())
    }
}

pub type RgbColor = (u8, u8, u8);

pub trait AsRgbColor {
    fn rgb_color(&self) -> RgbColor;
}

pub trait Color {}
