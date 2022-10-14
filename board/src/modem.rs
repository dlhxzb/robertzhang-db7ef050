use embedded_hal::serial::{Read, Write};
use rand::prelude::*;
use std::{collections::VecDeque, convert::TryInto, time::Duration};

const TCP_HOST: &str = "balloons.thetc.fakedomain";
const TCP_PORT: u32 = 64920;
const PACKET_IDENTIFY: u8 = 0x10;
const PACKET_IDENTIFY_RESPONSE: u8 = 0x11;
const PACKET_METRIC: u8 = 0x12;

pub struct AtModem {
    pub(crate) write_buffer: Vec<u8>,
    read_buffer: VecDeque<u8>,
    state: State,
    rng: ThreadRng,

    // Bytes sent by the client
    tcp_write_buffer: VecDeque<u8>,
    // Bytes sent to the client
    tcp_read_buffer: Vec<u8>,
    tcp_server_state: ServerState,
    tcp_server_identified: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum State {
    Initialising,
    Initialized,
    Registered,
    Connected,
    TcpSending(usize),
}

enum ServerState {
    Ready,
    ReadingPacket(usize),
}

impl AtModem {
    pub fn new() -> Self {
        Self {
            write_buffer: Vec::new(),
            read_buffer: VecDeque::new(),
            state: State::Initialising,
            rng: rand::thread_rng(),
            tcp_write_buffer: VecDeque::new(),
            tcp_read_buffer: Vec::new(),
            tcp_server_state: ServerState::Ready,
            tcp_server_identified: false,
        }
    }

    fn check_complete_command(&mut self) {
        loop {
            // The modem is waiting for bytes to send over TCP, read raw bytes
            if let State::TcpSending(bytes_count) = self.state {
                if self.write_buffer.len() < bytes_count {
                    // Not enough bytes available yet
                    break;
                }

                let bytes: Vec<_> = self.write_buffer.drain(..bytes_count).collect();
                self.tcp_write_buffer.extend(bytes);
                self.handle_tcp_recv();
                self.state = State::Connected;
                self.write_ok();
            }

            // Read a line

            let new_line = self.write_buffer.iter().position(|b| *b == b'\n');

            if let Some(index) = new_line {
                let complete_line: Vec<_> = self.write_buffer.drain(..index).collect();
                // Remove newline
                self.write_buffer.remove(0);
                // self.write_buffer.drain
                if let Ok(str) = std::str::from_utf8(&complete_line) {
                    println!("line {}", str);
                    let command = str.to_owned();
                    self.handle_command(command);
                } else {
                    self.write_error();
                }
            } else {
                break;
            }
        }
    }

