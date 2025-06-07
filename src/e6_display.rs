pub use crate::display::{AsRgbColor, Color, Display, Error, PartialUpdate, RgbColor};
use crate::nibbles_vec::NibblesVec;
use core::ops::{RangeInclusive, SubAssign};
use core::time::Duration;
use defmt::{Format, info};
use embedded_graphics::Pixel;
use embedded_graphics::geometry::Size;
use embedded_graphics::prelude::{DrawTarget, OriginDimensions, PixelColor};
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin, PinState};
use embedded_hal::spi::SpiDevice;

pub(crate) const SPI_CHUNK_SIZE: usize = 4096;
pub(crate) const RESET_DELAY_MS: u32 = 30;
pub(crate) const BUSY_WAIT_DELAY_MS: u32 = 100;
pub(crate) const BUSY_WAIT_TIMEOUT_MS: Duration = Duration::from_millis(20_000);

pub(crate) const INIT_SEQUENCE: &[(CommandCode, &[u8])] = &[
    (CommandCode::INIT, &[0x49, 0x55, 0x20, 0x08, 0x09, 0x18]),
    (CommandCode::PWR, &[0x3F]),
    (CommandCode::PSR, &[0x5F, 0x69]),
    (CommandCode::BTST1, &[0x40, 0x1F, 0x1F, 0x2C]),
    (CommandCode::BTST3, &[0x6F, 0x1F, 0x1F, 0x22]),
    (CommandCode::BTST2, &[0x6F, 0x1F, 0x17, 0x17]),
    (CommandCode::POFS, &[0x00, 0x54, 0x00, 0x44]),
    (CommandCode::TCON, &[0x02, 0x00]),
    (CommandCode::PLL, &[0x08]),
    (CommandCode::CDI, &[0x3F]),
    (CommandCode::TRES, &[0x03, 0x20, 0x01, 0xE0]),
    (CommandCode::PWS, &[0x2F]),
    (CommandCode::VDCS, &[0x01]),
];

pub struct E6Display<DC: OutputPin, RST: OutputPin, BUSY: InputPin, SPI: SpiDevice, DELAY: DelayNs>
{
    spi: SPI,
    dc_pin: DC,
    rst_pin: RST,
    busy_pin: BUSY,
    width: u16,
    height: u16,
    delay_source: DELAY,
    frame_buffer: NibblesVec<E6Color>,
}

#[repr(u8)]
#[derive(Format, Copy, Clone)]
#[allow(dead_code)]
pub(crate) enum CommandCode {
    PSR = 0x00,
    PWR = 0x01,
    POF = 0x02,
    POFS = 0x03,
    PON = 0x04,
    BTST1 = 0x05,
    BTST2 = 0x06,
    DSLP = 0x07,
    BTST3 = 0x08,
    DTM1 = 0x10,
    DSP = 0x11,
    DRF = 0x12,
    PLL = 0x30,
    CDI = 0x50,
    TCON = 0x60,
    TRES = 0x61,
    REV = 0x70,
    VDCS = 0x82,
    PTL = 0x83,
    PWS = 0xE3,
    INIT = 0xAA,
}

pub(crate) enum DataCommand {
    Data,
    Command,
}

