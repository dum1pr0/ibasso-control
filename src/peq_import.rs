//! Reads parametric EQ text in the Equalizer APO format, which squig.link and AutoEQ export:
//!
//! ```text
//! Preamp: -6.0 dB
//! Filter 1: ON PK Fc 26 Hz Gain 6.0 dB Q 0.500
//! ```
use crate::ibasso::PeqBand;

/// Limits of the device's PEQ, matching the editor.
pub const FREQUENCY: (u16, u16) = (20, 20000);
pub const GAIN: (f64, f64) = (-12.0, 12.0);
pub const Q: (f64, f64) = (0.1, 10.0);
pub const TOTAL_GAIN: (i32, i32) = (-24, 24);

#[derive(Debug, PartialEq)]
pub struct Imported {
    pub bands: [PeqBand; 10],
    pub total_gain: i8,
    /// Values that had to be changed to fit the device, in words for the user.
    pub notes: Vec<String>,
}

/// A band that does nothing, used for filters marked OFF and for unused bands.
fn off() -> PeqBand {
    PeqBand { filter_type: 0, frequency: 1000, gain: 0.0, q: 1.0 }
}

/// Filter type codes, as indexes into the device's list (0 off, 1 low shelf, 2 peak, 3 high shelf,
/// 4 low pass, 5 high pass, 6 band pass, 7 all pass).
fn filter_type(code: &str) -> Option<u8> {
    Some(match code.to_ascii_uppercase().as_str() {
        "PK" | "PEQ" | "PEAK" => 2,
        "LS" | "LSC" | "LSQ" | "LOWSHELF" => 1,
        "HS" | "HSC" | "HSQ" | "HIGHSHELF" => 3,
        "LP" | "LPQ" | "LOWPASS" => 4,
        "HP" | "HPQ" | "HIGHPASS" => 5,
        "BP" | "BANDPASS" => 6,
        "AP" | "ALLPASS" => 7,
        _ => return None,
    })
}

/// Reads the number that follows `key` among the words of a line, such as `Fc 26` or `Gain -3.5`.
fn number_after(words: &[&str], key: &str) -> Option<f64> {
    let index = words.iter().position(|w| w.eq_ignore_ascii_case(key))?;
    words.get(index + 1)?.replace(',', ".").parse().ok()
}

fn clamp(value: f64, (min, max): (f64, f64), what: &str, band: usize, notes: &mut Vec<String>) -> f64 {
    let clamped = value.clamp(min, max);
    if clamped != value {
        notes.push(format!("Dải {band}: {what} {value} ngoài giới hạn, đã đổi thành {clamped}"));
    }
    clamped
}

