#![no_std]
#![no_main]

use embedded_hal::i2c::{
    Error as Eh1Error,
    ErrorKind,
    ErrorType,
    I2c as Eh1I2c,
    Operation,
    SevenBitAddress,
};
use cortex_m_rt::entry;
use defmt::println;
use defmt_rtt as _;
use panic_probe as _;
use stm32f3xx_hal::{
    delay::Delay, 
    gpio::{
        OpenDrain, 
        Output, 
        gpioc::{PC0, PC11}
    }, 
    hal::blocking::i2c::{
        Read as HalRead,
        Write as HalWrite,
        WriteRead as HalWriteRead,
    }, 
    i2c::I2c,
    pac::{self, DWT}, 
    prelude::*, 
    serial::Serial, 
    time::rate::Extensions,
};
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use core::fmt::Write;
use heapless::String;
use core::ptr::write_volatile;

type OneWirePin = PC0<Output<OpenDrain>>;
type GpioCRegs = stm32f3xx_hal::pac::gpioc::RegisterBlock;
type RelayPin = PC11<Output<OpenDrain>>;

const DWT_CYCCNT: *mut u32 = 0xE0001004 as *mut u32;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CompatI2cError;

impl Eh1Error for CompatI2cError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

// Wrapper for Hal-i2c
pub struct I2cCompat<T>(pub T);

impl<T> ErrorType for I2cCompat<T> {
    type Error = CompatI2cError;
}

impl<T> Eh1I2c for I2cCompat<T>
where
    T: HalRead + HalWrite + HalWriteRead,
{
    fn transaction(
        &mut self,
        address: SevenBitAddress,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        for op in operations {
            match op {
                Operation::Read(buf) => {
                    self.0.read(address, buf).map_err(|_| CompatI2cError)?;
                }
                Operation::Write(buf) => {
                    self.0.write(address, buf).map_err(|_| CompatI2cError)?;
                }
            }
        }
        Ok(())
    }
}

// Delay with uart reading 
fn effective_delay<S> (
    delay: &mut Delay,
    uart: &mut S,
    pid: &mut Pid,
    buffer: &mut String<32>,
    ms: u16,
)
where 
    S: stm32f3xx_hal::hal::serial::Read<u8>
{
    for _ in 0..ms {
        delay.delay_ms(1_u16);
        user_input_read(uart, buffer, pid);
    }
}

// Reading the line`s state by idr register
fn read_line(gpioc: &GpioCRegs) -> bool {
    gpioc.idr.read().idr0().bit_is_set()
}

// Onewire Bus init. Returns true if the sensor was found
fn ow_reset(pin: &mut OneWirePin, gpioc: &GpioCRegs, delay: &mut Delay) -> bool {
    pin.set_low().ok();
    delay.delay_us(480_u32);

    pin.set_high().ok();
    delay.delay_us(70_u32);

    let present = !read_line(gpioc); // LOW = sensor found

    delay.delay_us(410_u32);
    present
}

// Write one bit
fn ow_write_bit(pin: &mut OneWirePin, delay: &mut Delay, bit: bool) {
    pin.set_low().ok();
    if bit {
        delay.delay_us(5_u32);
        pin.set_high().ok();
        delay.delay_us(55_u32);
    } else {
        delay.delay_us(60_u32);
        pin.set_high().ok();
        delay.delay_us(5_u32);
    }
}

// Read one bit
fn ow_read_bit(pin: &mut OneWirePin, gpioc: &GpioCRegs, delay: &mut Delay) -> bool {
    pin.set_low().ok();
    delay.delay_us(3_u32);

    pin.set_high().ok();
    delay.delay_us(10_u32);

    let bit = read_line(gpioc);

    delay.delay_us(47_u32);
    bit
}

// Write one byte
fn ow_write_byte(pin: &mut OneWirePin, delay: &mut Delay, byte: u8) {
    for i in 0..8 {
        ow_write_bit(pin, delay, (byte >> i) & 1 == 1);
    }
}

