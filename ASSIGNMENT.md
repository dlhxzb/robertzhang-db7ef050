# Introduction

## Problem

You are implementing the firmware for a connected balloon with a built-in modem and temperature sensor. For the purpose of warning users that their purchased balloons are reaching a temperature that will impact the level of inflation, the balloon IoT firmware is expected to report the temperature to the server every five seconds.

## Information and constraints

The PCB is made up of a mobile network connected modem, a temperature sensor and a microcontroller running your firmware.

The connection to the server takes place through a simple TCP protocol described below.

To simplify the simulation of a PCB, the crates in this assignment don't strictly run on Rust's no_std. However, you are expected to make no usage of allocations or the Rust `std` library. You are expected to be only using the Rust `core` library (https://docs.rs/core) and potentially no_std compatible crates. You can use the `println!` macro for debug printing, but no use of Vectors, Strings, Box and the likes. If you are unsure, ask. Heap allocations are hard and to be avoided on embedded, but if you get stuck, you can break this rule.

Please develop your assignment's solution within the `firmware` crate in this folder. You are allowed and encouraged to modularize your solution and/or add more crates as your see fit, as long as no use of the std

**This is the first time this assignment has been used for job applicants. If you find any issues that you think might not be part of the assignment, please contact us.**

## Structure

The `board` crate exposes a `Board` structure, that conveniently configures the drivers implementing the `embedded-hal` (https://docs.rs/embedded-hal) traits for you, making the temperature sensor, AT modem and a timer available to you. 

While the board crate makes use of the standard library to implement the simulation of a microcontroller, you are expected to not make use of it in your `firmware` crate.

# TTCOM9000 AT Modem

An AT protocol compatible modem is present on the PCB that is connected to the microcontroller through a UART serial interface.

## Command Set

The modem's command set is a line-based protocol. Each command is expected to be terminated by a new-line character (a single '\n').

The modem will in turn respond with a new-line terminated "OK" if the command was valid, or "ERROR" in any other cases. Any further responses by the command will follow after that.

## AT

Responds with OK if the modem is ready to receive commands.

## AT+STATUS

Responds with OK, and then the modem status, which can be either:
- "Initialized" => modem has finished booting
- "Registered" => modem has registered with the network
- "Connected" => modem has established a TCP connection

Example:

```
> AT+STATUS
< OK
< Initialized
```

## AT+REGISTER
Will make the modem try to register with the network. This can fail due to bad connectivity and thus needs to be repeated

Example:

```
> AT+REGISTER
< OK
```

## AT+TCPCONNECT "\<host\>",\<port\>

Establishes a TCP connection, and returns OK or ERROR.

Example:

```
> AT+TCPCONNECT "balloons.thetc.fakedomain",64920
< OK
```

## AT+TCPSEND \<length\>

Send a predefined number of bytes over TCP. The modem expects the exact number of bytes to be written to the serial device after the new-line of this command.

Example:

```
> AT+TCPSEND 4
> 1234 // binary data
< OK
```

The raw bytes to be sent are not followed by a new-line.

## AT+TCPRECV \<length\>

Attempts to receive a predefined number of bytes from the active TCP socket, returns the number of received bytes, which can be less than the requested number of bytes if less are available in the tcp socket.

Example:

```
> AT+TCPRECV 4
< OK
< 3
< 123 // binary data
```

The number of received bytes is followed by a new-line, and then the specified number of raw bytes.

# Metrics Server

Present at `balloons.thetc.fakedomain` port `64920`. A simple byte-based TCP network protocol. This endpoint does not actually exist. For the sake of this assignment, the protocol is simulated locally.

## Protocol

Each message always starts with a header of one length byte, specifying the length of the message, minus the length byte, and a single byte packet ID. After the header, the body of the message starts. For example

```
0x05 // 5 bytes length
0x10 // packet ID 16
0x01 // 4 bytes of body data
0x02
0x03
0x04
```

On connection, the device needs to identify itself to the server with its device ID. The message has the **packet ID 16** (0x10) followed by four bytes device ID. The server will respond with the **packet ID 17** (0x11) and a single byte indicating success (0x01) or failure (0x00).

After identification, the device can report metrics, including the temperature, to the server by sending messages with **packet ID 18** (0x12) followed by a 32 bit big-endian floating point number. The server will not respond.

If the server encounters any error, it will close the TCP connection.

# THETC2000 Temperature Sensor Datasheet

There is a temperature sensor on the board connected through an I2C bus. It responds to an **address of `0x19`**.

The sensor needs to be calibrated before use, otherwise it reports invalid temperature data.

## Register Description

The sensor has two registers, which can be written to by writing the register address, and then the value, to the bus, or read from, by writing only the register address to the bus.

### Calibrate Register

The calibrate command register is at **address `0x11`** and it is calibrated by setting the third bit to 1.

### Temperature Measurement Register

The temperature measurement register is at **address `0x81`** and upon reading returns two big-endian bytes which represent the temperature with two points after decimal. I.e. to convert it to a proper floating point number, its value need to be divided by 100.
