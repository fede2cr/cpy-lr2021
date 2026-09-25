//! LoRa modulation and packet parameters.
//!
//! Everything here is only valid while the packet type is LoRa, and the chip
//! range-checks none of it: an impossible spreading factor or a 2.4 GHz-only
//! bandwidth used sub-GHz produces a radio that reports success and then hears
//! nothing, so the checks happen here instead.
//!
//! The parameter layouts are packed, and differently from the LR1121's: this
//! part folds SF and BW into one byte, CR and LDRO into another, and header
//! type, CRC and IQ into three bits of a fourth. They come from RadioLib's
//! `LR2021_cmds_lora.cpp`.

use crate::cmd;
use crate::hal::{Hal, ERR_BUFFER};
use crate::xfer::{read_cmd, write_cmd};

/// Status of the last received LoRa frame, in raw register units.
///
/// The two RSSI fields are 9-bit counts of a half dBm, so dBm is
/// `-(raw as f32) / 2.0`; `snr_pkt` is quarter-dB, so dB is `raw as f32 / 4.0`.
/// The conversions need floats, which the natmod boundary does not carry, so
/// they are left to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketStatus {
    /// Coding rate from the received explicit header.
    pub cr: u8,
    /// Whether the transmitter enabled a payload CRC.
    pub has_crc: bool,
    pub len: u8,
    pub snr_pkt: i8,
    pub rssi_pkt: u16,
    pub rssi_signal_pkt: u16,
    /// Which of the parallel detectors caught the frame, 0-3.
    pub detector: u8,
}

/// Rejects SF/BW/CR combinations the chip cannot encode.
///
/// `high_band` must be true above 1 GHz. It is an argument rather than
/// something derived from a frequency because the driver never infers the
/// band: a wrong guess transmits into an antenna that is not fitted.
pub fn validate(sf: u8, bw: u8, cr: u8, high_band: bool) -> Result<(), i32> {
    if !(cmd::LORA_SF_MIN..=cmd::LORA_SF_MAX).contains(&sf) {
        return Err(ERR_BUFFER);
    }
    if cmd::lora_bw_hz(bw).is_none() {
        return Err(ERR_BUFFER);
    }
    if cmd::bw_needs_high_band(bw) && !high_band {
        return Err(ERR_BUFFER);
    }
    if cr == 0 || cr > cmd::LORA_CR_MAX {
        return Err(ERR_BUFFER);
    }
    Ok(())
}

/// Packs SF, BW, CR and LDRO into the two bytes SetLoRaModulationParams wants.
///
/// Separate from the command so the packing can be pinned by a test without a
/// chip; getting a nibble the wrong way round here is invisible on the wire.
pub fn modulation_params(sf: u8, bw: u8, cr: u8, ldro: u8) -> [u8; 2] {
    [
        ((sf & 0x0F) << 4) | (bw & 0x0F),
        ((cr & 0x0F) << 4) | (ldro & 0x0F),
    ]
}

/// Whether low data rate optimization is required for this SF and bandwidth.
///
/// The chip does not apply it on its own, and without it the long-symbol rates
/// drift far enough within a frame to lose it. The rule is a symbol time at or
/// above 16 ms, which is what the reference driver uses sub-GHz.
pub fn ldro_needed(sf: u8, bw: u8) -> bool {
    match cmd::lora_bw_hz(bw) {
        // Symbol time is 2**sf / bw seconds. Scaled by 1000 throughout to stay
        // in integers, the test is 2**sf * 1000 >= 16 * bw. Both sides stay
        // well inside u32 for SF12 at 1 MHz.
        Some(hz) => (1u32 << sf) * 1000 >= 16 * hz,
        None => false,
    }
}

pub fn set_modulation_params(
    hal: &Hal,
    sf: u8,
    bw: u8,
    cr: u8,
    ldro: u8,
    high_band: bool,
) -> Result<(), i32> {
    validate(sf, bw, cr, high_band)?;
    write_cmd(
        hal,
        cmd::CMD_SET_LORA_MODULATION_PARAMS,
        &modulation_params(sf, bw, cr, ldro),
    )
}

/// Packs the four bytes SetLoRaPacketParams wants.
///
/// Note the order: the payload length comes *before* the flags byte, and the
/// three flags share that one byte. The LR1121 spends six bytes on the same
/// information, so the layouts are not interchangeable.
pub fn packet_params(
    preamble_len: u16,
    header_type: u8,
    payload_len: u8,
    crc: u8,
    invert_iq: u8,
) -> [u8; 4] {
    [
        (preamble_len >> 8) as u8,
        preamble_len as u8,
        payload_len,
        ((header_type & 0x01) << 2) | ((crc & 0x01) << 1) | (invert_iq & 0x01),
    ]
}

