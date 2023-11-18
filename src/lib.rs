#![no_std]

use embassy_futures::select::{select, Either};
use embassy_sync::{blocking_mutex::raw::RawMutex, channel};
use embassy_time::{Duration, Timer};
use hal::rmt::{asynch::TxChannelAsync, PulseCode};

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    const OFF: Color = Color { r: 0, g: 0, b: 0 };
}

pub enum Cmd {
    Steady(Color),
    Blinky {
        color0: Color,
        color1: Color,
        period: Duration,
    },
}

pub type CmdChannel<M> = channel::Channel<M, Cmd, 2>;
pub type CmdReceiver<'a, M> = channel::Receiver<'a, M, Cmd, 2>;

// The WS2812 spec sheet has two different durations.
// The shorter duration is 0.40µs. That's 32 cycles at 80MHz.
const SHORT: u16 = 32;
// The shorter duration is 0.85µs. That's 68 cycles at 80MHz.
const LONG: u16 = 68;

// We send a "one" bit by setting the pin high for a long time and low for
// a short time.
const ONE: PulseCode = PulseCode {
    level1: true,
    length1: LONG,
    level2: false,
    length2: SHORT,
};

// We send a "zero" bit by setting the pin high for a short time and low for
// a long time.
const ZERO: PulseCode = PulseCode {
    level1: true,
    length1: SHORT,
    level2: false,
    length2: LONG,
};

// We send a "reset" code by setting the pin low for 50µs. That's 4000 cycles
// at 80MHz.
const RESET: PulseCode = PulseCode {
    level1: false,
    length1: 0,
    level2: false,
    length2: 4000,
};

// Convert a byte into a pulse code. Store the result in the buffer `out`, which
// must be 8 bytes long.
fn write_byte(out: &mut [u32], mut b: u8) {
    let one: u32 = ONE.into();
    let zero: u32 = ZERO.into();

    for sig in out {
        // Highest order bits get sent first.
        let bit = b & 0b1000_0000;
        *sig = if bit != 0 { one } else { zero };
        b <<= 1;
    }
}

pub async fn task<'ch, M: RawMutex>(
    mut rmt_channel: hal::rmt::Channel0<0>,
    receiver: CmdReceiver<'ch, M>,
) {
    hal::interrupt::enable(
        hal::peripherals::Interrupt::RMT,
        hal::interrupt::Priority::Priority10,
    )
    .unwrap();

    let mut cur = Cmd::Steady(Color::OFF);
    let mut zero_to_one = true;
    let mut blink_time = Duration::from_millis(0);
    let refresh_time = Duration::from_millis(50);
    // We need to send 25 pulses: 24 bits and the reset code.
    let mut buf = [0u32; 25];
    loop {
        let (sleep_time, color) = match &cur {
            Cmd::Steady(c) => (Duration::from_secs(3600), *c),
            Cmd::Blinky {
                color0,
                color1,
                period,
            } => {
                // The ratio of time used so far, from zero to 256.
                let ratio = blink_time.as_millis() as u32 * 256 / period.as_millis() as u32;
                let ratio = if zero_to_one { ratio } else { 256 - ratio };
                let lerp = |x, y| ((x as u32 * ratio + y as u32 * (256 - ratio)) / 256) as u8;
                let color = Color {
                    r: lerp(color0.r, color1.r),
                    g: lerp(color0.g, color1.g),
                    b: lerp(color0.b, color1.b),
                };
                blink_time = (blink_time + refresh_time).min(*period);
                if ratio == 256 && zero_to_one {
                    zero_to_one = false;
                    blink_time = Duration::from_millis(0);
                } else if ratio == 0 && !zero_to_one {
                    zero_to_one = true;
                    blink_time = Duration::from_millis(0);
                }
                (refresh_time, color)
            }
        };

        // According to the spec sheet, the order of bytes is GRB.
        write_byte(&mut buf[0..8], color.g);
        write_byte(&mut buf[8..16], color.r);
        write_byte(&mut buf[16..24], color.b);
        buf[24] = RESET.into();

        rmt_channel.transmit(&buf).await.unwrap();

        match select(Timer::after(sleep_time), receiver.receive()).await {
            Either::First(_timeout) => {}
            Either::Second(cmd) => {
                blink_time = Duration::from_millis(0);
                zero_to_one = true;
                cur = cmd;
            }
        }
    }
}
