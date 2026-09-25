//! FLRC: parameter encoding and the arithmetic the speed test needs.
//!
//! Everything here is pure and testable without hardware, which is the point —
//! it is the half of the driver that can be got right before the boards arrive.

use crate::cmd;

/// Continues the shared error numbering in `lrxxxx_hal::hal`, which ends at -8.
pub const ERR_PARAM: i32 = -9;

/// `(bitrate kbps, bandwidth kHz)` indexed by the combined `brBw` code.
///
/// `static`, not `const`: a `const` array is copied to the stack at every use,
/// which costs a `memcpy` helper the natmod linker then has to resolve.
pub static RATES: [(u16, u16); 8] = [
    (2600, 2666),
    (2080, 2222),
    (1300, 1333),
    (1040, 1333),
    (650, 888),
    (520, 769),
    (325, 444),
    (260, 444),
];

/// `(numerator, denominator)` of each coding rate, indexed by its code.
///
/// Deliberately a table: the codes run 1/2, 3/4, uncoded, 2/3, so there is no
/// arithmetic that maps code to ratio.
pub static CODING_RATIOS: [(u8, u8); 4] = [(1, 2), (3, 4), (1, 1), (2, 3)];

/// Channel bitrate in kbps, or `None` if the code is not one of the eight.
pub fn bitrate_kbps(br_bw: u8) -> Option<u16> {
    RATES.get(br_bw as usize).map(|r| r.0)
}

/// Channel bandwidth in kHz, or `None` if the code is not one of the eight.
pub fn bandwidth_khz(br_bw: u8) -> Option<u16> {
    RATES.get(br_bw as usize).map(|r| r.1)
}

/// Coding rate as `(numerator, denominator)`, or `None` for an unknown code.
pub fn coding_ratio(cr: u8) -> Option<(u8, u8)> {
    CODING_RATIOS.get(cr as usize).copied()
}

/// Validates and packs the arguments of SetFlrcModulationParams.
///
/// Two bytes, not three: the coding rate and the pulse shape share the second,
/// as the high and low nibble.
pub fn modulation_params(br_bw: u8, cr: u8, shaping: u8) -> Result<[u8; 2], i32> {
    if br_bw as usize >= RATES.len() || cr as usize >= CODING_RATIOS.len() {
        return Err(ERR_PARAM);
    }
    match shaping {
        cmd::SHAPING_NONE
        | cmd::SHAPING_GAUSS_BT_0_5
        | cmd::SHAPING_GAUSS_BT_1_0
        | cmd::SHAPING_RRC_ROLLOFF_0_5 => {}
        _ => return Err(ERR_PARAM),
    }
    Ok([br_bw, (cr << 4) | (shaping & 0x0F)])
}

/// Payload goodput in bits per second.
///
/// Measured, not predicted: `bytes` is what actually arrived and `elapsed_ms`
/// is what the burst took. Predicting FLRC time-on-air needs the frame overhead
/// from the datasheet, which is not yet in hand.
///
/// 64-bit intermediates are fine here — `LINK_RUNTIME = 1` pulls in libgcc, so
/// the division helper resolves — and are needed: 255 kB at 8000 already
/// overflows 32 bits.
pub fn goodput_bps(bytes: u32, elapsed_ms: u32) -> u32 {
    if elapsed_ms == 0 {
        return 0;
    }
    let bits = (bytes as u64) * 8 * 1000;
    (bits / elapsed_ms as u64) as u32
}

/// Packet error rate in tenths of a percent, saturating at 1000.
///
/// Tenths because the FFI boundary carries integers only; the Python side
/// divides for display.
pub fn per_permille(sent: u32, received: u32) -> u32 {
    if sent == 0 {
        return 0;
    }
    let lost = sent.saturating_sub(received);
    ((lost as u64 * 1000) / sent as u64) as u32
}
