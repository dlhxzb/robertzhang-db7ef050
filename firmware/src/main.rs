use board::Board;
use core::fmt::Write as StdWrite;
use embedded_hal::{
    blocking::i2c::WriteRead,
    prelude::_embedded_hal_blocking_delay_DelayMs,
    serial::{Read, Write},
};

const MODEM_REPLY_LEN: usize = 20;
const REPORT_INTERVAL: u32 = 5000; // ms

const I2C_TEMPERATURE_ADDRESS: u8 = 0x19;

fn main() {
    let mut board = Board::new();

    loop {
        // Write your firmware here
        let _ = connect_server(&mut board).and_then(|_| report_metrics(&mut board));
    }
}

fn connect_server(board: &mut Board) -> Result<(), ()> {
    const PACKET_IDENTIFY: u8 = 0x10;

    write_modem(board, "AT")
        .and_then(|_| check_modem_reply(board, "OK"))
        .and_then(|_| write_modem(board, "AT+STATUS"))
        .and_then(|_| check_modem_reply(board, "OK"))
        .and_then(|_| {
            read_modem(board, None).map(|reply| {
                println!(
                    "modem state:{:?}",
                    core::str::from_utf8(&&reply.bytes[..reply.len])
                )
            })
        })
        .and_then(|_| write_modem(board, "AT+REGISTER"))
        .and_then(|_| check_modem_reply(board, "OK"))
        .and_then(|_| write_modem(board, "AT+TCPCONNECT balloons.thetc.fakedomain,64920"))
        .and_then(|_| check_modem_reply(board, "OK"))
        .and_then(|_| write_modem(board, "AT+TCPSEND 6"))
        .and_then(|_| write_modem_bytes(board, &[0x05, PACKET_IDENTIFY, 0, 0, 0, 0x07])) // identify, 0x05=len-1, `0, 0, 0, 0x07`=device ID
        .and_then(|_| check_modem_reply(board, "OK"))
        .and_then(|_| write_modem(board, "AT+TCPRECV 3"))
        .and_then(|_| check_modem_reply(board, "OK"))
        .and_then(|_| read_modem(board, None)) // len of identify response(3), ignored
        .and_then(|_| read_modem(board, Some(3))) // [0x02, 0x11, 0x01]
        .and_then(|res| {
            // success (0x01), failure (0x00)
            if res.bytes.get(2) == Some(&0x01) {
                Ok(())
            } else {
                Err(())
            }
        })
}

fn report_metrics(board: &mut Board) -> Result<(), ()> {
    while calibrate(board).is_err() {}

    loop {
        // TCP error return, Serial error retry
        if let Ok(temp) = get_metrics(board) {
            send_metrics(board, temp)?;
            board.timer.delay_ms(REPORT_INTERVAL);
        }
    }
}

fn calibrate(board: &mut Board) -> Result<(), ()> {
    const I2C_REGISTER_CALIBRATE: u8 = 0x11;
    const I2C_COMMAND_CALIBRATE: u8 = 0b0010_0000;

    board
        .i2c_bus
        .write_read(
            I2C_TEMPERATURE_ADDRESS,
            &[I2C_REGISTER_CALIBRATE, I2C_COMMAND_CALIBRATE],
            &mut [],
        )
        .map_err(|e| {
            println!("Calibrate failed {:?}", e);
        })
}

fn get_metrics(board: &mut Board) -> Result<[u8; 2], ()> {
    const I2C_REGISTER_MEASUREMENT: u8 = 0x81;

    let mut temp = [0; 2];
    board
        .i2c_bus
        .write_read(
            I2C_TEMPERATURE_ADDRESS,
            &[I2C_REGISTER_MEASUREMENT],
            &mut temp,
        )
        .map_err(|e| {
            println!("get metrics failed {:?}", e);
        })?;
    Ok(temp)
}

fn send_metrics(board: &mut Board, temp: [u8; 2]) -> Result<(), ()> {
    const PACKET_METRIC: u8 = 0x12;

    let temp = u16::from_be_bytes(temp) as f32 / 100.0;
    let mut output = [0x05, PACKET_METRIC, 0, 0, 0, 0];
    output[2..].copy_from_slice(&temp.to_be_bytes());
    write_modem(board, "AT+TCPSEND 6")
        .and_then(|_| write_modem_bytes(board, &output))
        .and_then(|_| check_modem_reply(board, "OK"))
}

struct ModemReply {
    bytes: [u8; MODEM_REPLY_LEN],
    len: usize,
}

fn write_modem(board: &mut Board, cmd: &str) -> Result<(), ()> {
    let at = &mut board.at_modem as &mut dyn Write<u8, Error = _>;
    writeln!(at, "{cmd}").map_err(|e| {
        println!("Write modem failed:{:?}", e);
    })
}

fn write_modem_bytes(board: &mut Board, bytes: &[u8]) -> Result<(), ()> {
    for byte in bytes {
        board.at_modem.write(*byte).map_err(|_| {
            println!("Write modem bytes failed");
        })?;
    }
    Ok(())
}

// If no specific len, read to `\n`, but not include `\n`
fn read_modem(board: &mut Board, len: Option<usize>) -> Result<ModemReply, ()> {
    let mut reply = ModemReply {
        bytes: [0; MODEM_REPLY_LEN],
        len: 0,
    };
    let len = len.unwrap_or(MODEM_REPLY_LEN);
    for i in 0..len {
        let c = nb::block!(board.at_modem.read()).map_err(|_| {
            println!("read modem timeout");
        })?;
        if c == b'\n' {
            break;
        }
        reply.bytes[i] = c;
        reply.len += 1;
    }
    Ok(reply)
}

fn check_modem_reply(board: &mut Board, expect: &str) -> Result<(), ()> {
    let reply = read_modem(board, None)?;
    if &reply.bytes[..reply.len] == expect.as_bytes() {
        Ok(())
    } else {
        println!(
            "expect={expect}, modem reply={:?}",
            core::str::from_utf8(&&reply.bytes[..reply.len])
        );
        Err(())
    }
}