impl<DC: OutputPin, RST: OutputPin, BUSY: InputPin, SPI: SpiDevice, DELAY: DelayNs>
    E6Display<DC, RST, BUSY, SPI, DELAY>
{
    pub fn new(
        width: u16,
        height: u16,
        spi: SPI,
        dc_pin: DC,
        rst_pin: RST,
        busy_pin: BUSY,
        delay_source: DELAY,
    ) -> Self {
        Self {
            spi,
            dc_pin,
            rst_pin,
            busy_pin,
            width,
            height,
            delay_source,
            frame_buffer: NibblesVec::with_len(width as usize * height as usize),
        }
    }

    fn power_off(&mut self) -> Result<(), Error> {
        self.spi_write_command(CommandCode::POF)
    }

    fn power_on(&mut self) -> Result<(), Error> {
        self.spi_write_command(CommandCode::PON)
    }

    fn display_refresh(&mut self) -> Result<(), Error> {
        self.spi_write_command_and_data(CommandCode::DRF, &[0x00])
    }

    fn set_data_command(&mut self, data_command: DataCommand) -> Result<(), Error> {
        self.dc_pin
            .set_state(if let DataCommand::Data = data_command {
                PinState::High
            } else {
                PinState::Low
            })
            .map_err(Error::from_digital_pin_error)
    }

    fn spi_write_command(&mut self, command: CommandCode) -> Result<(), Error> {
        self.set_data_command(DataCommand::Command)?;
        self.spi
            .write(&[command as u8])
            .map_err(Error::from_spi_error)
    }

    fn spi_write_data(&mut self, data: &[u8]) -> Result<(), Error> {
        info!("Sending data chunk: {}", data.len());
        self.set_data_command(DataCommand::Data)?;
        self.spi.write(&data).map_err(Error::from_spi_error)?;
        Ok(())
    }

    fn spi_write_command_and_read<const R: usize>(
        &mut self,
        command: CommandCode,
    ) -> Result<[u8; R], Error> {
        let mut result = [0u8; R];
        self.set_data_command(DataCommand::Command)?;
        self.spi
            .transfer(&mut result, &[command as u8])
            .map_err(Error::from_spi_error)?;
        Ok(result)
    }

    fn spi_write_command_and_data(
        &mut self,
        command: CommandCode,
        data: &[u8],
    ) -> Result<(), Error> {
        self.spi_write_command(command)?;
        self.spi_write_data(data)?;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Error> {
        self.rst_pin
            .set_low()
            .map_err(Error::from_digital_pin_error)?;
        self.delay_source.delay_ms(RESET_DELAY_MS);
        self.rst_pin
            .set_high()
            .map_err(Error::from_digital_pin_error)?;
        self.delay_source.delay_ms(RESET_DELAY_MS);
        self.busy_wait()?;
        Ok(())
    }

    fn busy_wait(&mut self) -> Result<(), Error> {
        self.busy_wait_timeout(BUSY_WAIT_TIMEOUT_MS)
    }

    fn busy_wait_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        info!("The display could be busy, waiting...");
        let mut count = (timeout.as_millis() as u32 / BUSY_WAIT_DELAY_MS) + 1;
        while count > 0
            && self
                .busy_pin
                .is_low()
                .map_err(Error::from_digital_pin_error)?
        {
            self.delay_source.delay_ms(BUSY_WAIT_DELAY_MS);
            count.sub_assign(1);
        }
        info!("The display is free, continue...");
        Ok(())
    }

    fn refresh_display(&mut self) -> Result<(), Error> {
        self.power_on()?;
        self.busy_wait()?;
        //self.spi_write_command_and_data(CommandCode::BTST2, &[0x6F, 0x1F, 0x17, 0x49])?;
        self.display_refresh()?;
        self.busy_wait()?;
        self.power_off()?;
        self.busy_wait()?;
        Ok(())
    }

    fn send_frame_buffer(&mut self) -> Result<(), Error> {
        let mut buf = [0u8; SPI_CHUNK_SIZE];
        let len = self.frame_buffer.as_slice().len();
        let mut index = 0;

        self.spi_write_command(CommandCode::DTM1)?;
        while index < len {
            let chunk_size = (len - index).min(SPI_CHUNK_SIZE);
            (&mut buf[0..chunk_size])
                .copy_from_slice(&self.frame_buffer.as_slice()[index..index + chunk_size]);
            self.spi_write_data(&buf[0..chunk_size])?;
            self.busy_wait()?;
            index += chunk_size;
        }
        let result: [u8; 1] = self.spi_write_command_and_read(CommandCode::DSP)?;
        info!("Frame buffer sent: {}, index: {}", result, index);
        self.busy_wait()?;
        Ok(())
    }

    fn pixel_index(&self, x: usize, y: usize) -> usize {
        y * self.width as usize + x
    }
}

