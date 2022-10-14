mod modem;
mod temp_sensor;
use std::time::Duration;

use embedded_hal::blocking::delay::*;
use modem::AtModem;
use temp_sensor::DemoI2CBus;

pub struct Board {
    /// A I2C bus on the PCB, implementing the embedded_hal::blocking::i2c traits
    pub i2c_bus: DemoI2CBus,
    /// A AT modem connected through a UART serial interface, implementing the embedded_hal::serial traits
    pub at_modem: AtModem,
    /// A timer for delaying execution that implements the embedded_hal DelayMs trait
    pub timer: Timer,
}

impl Board {
    pub fn new() -> Self {
        Board {
            i2c_bus: DemoI2CBus::new(),
            at_modem: AtModem::new(),
            timer: Timer,
        }
    }
}

pub struct Timer;
impl DelayMs<u32> for Timer {
    fn delay_ms(&mut self, ms: u32) {
        std::thread::sleep(Duration::from_millis(ms as u64))
    }
}