    fn handle_command(&mut self, cmd: String) {
        // It's a slow modem
        std::thread::sleep(Duration::from_millis(100));

        // The modem needs time to initialize
        if self.state == State::Initialising {
            if self.rng.gen_range(0..100) < 40 {
                self.state = State::Initialized
            }
            self.write_error();
            return;
        }

        let tokens: Vec<_> = cmd.splitn(2, ' ').collect();
        if tokens.is_empty() {
            self.write_error();
            return;
        }

        match tokens[0] {
            "AT" => {
                if self.state != State::Initialising {
                    self.write_ok();
                } else if self.rng.gen_range(0..100) < 40 {
                    self.state = State::Initialising
                }
            }
            "AT+STATUS" => {
                self.write_ok();
                self.write_line(&format!("{:?}", self.state));
            }
            "AT+REGISTER" => {
                // No network
                if self.rng.gen_range(0..100) < 40 {
                    self.write_error();
                } else if self.state == State::Initialized {
                    self.state = State::Registered;
                    self.write_ok();
                } else {
                    // Already registered
                    self.write_error();
                }
            }
            "AT+TCPCONNECT" => {
                if tokens.len() < 2 || self.state != State::Registered {
                    self.write_error();
                    return;
                }

                let target: Vec<_> = tokens[1].splitn(2, ',').map(|s| s.trim()).collect();
                if target.len() < 2 {
                    self.write_error();
                    return;
                }

                let host = target[0].trim_matches(|v| v == '"');
                let port: u32 = match target[1].parse() {
                    Ok(port) => port,
                    _ => {
                        self.write_error();
                        return;
                    }
                };

                // Correct host, port
                if host == TCP_HOST && port == TCP_PORT {
                    self.state = State::Connected;
                    self.write_ok();
                } else {
                    self.write_error();
                }
            }
            "AT+TCPSEND" => {
                if tokens.len() < 2 || self.state != State::Connected {
                    self.write_error();
                    return;
                }

                let bytes_count: usize = match tokens[1].parse() {
                    Ok(count) => count,
                    _ => {
                        self.write_error();
                        return;
                    }
                };

                self.state = State::TcpSending(bytes_count);
            }
            "AT+TCPRECV" => {
                if tokens.len() < 2 || self.state != State::Connected {
                    self.write_error();
                    return;
                }

                let bytes_count: usize = match tokens[1].parse() {
                    Ok(count) => count,
                    _ => {
                        self.write_error();
                        return;
                    }
                };

                let returned_bytes_count = usize::min(bytes_count, self.tcp_read_buffer.len());
                let bytes: Vec<_> = self.tcp_read_buffer.drain(..returned_bytes_count).collect();

                // Write OK, returned bytes count, and bytes returned
                self.write_ok();
                self.write_line(&format!("{}", returned_bytes_count));
                self.read_buffer.extend(bytes);
            }
            _ => self.write_error(),
        }
    }

    fn write_error(&mut self) {
        self.write_line("ERROR");
    }
    fn write_ok(&mut self) {
        self.write_line("OK");
    }

    fn write_line(&mut self, line: &str) {
        self.read_buffer.extend(line.as_bytes());
        self.read_buffer.push_back(b'\n');
    }

    fn handle_tcp_recv(&mut self) {
        loop {
            // Read length
            let length = match self.tcp_server_state {
                ServerState::Ready => match self.tcp_write_buffer.pop_front() {
                    Some(length) => length as usize,
                    _ => break,
                },
                // Waiting for enough bytes to be available
                ServerState::ReadingPacket(length) => length,
            };

            // Not enough bytes for the full packet available yet
            if self.tcp_write_buffer.len() < length {
                self.tcp_server_state = ServerState::ReadingPacket(length);
                break;
            }

            let packet_data: Vec<_> = self.tcp_write_buffer.drain(..length).collect();
            let success = match packet_data[0] {
                PACKET_IDENTIFY => {
                    if packet_data.len() != 5 || self.tcp_server_identified {
                        false
                    } else {
                        println!("[SERVER] Client identified");
                        self.tcp_server_identified = true;
                        // Respond with ok
                        self.tcp_read_buffer
                            .extend(&[0x02, PACKET_IDENTIFY_RESPONSE, 0x01]);
                        true
                    }
                }
                PACKET_METRIC => {
                    if packet_data.len() != 5 || !self.tcp_server_identified {
                        false
                    } else {
                        let temperature = f32::from_be_bytes(packet_data[1..].try_into().unwrap());
                        println!("[SERVER] Temperature received: {}", temperature);
                        true
                    }
                }
                _ => false,
            };

            if !success {
                self.close_tcp()
            }

            self.tcp_server_state = ServerState::Ready;
        }
    }

    fn close_tcp(&mut self) {
        self.state = State::Registered;
        self.tcp_server_state = ServerState::Ready;
        self.tcp_write_buffer.clear();
        self.tcp_read_buffer.clear();
        self.tcp_server_identified = false;
    }
}

pub enum SerialError {
    Timeout,
}

impl Read<u8> for AtModem {
    type Error = SerialError;

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        if let Some(b) = self.read_buffer.pop_front() {
            Ok(b)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

impl Write<u8> for AtModem {
    type Error = SerialError;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        self.write_buffer.push(word);
        self.check_complete_command();
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        Ok(())
    }
}