// Read one byte
fn ow_read_byte(pin: &mut OneWirePin, gpioc: &GpioCRegs, delay: &mut Delay) -> u8 {
    let mut byte = 0u8;
    for i in 0..8 {
        if ow_read_bit(pin, gpioc, delay) {
            byte |= 1 << i;
        }
    }
    byte
}

// Address matching
fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for byte in data {
        let mut byte = *byte;
        for _ in 0..8 {
            let mix = (crc ^ byte) & 0x01;
            crc >>= 1;
            if mix != 0 {
                crc ^= 0x8C;
            }
            byte >>= 1;
        }
    }
    crc
}

// Measure the temperature
fn read_temperature<S>(
    pin: &mut OneWirePin, 
    gpioc: &GpioCRegs, 
    delay: &mut Delay, 
    uart: &mut S, 
    pid: &mut Pid, 
    buffer: &mut String<32>
) -> Option<f32> 
where S: stm32f3xx_hal::hal::serial::Read<u8>
{
    if !ow_reset(pin, gpioc, delay) { return None; }
    ow_write_byte(pin, delay, 0xCC);
    ow_write_byte(pin, delay, 0x44);
    effective_delay(delay, uart, pid, buffer, 750);

    if !ow_reset(pin, gpioc, delay) { return None; }
    ow_write_byte(pin, delay, 0xCC);
    ow_write_byte(pin, delay, 0xBE);

    // Read the scratchpad from ds18b20
    let mut scratchpad = [0u8; 9];
    for byte in scratchpad.iter_mut() {
        *byte = ow_read_byte(pin, gpioc, delay);
    }

    // Crc check
    if crc8(&scratchpad[..8]) != scratchpad[8] {
        println!("error: CRC mismatch");
        return None;
    }

    // First 2 bytes from scratchpad are raw temperature
    // so we read them as one 16-bit number and then 
    // convert raw result to real temperature.
    let raw = ((scratchpad[1] as i16) << 8) | (scratchpad[0] as i16);
    Some(raw as f32 * 0.0625)
}

// PID regulator
struct Pid {
    kp: f32,
    ki: f32,
    kd: f32,
    setpoint: f32,
    previous_error: f32,
    integral: f32,
}

impl Pid {
    fn new(kp: f32, ki: f32, kd: f32, setpoint: f32) -> Self{
        Self {
            kp,
            ki,
            kd,
            setpoint,
            previous_error: 0.0,
            integral: 0.0,
        }
    }
    
    // Calculates the power for heater
    fn pid_output (&mut self, current_temperature: f32, dt: f32) -> f32 {
        let temperature_error = self.setpoint - current_temperature;
        
        // Matching bang-bang pattern
        let bang_bang_threshold = match self.setpoint {
            sp if sp > 65.0 => 4.0,
            sp if sp > 50.0 => 5.0,
            sp if sp > 35.0 => 7.0,
            _ => 999.0, // unreal
        };

        // Bang-bang
        if temperature_error > bang_bang_threshold {
            return 100.0;
        }
        // Integral component saves the error
        self.integral += (temperature_error * dt).clamp(-50.0, 50.0);
        // Derivative component calculation
        let derivative = (temperature_error - self.previous_error) / dt;
        // Pid-output
        let output = (
            self.kp * temperature_error + 
            self.ki * self.integral + 
            self.kd * derivative
        ).clamp(0.0, 100.0);
        // Remember the error
        self.previous_error = temperature_error;

        return output;
    }
}

