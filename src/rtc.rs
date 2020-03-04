use x86_64::instructions::port::Port;
use x86_64::instructions::interrupts;

const SECOND_REG: u8 = 0x00;
const MINUTE_REG: u8 = 0x02;
const HOUR_REG: u8 = 0x04;
const DAY_REG: u8 = 0x07;
const MONTH_REG: u8 = 0x08;
const YEAR_REG: u8 = 0x09;

fn CMOS_read(index: u8) -> u8 {
    unsafe {
        let mut out = Port::new(0x70);
        out.write(index);
        let mut input = Port::new(0x71);
        input.read()
    }
}

fn CMOS_write(index: u8, data: u8) {
    unsafe {
        let mut out1 = Port::new(0x70);
        out1.write(index);
        let mut out2 = Port::new(0x71);
        out2.write(data);
    }
}

fn CMOS_get_update_in_progress() -> bool {
    (CMOS_read(0x0A) & 0x80) != 0
}

#[derive(Debug)]
pub struct Time {
    second: u8,
    minute: u8,
    hour: u8,
    day: u8,
    month: u8,
    year: u8,
}

impl Time {
    pub fn now() -> Time {
        // Returns UTC now
        interrupts::disable();
        while CMOS_get_update_in_progress() {}
        let mut second = CMOS_read(SECOND_REG);
        let mut minute = CMOS_read(MINUTE_REG);
        let mut hour = CMOS_read(HOUR_REG);
        let mut day = CMOS_read(DAY_REG);
        let mut month = CMOS_read(MONTH_REG);
        let mut year = CMOS_read(YEAR_REG);

        if (!(CMOS_read(0x0b) & 0x04) > 0) {
            second = bcd_to_binary(second);
            minute = bcd_to_binary(minute);
            hour = bcd_to_binary(hour);
            day = bcd_to_binary(day);
            month = bcd_to_binary(month);
            year = bcd_to_binary(year);
        }

        assert!(year >= 20);

        let t = Time {
            second,
            minute,
            hour,
            day,
            month,
            year
        };
        interrupts::enable();
        t
    }

}

fn bcd_to_binary(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}