/// Preamble length, header mode, payload length, CRC and IQ inversion.
///
/// `payload_len` is the length to transmit. In implicit header mode it is also
/// the exact length to receive, since there is no header to learn it from. In
/// explicit header mode it still matters on receive: it is the largest payload
/// the modem will accept, and a packet announcing more is abandoned at
/// header-decode time, without an `RxDone` and without a CRC error. A receiver
/// left holding a transmit length therefore goes quietly deaf to anything
/// longer.
pub fn set_packet_params(
    hal: &Hal,
    preamble_len: u16,
    header_type: u8,
    payload_len: u8,
    crc: u8,
    invert_iq: u8,
) -> Result<(), i32> {
    let params = packet_params(preamble_len, header_type, payload_len, crc, invert_iq);
    write_cmd(hal, cmd::CMD_SET_LORA_PACKET_PARAMS, &params)
}

pub fn set_sync_word(hal: &Hal, sync_word: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_LORA_SYNCWORD, &[sync_word])
}

/// Aborts reception if no header arrives within this many symbols.
///
/// `mantissa_exponent` selects the alternate encoding of `symbols`; the plain
/// symbol count is what a receive loop wants.
pub fn set_synch_timeout(hal: &Hal, symbols: u8, mantissa_exponent: bool) -> Result<(), i32> {
    write_cmd(
        hal,
        cmd::CMD_SET_LORA_SYNCH_TIMEOUT,
        &[symbols, mantissa_exponent as u8],
    )
}

/// Decodes the six-byte GetLoRaPacketStatus reply.
pub fn decode_packet_status(buf: &[u8; 6]) -> PacketStatus {
    // Each RSSI is nine bits: eight in its own byte and the ninth scattered
    // into the trailing flags byte.
    let rssi_pkt = ((buf[3] as u16) << 1) | ((buf[5] as u16 & 0x02) >> 1);
    let rssi_signal_pkt = ((buf[4] as u16) << 1) | (buf[5] as u16 & 0x01);
    // One-hot across bits 5:2, reported as an index.
    let detector = match (buf[5] >> 2) & 0x0F {
        0x01 => 0,
        0x02 => 1,
        0x04 => 2,
        0x08 => 3,
        _ => 0,
    };
    PacketStatus {
        cr: buf[0] & 0x0F,
        has_crc: (buf[0] & 0x10) != 0,
        len: buf[1],
        snr_pkt: buf[2] as i8,
        rssi_pkt,
        rssi_signal_pkt,
        detector,
    }
}

pub fn get_packet_status(hal: &Hal) -> Result<PacketStatus, i32> {
    let mut buf = [0u8; 6];
    read_cmd(hal, cmd::CMD_GET_LORA_PACKET_STATUS, &[], &mut buf)?;
    Ok(decode_packet_status(&buf))
}

/// Returns (received, CRC errors, header CRC errors, false syncs).
pub fn get_rx_stats(hal: &Hal) -> Result<(u16, u16, u16, u16), i32> {
    let mut buf = [0u8; 8];
    read_cmd(hal, cmd::CMD_GET_LORA_RX_STATS, &[], &mut buf)?;
    Ok((
        u16::from_be_bytes([buf[0], buf[1]]),
        u16::from_be_bytes([buf[2], buf[3]]),
        u16::from_be_bytes([buf[4], buf[5]]),
        // Big-endian like the rest. RadioLib reads this last field as
        // little-endian; nothing in the command reference suggests it differs,
        // and the false-sync count is diagnostic only, so a byte-swapped value
        // there would be easy to miss.
        u16::from_be_bytes([buf[6], buf[7]]),
    ))
}

/// Channel activity detection thresholds and what to do when it finishes.
pub fn set_cad_params(
    hal: &Hal,
    sym_num: u8,
    preamble_only: bool,
    pnr_delta: u8,
    exit_mode: u8,
    timeout: u32,
    det_peak: u8,
) -> Result<(), i32> {
    if timeout > cmd::TIMEOUT_MAX {
        return Err(ERR_BUFFER);
    }
    let params = [
        sym_num,
        ((preamble_only as u8) << 4) | (pnr_delta & 0x0F),
        exit_mode,
        (timeout >> 16) as u8,
        (timeout >> 8) as u8,
        timeout as u8,
        det_peak & 0x7F,
    ];
    write_cmd(hal, cmd::CMD_SET_LORA_CAD_PARAMS, &params)
}

pub fn set_cad(hal: &Hal) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_LORA_CAD, &[])
}
