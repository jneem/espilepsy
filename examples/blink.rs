#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_assoc_type)]

use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::{Duration, Timer};
use espilepsy::Color;
use hal::{
    gpio::Output,
    interrupt::software::SoftwareInterruptControl,
    rmt::{Rmt, TxChannelConfig, TxChannelCreator},
    time::Rate,
    timer::timg::TimerGroup,
};
use static_cell::StaticCell;

use esp_backtrace as _;

macro_rules! singleton {
    ($val:expr, $typ:ty) => {{
        static STATIC_CELL: StaticCell<$typ> = StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

type BlinkyChannel = Channel<CriticalSectionRawMutex, espilepsy::Cmd, 2>;
type BlinkyReceiver<'a> = Receiver<'a, CriticalSectionRawMutex, espilepsy::Cmd, 2>;
type LedPin<'a> = Output<'a>;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let config = hal::Config::default();
    let peripherals = hal::init(config);

    hal::interrupt::enable(
        hal::peripherals::Interrupt::RMT,
        hal::interrupt::Priority::Priority1,
    )
    .unwrap();

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    let pin = Output::new(
        peripherals.GPIO7,
        hal::gpio::Level::High,
        hal::gpio::OutputConfig::default(),
    );
    let ch = singleton!(Channel::new(), BlinkyChannel);
    spawner.must_spawn(led(peripherals.RMT, pin, ch.receiver()));

    ch.send(espilepsy::Cmd::Blinky {
        color0: Color { r: 20, g: 0, b: 0 },
        color1: Color { r: 0, g: 20, b: 0 },
        period: Duration::from_millis(500),
    })
    .await;
    loop {
        Timer::after_millis(5000).await;
    }
}

#[embassy_executor::task]
async fn led(
    rmt: hal::peripherals::RMT<'static>,
    pin: LedPin<'static>,
    recv: BlinkyReceiver<'static>,
) {
    let rmt = Rmt::new(rmt, Rate::from_mhz(80u32)).unwrap().into_async();
    let channel = rmt
        .channel0
        .configure_tx(pin, TxChannelConfig::default().with_clk_divider(1))
        .unwrap();
    espilepsy::task(channel, recv).await
}
