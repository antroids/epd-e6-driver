#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use core::iter;
use defmt::*;
use defmt_rtt as _;
use embedded_alloc::LlffHeap as Heap;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::MODE_3;
use embedded_hal_bus::spi::ExclusiveDevice;
use epd_e6_driver::prelude::*;
use panic_probe as _;
use rp235x_hal::clocks::init_clocks_and_plls;
use rp235x_hal::fugit::RateExtU32;
use rp235x_hal::gpio::{FunctionSioInput, FunctionSioOutput, FunctionSpi, Pin, PullNone, PullUp};
use rp235x_hal::pac;
use rp235x_hal::spi::Disabled;
use rp235x_hal::spi::FrameFormat::MotorolaSpi;
use rp235x_hal::{self as hal, Spi, Timer, entry};

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
// use some_bsp;

/// Tell the Boot ROM about our application
#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

#[global_allocator]
static HEAP: Heap = Heap::empty();

const WIDTH: u16 = 800;
const HEIGHT: u16 = 480;

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

    let spi: Spi<Disabled, _, _> = Spi::new(pac.SPI1, (spi_miso_pin, spi_mosi_pin, spi_sck_pin));
    let spi = spi.init(&mut pac.RESETS, 1000.kHz(), 1000.kHz(), MotorolaSpi(MODE_3));
    let spi_device = ExclusiveDevice::new(spi, spi_cs_pin, timer).unwrap();

    let mut display = E6Display::new(
        WIDTH,
        HEIGHT,
        spi_device,
        spi_dc_pin,
        spi_reset_pin,
        spi_busy_pin,
        timer,
        Nibbles::new(
            [0u8; underlying_data_len(WIDTH as usize * HEIGHT as usize)],
            WIDTH as usize * HEIGHT as usize,
        ),
    );

    display.initialize().unwrap();

    let line_width = display.width() / 6;
    for (index, color) in [
        E6Color::Black,
        E6Color::White,
        E6Color::Red,
        E6Color::Green,
        E6Color::Yellow,
        E6Color::Blue,
    ]
    .into_iter()
    .enumerate()
    {
        display
            .partial_update(
                iter::repeat_n(color, display.len()),
                index as u16 * line_width..=(index as u16 + 1) * line_width - 1,
                0..=display.height() - 1,
            )
            .unwrap();
        info!("Fill color: {}", color);
    }
    display.refresh().unwrap();

    loop {
        led_pin.set_high().unwrap();
        timer.delay_ms(1000);
        led_pin.set_low().unwrap();
        timer.delay_ms(1000);
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