// User interface
fn user_input_parse(pid: &mut Pid, buffer: &mut String<32>) {
    // Split the string on 2 parts by White Space
    let mut parts = buffer.trim().splitn(2, ' ');
    let command = parts.next().unwrap_or("");
    let variable = parts.next().unwrap_or("");

    match command {
        // Commands with args
        "kp" | "ki" | "kd" | "sp" => {
            if let Ok(value) = variable.parse::<f32>() {
                match command {
                    "kp" => { pid.kp = value; println!("kp is set to {}", value); }
                    "ki" => { pid.ki = value; println!("ki is set to {}", value); }
                    "kd" => { pid.kd = value; println!("kd is set to {}", value); }
                    "sp" => {
                        if (20.0..=80.0).contains(&value) {
                            pid.setpoint = value;
                            pid.previous_error = 0.0;
                            pid.integral = 0.0;
                            println!("setpoint is set to {}", value);
                        } else {
                            println!("error: setpoint {} is out of bounds (20..80)", value);
                        }
                    }
                    _ => unreachable!(),
                }
            } else {
                println!("error: invalid number format");
            }
        }
        // Commands without args
        "stats" => {
            println!("kp: {}, ki: {}, kd: {}, integral: {}", pid.kp, pid.ki, pid.kd, pid.integral);
        }
        "help" => {
            println!("---------------");
            println!("HELP");
            println!("*type one from the commands below and after white space type the value*");
            println!("Commands: kp, ki, kd, sp, stats");
            println!("---------------");
        }
        _ if !command.is_empty() => {
            println!("unknown command '{}'! Type 'help'", command);
        }
        _ => {}
    }
}

// Read the user input string from puTTY terminal to manage coefficients
fn user_input_read<S> (
    uart: &mut S,
    buffer: &mut String<32>,
    pid: &mut Pid,
) 
where 
    S: stm32f3xx_hal::hal::serial::Read<u8>
{
    match uart.read() {
        Ok(byte) => {
            let ch = byte as char;
            if ch == '\n' || ch == '\r'{
                user_input_parse(pid, buffer);
                buffer.clear();
            } else {
                if buffer.push(ch).is_err() {
                    buffer.clear();
                }
            }
        }
        Err(_) => {}
    }
}

