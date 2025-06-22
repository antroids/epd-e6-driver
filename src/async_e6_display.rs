use crate::display::{AsyncDisplay, AsyncPartialUpdate, Display, Error};
use crate::e6_display::{
    CommandCode, DataCommand, E6Color, INIT_SEQUENCE, RESET_DELAY_MS, SPI_CHUNK_SIZE,
};
use crate::nibbles::Nibbles;
use core::ops::RangeInclusive;
use defmt::info;
use embedded_graphics::Pixel;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_hal::digital::{OutputPin, PinState};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::SpiDevice;

pub struct AsyncE6Display<
    DC: OutputPin,
    RST: OutputPin,
    BUSY: Wait,
    SPI: SpiDevice,
    DELAY: DelayNs,
    S: AsMut<[u8]> + AsRef<[u8]>,
> {
    spi: SPI,
    dc_pin: DC,
    rst_pin: RST,
    busy_pin: BUSY,
    width: u16,
    height: u16,
    delay_source: DELAY,
    frame_buffer: Nibbles<S, E6Color>,
}

impl<
    DC: OutputPin,
    RST: OutputPin,
    BUSY: Wait,
    SPI: SpiDevice,
    DELAY: DelayNs,
    S: AsMut<[u8]> + AsRef<[u8]>,
> AsyncE6Display<DC, RST, BUSY, SPI, DELAY, S>
{
    pub fn new(
        width: u16,
        height: u16,
        spi: SPI,
        dc_pin: DC,
        rst_pin: RST,
        busy_pin: BUSY,
        delay_source: DELAY,
        frame_buffer: Nibbles<S, E6Color>,
    ) -> Self {
        assert!(
            frame_buffer.len() < width as usize * height as usize,
            "Frame Buffer has not enough space for all pixels"
        );
        Self {
            spi,
            dc_pin,
            rst_pin,
            busy_pin,
            width,
            height,
            delay_source,
            frame_buffer,
        }
    }

    async fn power_off(&mut self) -> Result<(), Error> {
        self.spi_write_command(CommandCode::POF).await
    }

    async fn power_on(&mut self) -> Result<(), Error> {
        self.spi_write_command(CommandCode::PON).await
    }

    async fn display_refresh(&mut self) -> Result<(), Error> {
        self.spi_write_command_and_data(CommandCode::DRF, &[0x00])
            .await
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

    async fn spi_write_command(&mut self, command: CommandCode) -> Result<(), Error> {
        self.set_data_command(DataCommand::Command)?;
        self.spi
            .write(&[command as u8])
            .await
            .map_err(Error::from_spi_error)
    }

    async fn spi_write_data(&mut self, data: &[u8]) -> Result<(), Error> {
        info!("Sending data chunk: {}", data.len());
        self.set_data_command(DataCommand::Data)?;
        self.spi.write(&data).await.map_err(Error::from_spi_error)?;
        Ok(())
    }

    async fn spi_write_command_and_read<const R: usize>(
        &mut self,
        command: CommandCode,
    ) -> Result<[u8; R], Error> {
        let mut result = [0u8; R];
        self.set_data_command(DataCommand::Command)?;
        self.spi
            .transfer(&mut result, &[command as u8])
            .await
            .map_err(Error::from_spi_error)?;
        Ok(result)
    }

    async fn spi_write_command_and_data(
        &mut self,
        command: CommandCode,
        data: &[u8],
    ) -> Result<(), Error> {
        self.spi_write_command(command).await?;
        self.spi_write_data(data).await?;
        Ok(())
    }

    async fn reset(&mut self) -> Result<(), Error> {
        self.rst_pin
            .set_low()
            .map_err(Error::from_digital_pin_error)?;
        self.delay_source.delay_ms(RESET_DELAY_MS).await;
        self.rst_pin
            .set_high()
            .map_err(Error::from_digital_pin_error)?;
        self.delay_source.delay_ms(RESET_DELAY_MS).await;
        self.busy_wait().await?;
        Ok(())
    }

    async fn busy_wait(&mut self) -> Result<(), Error> {
        info!("The display could be busy, waiting...");
        self.busy_pin
            .wait_for_high()
            .await
            .map_err(Error::from_digital_pin_error)?;
        info!("The display is free, continue...");
        Ok(())
    }

    async fn refresh_display(&mut self) -> Result<(), Error> {
        self.power_on().await?;
        self.busy_wait().await?;
        //self.spi_write_command_and_data(CommandCode::BTST2, &[0x6F, 0x1F, 0x17, 0x49])?;
        self.display_refresh().await?;
        self.busy_wait().await?;
        self.power_off().await?;
        self.busy_wait().await?;
        Ok(())
    }

    async fn send_frame_buffer(&mut self) -> Result<(), Error> {
        let mut buf = [0u8; SPI_CHUNK_SIZE];
        let len = self.frame_buffer.len();
        let mut index = 0;

        self.spi_write_command(CommandCode::DTM1).await?;
        while index < len {
            let chunk_size = (len - index).min(SPI_CHUNK_SIZE);
            (&mut buf[0..chunk_size]).copy_from_slice(
                &self.frame_buffer.as_underlying_data().as_ref()[index..index + chunk_size],
            );
            self.spi_write_data(&buf[0..chunk_size]).await?;
            self.busy_wait().await?;
            index += chunk_size;
        }
        let result: [u8; 1] = self.spi_write_command_and_read(CommandCode::DSP).await?;
        info!("Frame buffer sent: {}, index: {}", result, index);
        self.busy_wait().await?;
        Ok(())
    }

    fn pixel_index(&self, x: usize, y: usize) -> usize {
        y * self.width as usize + x
    }
}

impl<
    DC: OutputPin + Send,
    RST: OutputPin + Send,
    BUSY: Wait + Send,
    SPI: SpiDevice + Send,
    DELAY: DelayNs + Send,
    S: AsMut<[u8]> + AsRef<[u8]>,
> AsyncPartialUpdate<E6Color> for AsyncE6Display<DC, RST, BUSY, SPI, DELAY, S>
{
    async fn partial_update(
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
                        .set(self.pixel_index(x as usize, y as usize), color);
                }
            }
        }
        Ok(())
    }
}

