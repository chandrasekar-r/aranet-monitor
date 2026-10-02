//! Decoding of Aranet4 BLE advertisement manufacturer data.
//!
//! Layout (company ID 0x0702 already stripped by the BLE stack), little-endian:
//! `[0]` flags, `[1..4]` firmware version, `[4..8]` reserved,
//! `[8..10]` CO₂ ppm, `[10..12]` temperature ×20, `[12..14]` pressure ×10 hPa,
//! `[14]` humidity %, `[15]` battery %, `[16]` status colour,
//! `[17..19]` measurement interval s, `[19..21]` seconds since measurement.

pub const MANUFACTURER_ID: u16 = 0x0702;

const FLAG_INTEGRATIONS: u8 = 1 << 5;
const MIN_LEN: usize = 21;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    /// `None` while the sensor is calibrating or reports an invalid value.
    pub co2: Option<u16>,
    pub temperature: Option<f32>,
    pub pressure: Option<f32>,
    pub humidity: u8,
    pub battery: u8,
    pub interval: u16,
    pub ago: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Advertisement too short to be an Aranet4 packet.
    TooShort,
    /// "Smart Home integrations" is off, so no measurements are broadcast.
    IntegrationsOff,
}

fn u16_at(d: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([d[i], d[i + 1]])
}

pub fn decode(data: &[u8]) -> Result<Reading, DecodeError> {
    if data.len() < 5 {
        return Err(DecodeError::TooShort);
    }
    if data[0] & FLAG_INTEGRATIONS == 0 {
        return Err(DecodeError::IntegrationsOff);
    }
    if data.len() < MIN_LEN {
        return Err(DecodeError::TooShort);
    }

    let co2 = u16_at(data, 8);
    let temp = u16_at(data, 10);
    let pressure = u16_at(data, 12);
    let humidity = data[14];

    Ok(Reading {
        co2: (co2 >> 15 == 0).then_some(co2),
        temperature: (temp >> 14 & 1 == 0).then_some(temp as f32 * 0.05),
        pressure: (pressure >> 15 == 0).then_some(pressure as f32 * 0.1),
        humidity,
        battery: data[15],
        interval: u16_at(data, 17),
        ago: u16_at(data, 19),
    })
}

/// "Aranet4 0874F" -> "0874F".
pub fn short_name(name: &str) -> &str {
    name.strip_prefix("Aranet4 ").unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    // Captured from the real "Aranet4 0874F" on 2026-09-23.
    const SAMPLE_0874F: &str = "21040201000900014b029601bc272c5c0158020e0128";

    #[test]
    fn decodes_real_packet() {
        let r = decode(&hex(SAMPLE_0874F)).unwrap();
        assert_eq!(r.co2, Some(587));
        assert!((r.temperature.unwrap() - 20.3).abs() < 0.01);
        assert!((r.pressure.unwrap() - 1017.2).abs() < 0.01);
        assert_eq!(r.humidity, 44);
        assert_eq!(r.battery, 92);
        assert_eq!(r.interval, 600);
        assert_eq!(r.ago, 270);
    }

    #[test]
    fn decodes_other_real_packet() {
        let r = decode(&hex("21040201000900010d038e01bb27325c015802380264")).unwrap();
        assert_eq!(r.co2, Some(781));
        assert_eq!(r.humidity, 50);
    }

    #[test]
    fn integrations_off_is_reported() {
        let mut d = hex(SAMPLE_0874F);
        d[0] &= !FLAG_INTEGRATIONS;
        assert_eq!(decode(&d), Err(DecodeError::IntegrationsOff));
        assert_eq!(decode(&d[..7]), Err(DecodeError::IntegrationsOff));
    }

    #[test]
    fn short_packets_are_rejected() {
        assert_eq!(decode(&[0x21, 0, 0]), Err(DecodeError::TooShort));
        assert_eq!(decode(&hex(SAMPLE_0874F)[..15]), Err(DecodeError::TooShort));
    }

    #[test]
    fn calibration_magic_values_become_none() {
        let mut d = hex(SAMPLE_0874F);
        d[9] |= 0x80; // CO₂ high bit
        d[11] |= 0x40; // temperature bit 14
        let r = decode(&d).unwrap();
        assert_eq!(r.co2, None);
        assert_eq!(r.temperature, None);
    }

    #[test]
    fn short_name_strips_prefix() {
        assert_eq!(short_name("Aranet4 0874F"), "0874F");
        assert_eq!(short_name("Other"), "Other");
    }
}
