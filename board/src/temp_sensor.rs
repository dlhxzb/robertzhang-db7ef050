use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use rand::prelude::*;

const I2C_TEMPERATURE_ADDRESS: u8 = 0x19;
const I2C_REGISTER_CALIBRATE: u8 = 0x11;
const I2C_COMMAND_CALIBRATE: u8 = 0b0010_0000;

const I2C_REGISTER_MEASUREMENT: u8 = 0x81;

const TEMPERATURE_VARIANCE: i16 = 100;

/// An I2C bus with a flakey temperature sensor on it
pub struct DemoI2CBus {
    is_calibrated: bool,
    temperature: i16,
    rng: ThreadRng,
}

impl DemoI2CBus {
    pub fn new() -> Self {
        Self {
            is_calibrated: false,
            temperature: 971i16,
            rng: rand::thread_rng(),
        }
    }
}

#[derive(Debug)]
pub enum DemoI2CError {
    AddressNack,
    Transmit,
    Receive,
}

impl WriteRead for DemoI2CBus {
    type Error = DemoI2CError;

    fn write_read(
        &mut self,
        address: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> Result<(), Self::Error> {
        if address != I2C_TEMPERATURE_ADDRESS {
            return Err(DemoI2CError::AddressNack);
        }

        if bytes.is_empty() {
            return Err(DemoI2CError::Receive);
        }

        // Oh no, electronics happened
        match self.rng.gen_range(0..200) {
            // This is too mean
            // 0 => {
            //     buffer.fill_with(|| rng.gen());
            //     return Ok(());
            // }
            1 => {
                return Err(DemoI2CError::AddressNack);
            }
            2 => {
                return Err(DemoI2CError::Transmit);
            }
            3 => {
                return Err(DemoI2CError::Receive);
            }
            _ => (),
        }

        match bytes[0] {
            I2C_REGISTER_CALIBRATE => {
                if bytes.len() >= 2 && bytes[1] & I2C_COMMAND_CALIBRATE != 0 {
                    // calibrate the sensor
                    self.is_calibrated = true
                }
            }
            I2C_REGISTER_MEASUREMENT => {
                if !self.is_calibrated {
                    // Sensor not calibrated
                    if buffer.len() > 1 {
                        buffer[0] = self.rng.gen();
                    }
                    if buffer.len() > 2 {
                        buffer[1] = self.rng.gen();
                    }
                } else {
                    let temp = self.temperature as i16
                        + self
                            .rng
                            .gen_range(-TEMPERATURE_VARIANCE..TEMPERATURE_VARIANCE);
                    let temp_bytes = temp.to_be_bytes();
                    buffer[..2].copy_from_slice(&temp_bytes);
                }
            }
            _ => {
                buffer.fill_with(|| self.rng.gen());
            }
        }

        Ok(())
    }
}

impl Write for DemoI2CBus {
    type Error = DemoI2CError;

    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        let mut empty = [];
        self.write_read(addr, bytes, &mut empty)
    }
}

impl Read for DemoI2CBus {
    type Error = DemoI2CError;

    fn read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        self.write_read(address, &[], buffer)
    }
}