impl<
    DC: OutputPin + Send,
    RST: OutputPin + Send,
    BUSY: Wait + Send,
    SPI: SpiDevice + Send,
    DELAY: DelayNs + Send,
    S: AsMut<[u8]> + AsRef<[u8]>,
> Display<E6Color> for AsyncE6Display<DC, RST, BUSY, SPI, DELAY, S>
{
    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }
}

impl<
    DC: OutputPin + Send,
    RST: OutputPin + Send,
    BUSY: Wait + Send,
    SPI: SpiDevice + Send,
    DELAY: DelayNs + Send,
    S: AsMut<[u8]> + AsRef<[u8]>,
> AsyncDisplay<E6Color> for AsyncE6Display<DC, RST, BUSY, SPI, DELAY, S>
{
    async fn initialize(&mut self) -> Result<(), Error> {
        info!("Initialize display");
        self.reset().await?;
        for (command_code, data) in INIT_SEQUENCE {
            self.spi_write_command_and_data(*command_code, data).await?;
        }
        Ok(())
    }

    async fn update(&mut self, iter: impl IntoIterator<Item = E6Color>) -> Result<(), Error> {
        let mut iter = iter.into_iter();
        for index in 0..self.frame_buffer.len() {
            if let Some(color) = iter.next() {
                self.frame_buffer.set(index, color);
            }
        }
        Ok(())
    }

    async fn refresh(&mut self) -> Result<(), Error> {
        self.send_frame_buffer().await?;
        self.refresh_display().await
    }
}

impl<
    DC: OutputPin + Send,
    RST: OutputPin + Send,
    BUSY: Wait + Send,
    SPI: SpiDevice + Send,
    DELAY: DelayNs + Send,
    S: AsMut<[u8]> + AsRef<[u8]>,
> OriginDimensions for AsyncE6Display<DC, RST, BUSY, SPI, DELAY, S>
{
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl<
    DC: OutputPin + Send,
    RST: OutputPin + Send,
    BUSY: Wait + Send,
    SPI: SpiDevice + Send,
    DELAY: DelayNs + Send,
    S: AsMut<[u8]> + AsRef<[u8]>,
> DrawTarget for AsyncE6Display<DC, RST, BUSY, SPI, DELAY, S>
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