impl<DC: OutputPin, RST: OutputPin, BUSY: InputPin, SPI: SpiDevice, DELAY: DelayNs> Display<E6Color>
    for E6Display<DC, RST, BUSY, SPI, DELAY>
{
    fn initialize(&mut self) -> Result<(), Error> {
        info!("Initialize display");
        self.reset()?;
        for (command_code, data) in INIT_SEQUENCE {
            self.spi_write_command_and_data(*command_code, data)?;
        }
        Ok(())
    }
    fn update(&mut self, iter: impl IntoIterator<Item = E6Color>) -> Result<(), Error> {
        let mut iter = iter.into_iter();
        for index in 0..self.frame_buffer.len() {
            if let Some(color) = iter.next() {
                self.frame_buffer.set(index, color);
            }
        }
        Ok(())
    }

    fn refresh(&mut self) -> Result<(), Error> {
        self.send_frame_buffer()?;
        self.refresh_display()
    }

    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }
}

impl<DC: OutputPin, RST: OutputPin, BUSY: InputPin, SPI: SpiDevice, DELAY: DelayNs>
    PartialUpdate<E6Color> for E6Display<DC, RST, BUSY, SPI, DELAY>
{
    fn partial_update(
        &mut self,
        iter: impl IntoIterator<Item = E6Color>,
        horizontal: RangeInclusive<u16>,
        vertical: RangeInclusive<u16>,
    ) -> Result<(), Error> {
        let mut iter = iter.into_iter();
        for y in vertical {
            for x in horizontal.clone() {
                if let Some(color) = iter.next() {
                    self.frame_buffer
                        .set(self.pixel_index(x as usize, y as usize), color.into());
                }
            }
        }
        Ok(())
    }
}

const E6_PALETTE: [RgbColor; 6] = {
    [
        (0, 0, 0),
        (255, 255, 255),
        (255, 255, 0),
        (255, 0, 0),
        (0, 0, 255),
        (0, 255, 0),
    ]
};

#[derive(Format, Copy, Clone, PartialOrd, PartialEq)]
#[repr(u8)]
pub enum E6Color {
    Black = 0,
    White = 1,
    Yellow = 2,
    Red = 3,
    Blue = 5,
    Green = 6,
}

impl AsRgbColor for E6Color {
    fn rgb_color(&self) -> RgbColor {
        E6_PALETTE[*self as usize]
    }
}

impl Color for E6Color {}

// Embedded Graphics Impl
impl PixelColor for E6Color {
    type Raw = ();
}

impl From<u8> for E6Color {
    fn from(value: u8) -> Self {
        match value {
            0 => E6Color::Black,
            1 => E6Color::White,
            2 => E6Color::Yellow,
            3 => E6Color::Red,
            5 => E6Color::Blue,
            6 => E6Color::Green,
            _ => panic!("Unknown E6 color index {}", value),
        }
    }
}

impl From<E6Color> for u8 {
    fn from(value: E6Color) -> Self {
        value as u8
    }
}

impl<DC: OutputPin, RST: OutputPin, BUSY: InputPin, SPI: SpiDevice, DELAY: DelayNs> OriginDimensions
    for E6Display<DC, RST, BUSY, SPI, DELAY>
{
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl<DC: OutputPin, RST: OutputPin, BUSY: InputPin, SPI: SpiDevice, DELAY: DelayNs> DrawTarget
    for E6Display<DC, RST, BUSY, SPI, DELAY>
{
    type Color = E6Color;
    type Error = Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels.into_iter().take(self.frame_buffer.len()) {
            self.frame_buffer
                .set(self.pixel_index(p.x as usize, p.y as usize), c);
        }
        Ok(())
    }
}