// The main functions for managing the heater power with time-proportional PWM.
fn pwm<S> (
    pin: &mut RelayPin,
    delay: &mut Delay,
    pid_output: f32,
    period_ms: u16,
    pid: &mut Pid,
    uart: &mut S,
    buffer: &mut String<32>,
)
where 
    S: stm32f3xx_hal::hal::serial::Read<u8>
{
    // It is no point in switching relays too fast,
    // bc the heater can't heat up/cool down that quickly 
    let effective_output = if pid_output < 5.0 {
        0.0
    } else if pid_output > 95.0 {
        100.0
    } else {
        pid_output
    };
    // Time 
    let on_time = ((effective_output / 100.0) * period_ms as f32) as u16;
    let off_time = period_ms - on_time;

    // On
    if on_time > 0 {
        pin.set_low().ok();
        println!("heater ON {} ms",on_time);
        effective_delay(delay, uart, pid, buffer, on_time);
    }
    // Off
    if off_time > 0 {
        pin.set_high().ok();
        println!("heater OFF {} ms", off_time);
        effective_delay(delay, uart, pid, buffer, off_time);
    }
}

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let mut rcc = dp.RCC.constrain();
    let mut flash = dp.FLASH.constrain();
    let clocks = rcc.cfgr
        .use_hse(8.MHz())
        .sysclk(72.MHz())
        .pclk1(36.MHz())
        .pclk2(72.MHz())
        .freeze(&mut flash.acr);
    let mut delay = Delay::new(cp.SYST, clocks);

    // Ds18b20 pin configuration
    let gpioc_pac = unsafe { &*pac::GPIOC::ptr() };
    let mut gpioc = dp.GPIOC.split(&mut rcc.ahb);
    let mut ds18b20_pin = gpioc
        .pc0
        .into_open_drain_output(&mut gpioc.moder, &mut gpioc.otyper);
    ds18b20_pin.internal_pull_up(&mut gpioc.pupdr, true);

    // Ssd1306 display pins configuration
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb);
    let mut scl = gpiob
        .pb6
        .into_af_open_drain(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    scl.internal_pull_up(&mut gpiob.pupdr, true);

    let mut sda = gpiob
        .pb7
        .into_af_open_drain(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    sda.internal_pull_up(&mut gpiob.pupdr, true);

    // I2c configuration for ssd1306 display
    let i2c = I2c::new(
        dp.I2C1, 
        (scl,sda),
        400_000.Hz(), 
        clocks, 
        &mut rcc.apb1,
    );

    // Ssd1306 display configuration 
    let interface = I2CDisplayInterface::new(I2cCompat(i2c));
    let mut display = Ssd1306::new(
        interface, 
        DisplaySize128x64, 
        DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();
    display.init().unwrap();

    // Display style 
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    // Relay pin configuration
    let mut relay_pin = gpioc
        .pc11
        .into_open_drain_output(&mut gpioc.moder, &mut gpioc.otyper);
    relay_pin.set_high().ok();


    // Uart pins configuration
    let tx = gpioc
        .pc4
        .into_af_push_pull(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrl);

    let rx = gpioc
        .pc5
        .into_af_push_pull(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrl);

    // Uart configuration
    let mut uart1 = Serial::new(
        dp.USART1, 
        (tx, rx), 
        115_200.Bd(), 
        clocks, 
        &mut rcc.apb2,
    );

    // PID configuration
    let mut pid1 = Pid::new(2.0, 0.0, 0.0, 0.0);
    
    // Buffer for UART user input interface
    let mut buffer: String<32> = String::new();
    uart1.write_str(&"to start the temperature controlling, set up the setpoint by the command * sp 'your value' *").unwrap();

    // Cycle counter
    let mut dcb = cp.DCB;
    let mut dwt = cp.DWT;
    dcb.enable_trace();
    dwt.enable_cycle_counter();

    // Reset cycle counter
    unsafe{write_volatile(DWT_CYCCNT, 0);}
    let mut total_seconds: u32 = 0;

    loop {
        display.clear(BinaryColor::Off).unwrap();

        match read_temperature(&mut ds18b20_pin, gpioc_pac, &mut delay, &mut uart1, &mut pid1, &mut buffer) {
            // Sensor found
            Some(temp) => {
                let pid1_output = pid1.pid_output(temp, 1.0);

                // Preparing strings to show
                let mut uart_telemetry_buffer: String<128> = String::new();
                let mut disp_buf: String<32> = String::new();

                // Time
                total_seconds += DWT::cycle_count()/72_000_000;
                unsafe{write_volatile(DWT_CYCCNT, 0);}

                // Show telemetry into UART and debug terminals
                write!(
                    uart_telemetry_buffer, 
                    "Temp: {:.2} °C | Time: {:.2} |Setpoint: {:.2} °C | Output: {:.2} % | Error: {:.2} °C\n\r", 
                    temp, total_seconds, pid1.setpoint, pid1_output, (pid1.setpoint - temp).abs()
                ).unwrap();
                uart1.write_str(&uart_telemetry_buffer).unwrap(); // Show telemetry into UART terminal
                println!("{}", uart_telemetry_buffer.trim()); // Show telemetry into debug terminal

                // Draw telemetry into display buffer 
                disp_buf.clear();
                write!(disp_buf, "Temp: {:.2} C", temp).unwrap();
                Text::new(&disp_buf, Point::new(0, 10), text_style).draw(&mut display).unwrap();

                disp_buf.clear();
                write!(disp_buf, "Setpoint: {:.2} C", pid1.setpoint).unwrap();
                Text::new(&disp_buf, Point::new(0, 26), text_style).draw(&mut display).unwrap();

                disp_buf.clear();
                write!(disp_buf, "Output: {:.2} %", pid1_output).unwrap();
                Text::new(&disp_buf, Point::new(0, 42), text_style).draw(&mut display).unwrap();

                disp_buf.clear();
                write!(disp_buf, "Error: {:.2} C", (pid1.setpoint - temp).abs()).unwrap();
                Text::new(&disp_buf, Point::new(0, 58), text_style).draw(&mut display).unwrap();

                // Start temperature controlling
                pwm(&mut relay_pin, &mut delay, pid1_output, 20000, &mut pid1, &mut uart1, &mut buffer); 
            }
            // Sensor not found
            None => {
                Text::new("Sensor error", Point::new(0, 30), text_style)
                    .draw(&mut display)
                    .unwrap();
                println!("Sensor not found!");
            }
        }

        // Write out data to a display
        display.flush().unwrap();
    }
}
