#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::{Duration, Timer};
use espilepsy::Color;
use hal::{
    clock::{ClockControl, Clocks},
    embassy,
    gpio::{GpioPin, Output, PushPull, IO},
    peripherals::Peripherals,
    prelude::*,
    rmt::{Rmt, TxChannelConfig, TxChannelCreatorAsync},
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
type LedPin = GpioPin<Output<PushPull>, 7>;

#[main]
async fn main(spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = singleton!(
        ClockControl::boot_defaults(system.clock_control).freeze(),
        Clocks
    );
    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    embassy::init(
        &clocks,
        hal::systimer::SystemTimer::new_async(peripherals.SYSTIMER),
    );
    hal::interrupt::enable(
        hal::peripherals::Interrupt::RMT,
        hal::interrupt::Priority::Priority1,
    )
    .unwrap();

    let pin = io.pins.gpio7.into_push_pull_output();
    let ch = singleton!(Channel::new(), BlinkyChannel);
    spawner.must_spawn(led(peripherals.RMT, pin, ch.receiver(), &*clocks));

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
    rmt: hal::peripherals::RMT,
    pin: LedPin,
    recv: BlinkyReceiver<'static>,
    clocks: &'static Clocks<'static>,
) {
    let rmt = Rmt::new_async(rmt, 80u32.MHz(), clocks).unwrap();
    let channel = rmt
        .channel0
        .configure(
            pin,
            TxChannelConfig {
                clk_divider: 1,
                ..TxChannelConfig::default()
            },
        )
        .unwrap();
    espilepsy::task(channel, recv).await
}