pub fn parse(text: &str) -> Result<Imported, String> {
    let mut bands = [off(); 10];
    let mut used = 0;
    let mut preamp = None;
    let mut notes = Vec::new();
    for (number, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let line_error = |why: &str| format!("Dòng {}: {why}: \"{}\"", number + 1, raw.trim());
        let (head, rest) = line.split_once(':').ok_or_else(|| line_error("không đọc được"))?;
        let head = head.trim().to_ascii_lowercase();
        let words: Vec<&str> = rest.split_whitespace().collect();
        if head == "preamp" {
            let value = words.first().and_then(|w| w.replace(',', ".").parse::<f64>().ok());
            preamp = Some(value.ok_or_else(|| line_error("thiếu giá trị preamp"))?);
            continue;
        }
        if !head.starts_with("filter") {
            // Other Equalizer APO commands (Channel, Device, ...) have no meaning for the DAC.
            continue;
        }
        let enabled = match words.first().map(|w| w.to_ascii_uppercase()) {
            Some(w) if w == "ON" => true,
            Some(w) if w == "OFF" => false,
            _ => return Err(line_error("thiếu ON/OFF")),
        };
        if !enabled {
            continue;
        }
        if used == bands.len() {
            return Err(format!("Có hơn {} bộ lọc đang bật, DAC chỉ có {} dải.", bands.len(), bands.len()));
        }
        let code = words.get(1).ok_or_else(|| line_error("thiếu loại bộ lọc"))?;
        let filter_type = filter_type(code).ok_or_else(|| line_error(&format!("loại bộ lọc {code} không hỗ trợ")))?;
        let band = used + 1;
        let frequency = number_after(&words, "Fc").ok_or_else(|| line_error("thiếu tần số Fc"))?;
        let frequency = clamp(frequency.round(), (FREQUENCY.0 as f64, FREQUENCY.1 as f64), "tần số", band, &mut notes);
        let gain = number_after(&words, "Gain").unwrap_or(0.0);
        let gain = clamp(gain, GAIN, "gain", band, &mut notes);
        // Shelves exported without a Q use the usual 0.707.
        let q = number_after(&words, "Q").unwrap_or(std::f64::consts::FRAC_1_SQRT_2);
        let q = clamp(q, Q, "Q", band, &mut notes);
        bands[used] = PeqBand { filter_type, frequency: frequency as u16, gain, q };
        used += 1;
    }
    if used == 0 {
        return Err("Không tìm thấy bộ lọc nào đang bật (dạng \"Filter 1: ON PK Fc 100 Hz Gain -3 dB Q 1.0\").".into());
    }
    // The device takes whole decibels. Rounding down never adds gain, so it cannot cause clipping.
    let preamp = preamp.unwrap_or(0.0);
    let total_gain = (preamp.floor() as i32).clamp(TOTAL_GAIN.0, TOTAL_GAIN.1);
    if total_gain as f64 != preamp {
        notes.push(format!("Preamp {preamp} dB được làm tròn thành {total_gain} dB"));
    }
    Ok(Imported { bands, total_gain: total_gain as i8, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUIGLINK: &str = "Preamp: -6.0 dB
Filter 1: ON PK Fc 26 Hz Gain 6.0 dB Q 0.500
Filter 2: ON PK Fc 71 Hz Gain 0.5 dB Q 2.000
Filter 3: ON PK Fc 180 Hz Gain -3.5 dB Q 1.000
Filter 4: ON PK Fc 690 Hz Gain 3.5 dB Q 0.900
Filter 5: ON PK Fc 3500 Hz Gain 2.2 dB Q 2.000
Filter 6: ON PK Fc 4300 Hz Gain -8.9 dB Q 2.000
Filter 7: ON PK Fc 6000 Hz Gain 4.6 dB Q 2.000
Filter 8: ON PK Fc 9600 Hz Gain -8.6 dB Q 1.500
Filter 9: ON PK Fc 15000 Hz Gain 5.0 dB Q 0.500
Filter 10: OFF PK Fc 0 Hz Gain 0.0 dB Q 0.000
";

    #[test]
    fn reads_a_squiglink_export() {
        let eq = parse(SQUIGLINK).unwrap();
        assert_eq!(eq.total_gain, -6);
        assert_eq!(eq.bands[0], PeqBand { filter_type: 2, frequency: 26, gain: 6.0, q: 0.5 });
        assert_eq!(eq.bands[5], PeqBand { filter_type: 2, frequency: 4300, gain: -8.9, q: 2.0 });
        assert_eq!(eq.bands[8], PeqBand { filter_type: 2, frequency: 15000, gain: 5.0, q: 0.5 });
        // The OFF filter becomes an unused band instead of a 0 Hz one.
        assert_eq!(eq.bands[9].filter_type, 0);
        // Frequencies below 20 Hz would be clamped and noted; 26 Hz is in range.
        assert!(eq.notes.is_empty(), "{:?}", eq.notes);
    }

    #[test]
    fn reads_shelves_and_windows_line_endings() {
        let eq = parse("Preamp: -3.4 dB\r\nFilter 1: ON LSC Fc 105 Hz Gain 5.5 dB\r\nFilter 2: ON HSQ Fc 8000 Hz Gain -2 dB Q 0.7\r\n").unwrap();
        assert_eq!(eq.bands[0].filter_type, 1);
        assert!((eq.bands[0].q - 0.707).abs() < 0.001);
        assert_eq!(eq.bands[1].filter_type, 3);
        assert_eq!(eq.total_gain, -4);
        assert_eq!(eq.notes.len(), 1);
    }

    #[test]
    fn out_of_range_values_are_clamped_and_reported() {
        let eq = parse("Filter 1: ON PK Fc 10 Hz Gain 15 dB Q 0.05").unwrap();
        assert_eq!(eq.bands[0], PeqBand { filter_type: 2, frequency: 20, gain: 12.0, q: 0.1 });
        assert_eq!(eq.notes.len(), 3);
    }

    #[test]
    fn rejects_text_the_device_cannot_hold() {
        assert!(parse("").is_err());
        assert!(parse("Filter 1: ON XYZ Fc 100 Hz Gain 1 dB Q 1").is_err());
        let eleven: String = (1..=11).map(|i| format!("Filter {i}: ON PK Fc 1000 Hz Gain 1 dB Q 1\n")).collect();
        assert!(parse(&eleven).is_err());
    }
}
