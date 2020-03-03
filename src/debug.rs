use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL.lock().write_fmt(args).expect("Printing to serial failed");
    });
}

#[macro_export]
macro_rules! dbg_print {
    ($($arg:tt)*) => {
        $crate::debug::_print(format_args!($($arg)*));
    }
}

#[macro_export]
macro_rules! dbg_println {
    () => ($crate::dbg_print!("\n"));
    ($fmt:expr) => ($crate::dbg_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::dbg_print!(
                concat!($fmt, "\n"), $($arg)*));
}
