#![no_main]
#![no_std]

use cortex_m_rt::entry;
use rtt_target::{rtt_init_print, rprintln};
use core::fmt::Write;
use heapless::Vec;
use panic_rtt_target as _;

#[cfg(feature = "v1")]
use microbit::{
    hal::twi,
    pac::twi0::frequency::FREQUENCY_A,
};

#[cfg(feature = "v2")]
use microbit::{
    hal::twim,
    pac::twim0::frequency::FREQUENCY_A,
};

#[cfg(feature = "v2")]
use microbit::{
    hal::prelude::*,
    hal::uarte,
    hal::uarte::{Baudrate, Parity},
};

#[cfg(feature = "v2")]
mod serial_setup;
#[cfg(feature = "v2")]
use serial_setup::UartePort;

use lsm303agr::{
    AccelOutputDataRate, Lsm303agr,
    MagOutputDataRate, 
};

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = microbit::Board::take().unwrap();


    #[cfg(feature = "v1")]
    let i2c = { twi::Twi::new(board.TWI0, board.i2c.into(), FREQUENCY_A::K100) };

    #[cfg(feature = "v2")]
    let i2c = { twim::Twim::new(board.TWIM0, board.i2c_internal.into(), FREQUENCY_A::K100) };

    #[cfg(feature = "v2")]
    let mut serial = {
        let serial = uarte::Uarte::new(
            board.UARTE0,
            board.uart.into(),
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        );
        UartePort::new(serial)
    };

    // Code from documentation
    let mut sensor = Lsm303agr::new_with_i2c(i2c);
    sensor.init().unwrap();
    sensor.set_accel_odr(AccelOutputDataRate::Hz50).unwrap();
    sensor.set_mag_odr(MagOutputDataRate::Hz50).unwrap();
    let mut sensor = sensor.into_mag_continuous().ok().unwrap();

    // buffer to store input over serial
    let mut buffer: Vec<u8, 32> = Vec::new();

    loop {
        let byte = nb::block!(serial.read()).unwrap();

        let byte_as_char = byte as char;
        rprintln!("{}", byte);
        rprintln!("{}", byte_as_char);

        // handling "Enter" press over serial

        if byte == 13 {
            rprintln!("Escaped.");
            let mut buffer_as_str = core::str::from_utf8(&buffer);
            match buffer_as_str {
                Ok(val) => {
                    rprintln!("Command: {}", val);
                    match val {
                        "M" => {
                            if sensor.mag_status().unwrap().xyz_new_data {
                                let data = sensor.mag_data().unwrap();
                                rprintln!("Mags: x {} y {} z {}", data.x, data.y, data.z);
                            }
                        },
                        "A" => {
                            if sensor.accel_status().unwrap().xyz_new_data {
                                let data = sensor.accel_data().unwrap();
                                // RTT instead of normal print
                                rprintln!("Acceleration: x {} y {} z {}", data.x, data.y, data.z);
                            }
                        },
                        &_ => {
                            rprintln!("Whoops.");
                        }
                    }
                },
                Err(_) => {
                    rprintln!("Error reading from string.");
                }
            }
            buffer.clear();
        } else {
            let buffer_push_result = buffer.push(byte);
            match buffer_push_result {
                Ok(_) => rprintln!("Successful push."),
                Err(_) => {
                    rprintln!("Failed push.");
                    panic!("Blah blah");
                }
            }
        }

        // if sensor.accel_status().unwrap().xyz_new_data {
        //     let data = sensor.accel_data().unwrap();
        //     // RTT instead of normal print
        //     rprintln!("Acceleration: x {} y {} z {}", data.x, data.y, data.z);
        // }

        // if sensor.mag_status().unwrap().xyz_new_data {
        //     let data = sensor.mag_data().unwrap();
        //     rprintln!("Mags: x {} y {} z {}", data.x, data.y, data.z);
        // }

    }
}