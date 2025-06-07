#![no_std]
#![no_main]
extern crate alloc;

use core::iter;
use defmt::*;
use defmt_rtt as _;
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::spi::{Config, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Timer};
use embedded_alloc::LlffHeap as Heap;
use epd_e6_driver::e6_display::E6Color;
use epd_e6_driver::prelude::*;
use panic_probe as _;
use rp235x_hal::{self as hal};

/// Tell the Boot ROM about our application
#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

#[global_allocator]
static HEAP: Heap = Heap::empty();

const WIDTH: u16 = 800;
const HEIGHT: u16 = 480;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Starting async program");

    // Initialize the allocator BEFORE you use it
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 1024 * 256;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
    }

    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_9, Level::Low);

    let cs = Output::new(p.PIN_13, Level::High);
    let dc = Output::new(p.PIN_6, Level::High);
    let rst = Output::new(p.PIN_7, Level::High);
    let busy = Input::new(p.PIN_8, Pull::Up);
    let spi = Spi::new(
        p.SPI1,
        p.PIN_10,
        p.PIN_11,
        p.PIN_12,
        p.DMA_CH0,
        p.DMA_CH1,
        Config::default(),
    );
    let spi: Mutex<CriticalSectionRawMutex, _> = Mutex::new(spi);
    let spi_device = SpiDevice::new(&spi, cs);

    let mut display = AsyncE6Display::new(WIDTH, HEIGHT, spi_device, dc, rst, busy, Delay);

    display.initialize().await.unwrap();
    let line_width = display.width() / 6;
    for (index, color) in [
        E6Color::Black,
        E6Color::Blue,
        E6Color::Yellow,
        E6Color::White,
        E6Color::Red,
        E6Color::Green,
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
            .await
            .unwrap();
        info!("Fill color: {}", color);
    }
    display.refresh().await.unwrap();

    loop {
        info!("led on!");
        led.set_high();
        Timer::after_secs(1).await;

        info!("led off!");
        led.set_low();
        Timer::after_secs(1).await;
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
