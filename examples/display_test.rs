#![no_std]
#![no_main]
extern crate alloc;

use defmt::*;
use defmt_rtt as _;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::iso_8859_16::FONT_10X20;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyleBuilder, StyledDrawable, Triangle};
use embedded_graphics::text::{Alignment, Text};
use panic_probe as _;
use rp235x_hal::clocks::init_clocks_and_plls;
use rp235x_hal::fugit::RateExtU32;
use rp235x_hal::gpio::{FunctionSioInput, FunctionSioOutput, FunctionSpi, Pin, PullNone, PullUp};
use rp235x_hal::pac;
use rp235x_hal::{self as hal, Spi, Timer, entry};

mod display;
mod e6_display;

mod nibbles_vec;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
// use some_bsp;

/// Tell the Boot ROM about our application
#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

use crate::display::Display;
use crate::e6_display::E6Color;
use embedded_alloc::LlffHeap as Heap;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::MODE_3;
use embedded_hal_bus::spi::ExclusiveDevice;
use rp235x_hal::spi::Disabled;
use rp235x_hal::spi::FrameFormat::MotorolaSpi;

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[entry]
fn main() -> ! {
    // Initialize the allocator BEFORE you use it
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 1024 * 256;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
    }

    info!("Program start");
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let sio = hal::Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // This is the correct pin on the Raspberry Pico 2 board. On other boards, even if they have an
    // on-board LED, it might need to be changed.
    //
    // Notably, on the Pico 2 W, the LED is not connected to any of the RP2350 GPIOs but to the cyw43 module instead.
    // One way to do that is by using [embassy](https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/wifi_blinky.rs)
    //
    // If you have a Pico W and want to toggle a LED with a simple GPIO output pin, you can connect an external
    // LED to one of the GPIO pins, and reference that pin here. Don't forget adding an appropriate resistor
    // in series with the LED.
    let mut led_pin = pins.gpio9.into_push_pull_output();

    //SPI
    let spi_dc_pin: Pin<_, FunctionSioOutput, PullNone> = pins.gpio6.reconfigure();
    let spi_reset_pin: Pin<_, FunctionSioOutput, PullNone> = pins.gpio7.reconfigure();
    let spi_cs_pin: Pin<_, FunctionSioOutput, PullNone> = pins.gpio13.reconfigure();
    let spi_busy_pin: Pin<_, FunctionSioInput, PullUp> = pins.gpio8.reconfigure();
    let spi_sck_pin: Pin<_, FunctionSpi, PullNone> = pins.gpio10.reconfigure();
    let spi_miso_pin: Pin<_, FunctionSpi, PullNone> = pins.gpio11.reconfigure();
    let spi_mosi_pin: Pin<_, FunctionSpi, PullNone> = pins.gpio12.reconfigure();
    let mut timer = Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);

    let mut spi: Spi<Disabled, _, _> =
        Spi::new(pac.SPI1, (spi_miso_pin, spi_mosi_pin, spi_sck_pin));
    spi.set_baudrate(1000.kHz(), 1000.kHz());
    let spi = spi.init(&mut pac.RESETS, 1000.kHz(), 1000.kHz(), MotorolaSpi(MODE_3));
    let spi_device = ExclusiveDevice::new(spi, spi_cs_pin, timer).unwrap();

    //embedded_hal::delay::DelayNs::delay_ms(&mut timer, 3000);

    let mut display = e6_display::E6Display::new(
        800,
        480,
        spi_device,
        spi_dc_pin,
        spi_reset_pin,
        spi_busy_pin,
        timer,
    );

    display.initialize().unwrap();

    let style1 = PrimitiveStyleBuilder::new()
        .stroke_color(E6Color::Green)
        .stroke_width(5)
        .build();
    let style2 = PrimitiveStyleBuilder::new()
        .stroke_color(E6Color::Red)
        .stroke_width(5)
        .build();
    let style3 = PrimitiveStyleBuilder::new()
        .stroke_color(E6Color::Yellow)
        .stroke_width(5)
        .build();

    display.clear(E6Color::White).unwrap();

    Triangle::new(
        Point::new(50, 50),
        Point::new(100, 100),
        Point::new(100, 80),
    )
    .into_styled(style1)
    .draw(&mut display)
    .unwrap();

    Circle::new(Point::new(500, 150), 50)
        .draw_styled(&style2, &mut display)
        .unwrap();

    let character_style = MonoTextStyle::new(&FONT_10X20, E6Color::Black);
    Text::with_alignment(
        "Hello world!",
        display.bounding_box().center() + Point::new(0, 15),
        character_style,
        Alignment::Center,
    )
    .draw(&mut display)
    .unwrap();

    display.refresh().unwrap();

    loop {
        // info!("on!");
        led_pin.set_high().unwrap();
        embedded_hal::delay::DelayNs::delay_ms(&mut timer, 500);
        // info!("off!");
        led_pin.set_low().unwrap();
        embedded_hal::delay::DelayNs::delay_ms(&mut timer, 500);
    }
}

/// Program metadata for `picotool info`
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [rp235x_hal::binary_info::EntryAddr; 5] = [
    rp235x_hal::binary_info::rp_cargo_bin_name!(),
    rp235x_hal::binary_info::rp_cargo_version!(),
    rp235x_hal::binary_info::rp_program_description!(c"RP2350 Template"),
    rp235x_hal::binary_info::rp_cargo_homepage_url!(),
    rp235x_hal::binary_info::rp_program_build_attribute!(),
];

// End of file
