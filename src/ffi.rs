//! C ABI surface consumed by `natmod/lr2021.c`.
//!
//! Every function takes the callback table and returns an `i32`: 0 or a value
//! on success, one of the negative `hal::ERR_*` codes on failure. Results wider
//! than that come back through out pointers, so neither side allocates.
//!
//! Differs from the LR1121 boundary in one way on purpose: parameters are `i32`
//! rather than `u8`/`u16`, and out pointers are `*mut i32`. Narrow C parameters
//! truncate an out-of-range argument silently, which turns a typo in a Python
//! caller into a radio that is configured wrongly but reports success; taking
//! them wide lets the range check happen here and fail loudly instead. The
//! packet-status block also mixes widths and a signed field, which a byte array
//! cannot carry.

use crate::cmd;
use crate::flrc;
use crate::hal::{Hal, ERR_NULL_HAL, OK};
use crate::lora;
use crate::radio;
use crate::system as ops;
use crate::xfer;

unsafe fn hal_of<'a>(hal: *const Hal) -> Option<&'a Hal> {
    if hal.is_null() {
        None
    } else {
        Some(&*hal)
    }
}

fn status_of(r: Result<(), i32>) -> i32 {
    match r {
        Ok(()) => OK,
        Err(e) => e,
    }
}

/// Returns a non-negative value as itself, or the error code.
fn value_of<T: Into<i32>>(r: Result<T, i32>) -> i32 {
    match r {
        Ok(v) => v.into(),
        Err(e) => e,
    }
}

/// Channel bitrate in kbps for a combined `brBw` code, or `ERR_PARAM`.
#[no_mangle]
pub extern "C" fn lr2021_flrc_bitrate_kbps(br_bw: i32) -> i32 {
    match flrc::bitrate_kbps(br_bw as u8) {
        Some(v) if (0..=0xFF).contains(&br_bw) => v as i32,
        _ => flrc::ERR_PARAM,
    }
}

/// Channel bandwidth in kHz for a combined `brBw` code, or `ERR_PARAM`.
#[no_mangle]
pub extern "C" fn lr2021_flrc_bandwidth_khz(br_bw: i32) -> i32 {
    match flrc::bandwidth_khz(br_bw as u8) {
        Some(v) if (0..=0xFF).contains(&br_bw) => v as i32,
        _ => flrc::ERR_PARAM,
    }
}

/// Validates a modulation pair and packs it into the low two bytes.
///
/// Returned as one integer rather than an out-struct: two bytes is small
/// enough that a struct would cost more at the boundary than it saves.
#[no_mangle]
pub extern "C" fn lr2021_flrc_modulation_params(br_bw: i32, cr: i32, shaping: i32) -> i32 {
    if !(0..=0xFF).contains(&br_bw) || !(0..=0xFF).contains(&cr) || !(0..=0xFF).contains(&shaping) {
        return flrc::ERR_PARAM;
    }
    match flrc::modulation_params(br_bw as u8, cr as u8, shaping as u8) {
        Ok(p) => ((p[0] as i32) << 8) | (p[1] as i32),
        Err(e) => e,
    }
}

/// Measured payload goodput in bits per second.
#[no_mangle]
pub extern "C" fn lr2021_flrc_goodput_bps(bytes: i32, elapsed_ms: i32) -> i32 {
    if bytes < 0 || elapsed_ms < 0 {
        return flrc::ERR_PARAM;
    }
    // Caps at i32::MAX rather than wrapping: 2.1 Gbps is far past anything this
    // part can do, so a value that large means the inputs were wrong.
    flrc::goodput_bps(bytes as u32, elapsed_ms as u32).min(i32::MAX as u32) as i32
}

/// Packet error rate in tenths of a percent.
#[no_mangle]
pub extern "C" fn lr2021_flrc_per_permille(sent: i32, received: i32) -> i32 {
    if sent < 0 || received < 0 {
        return flrc::ERR_PARAM;
    }
    flrc::per_permille(sent as u32, received as u32) as i32
}

/// Resolves the `Hal` pointer once per entry point, or returns `ERR_NULL_HAL`.
macro_rules! with_hal {
    ($hal:expr, |$h:ident| $body:expr) => {
        match unsafe { hal_of($hal) } {
            Some($h) => $body,
            None => ERR_NULL_HAL,
        }
    };
}

// ─── System ──────────────────────────────────────────────────────────────────

/// Waits for BUSY to fall. Callers use it after a hardware reset.
#[no_mangle]
pub unsafe extern "C" fn lr2021_wait_ready(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| status_of(h.wait_busy()))
}

/// Writes stat1, stat2 and the interrupt vector into `out[0..3]`.
///
/// This is the cheapest proof that a chip is present and answering, so the
/// undriven-MISO check lives on this path rather than in the caller.
#[no_mangle]
pub unsafe extern "C" fn lr2021_get_status(hal: *const Hal, out: *mut i32) -> i32 {
    if out.is_null() {
        return ERR_NULL_HAL;
    }
    with_hal!(hal, |h| match xfer::get_status(h) {
        Ok((s1, s2, irq)) => {
            xfer::check_status(s1).map_or_else(
                |e| e,
                |()| {
                    unsafe {
                        *out = s1 as i32;
                        *out.add(1) = s2 as i32;
                        *out.add(2) = irq as i32;
                    }
                    OK
                },
            )
        }
        Err(e) => e,
    })
}

/// Writes firmware major and minor into `out[0..2]`.
#[no_mangle]
pub unsafe extern "C" fn lr2021_get_version(hal: *const Hal, out: *mut i32) -> i32 {
    if out.is_null() {
        return ERR_NULL_HAL;
    }
    with_hal!(hal, |h| match ops::get_version(h) {
        Ok(v) => {
            unsafe {
                *out = v.fw_major as i32;
                *out.add(1) = v.fw_minor as i32;
            }
            OK
        }
        Err(e) => e,
    })
}

/// Writes a 32-bit random word into `*out`, which an i32 return cannot carry.
#[no_mangle]
pub unsafe extern "C" fn lr2021_get_random_number(hal: *const Hal, out: *mut i32) -> i32 {
    if out.is_null() {
        return ERR_NULL_HAL;
    }
    with_hal!(hal, |h| match ops::get_random_number(h) {
        Ok(v) => {
            unsafe {
                *out = v as i32;
            }
            OK
        }
        Err(e) => e,
    })
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_get_errors(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| value_of(ops::get_errors(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_clear_errors(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| status_of(ops::clear_errors(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_standby(hal: *const Hal, mode: i32) -> i32 {
    with_hal!(hal, |h| status_of(ops::set_standby(h, mode as u8)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_fs(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| status_of(ops::set_fs(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_sleep(hal: *const Hal, config: i32, wakeup_ticks: i32) -> i32 {
    with_hal!(hal, |h| status_of(ops::set_sleep(
        h,
        config as u8,
        wakeup_ticks as u32
    )))
}

/// Loads the patch RAM. Required before any radio configuration.
#[no_mangle]
pub unsafe extern "C" fn lr2021_activate_pram(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| status_of(ops::activate_pram(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_calibrate(hal: *const Hal, blocks: i32) -> i32 {
    with_hal!(hal, |h| status_of(ops::calibrate(h, blocks as u8)))
}

/// Assigns a DIO to a function such as the RF switch.
///
/// Exposed alongside [`lr2021_set_rf_switch`] because a three-line front end
/// like the W12's sub-GHz GC1109 does not fit that helper's Tx/Rx pair.
#[no_mangle]
pub unsafe extern "C" fn lr2021_set_dio_function(hal: *const Hal, dio: i32, function: i32) -> i32 {
    with_hal!(hal, |h| status_of(ops::set_dio_function(
        h,
        dio as u8,
        function as u8
    )))
}

/// Which chip modes drive a given RF-switch DIO high.
#[no_mangle]
pub unsafe extern "C" fn lr2021_set_dio_rf_switch_config(
    hal: *const Hal,
    dio: i32,
    modes: i32,
) -> i32 {
    with_hal!(hal, |h| status_of(ops::set_dio_rf_switch_config(
        h,
        dio as u8,
        modes as u8
    )))
}

/// Points the RF switch at a Tx DIO and an Rx DIO, with their sleep pulls.
#[no_mangle]
pub unsafe extern "C" fn lr2021_set_rf_switch(
    hal: *const Hal,
    tx_dio: i32,
    tx_pull: i32,
    rx_dio: i32,
    rx_pull: i32,
) -> i32 {
    with_hal!(hal, |h| status_of(ops::set_rf_switch(
        h,
        tx_dio as u8,
        tx_pull as u8,
        rx_dio as u8,
        rx_pull as u8
    )))
}

// ─── Radio ───────────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_rf_frequency(hal: *const Hal, hz: i32) -> i32 {
    if hz <= 0 {
        return flrc::ERR_PARAM;
    }
    with_hal!(hal, |h| status_of(radio::set_rf_frequency(h, hz as u32)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_packet_type(hal: *const Hal, packet_type: i32) -> i32 {
    with_hal!(hal, |h| status_of(radio::set_packet_type(
        h,
        packet_type as u8
    )))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_get_packet_type(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| value_of(radio::get_packet_type(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_rx_path(hal: *const Hal, path: i32, boost: i32) -> i32 {
    with_hal!(hal, |h| status_of(radio::set_rx_path(
        h, path as u8, boost as u8
    )))
}

/// Selects the sub-GHz PA and configures it for `dbm` from the optimal tables.
#[no_mangle]
pub unsafe extern "C" fn lr2021_set_power_dbm(hal: *const Hal, dbm: i32, ramp: i32) -> i32 {
    if !(-128..=127).contains(&dbm) {
        return flrc::ERR_PARAM;
    }
    with_hal!(hal, |h| status_of(radio::set_power_dbm(
        h, dbm as i8, ramp as u8
    )))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_rx(hal: *const Hal, timeout: i32) -> i32 {
    if timeout < 0 {
        return flrc::ERR_PARAM;
    }
    with_hal!(hal, |h| status_of(radio::set_rx(h, timeout as u32)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_tx(hal: *const Hal, timeout: i32) -> i32 {
    if timeout < 0 {
        return flrc::ERR_PARAM;
    }
    with_hal!(hal, |h| status_of(radio::set_tx(h, timeout as u32)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_set_dio_irq_config(hal: *const Hal, dio: i32, irq: i32) -> i32 {
    with_hal!(hal, |h| status_of(radio::set_dio_irq_config(
        h, dio as u8, irq as u32
    )))
}

/// Reads and clears the interrupt vector.
///
/// The vector is 26 bits wide, so it always fits the non-negative half of the
/// return value and needs no out pointer.
#[no_mangle]
pub unsafe extern "C" fn lr2021_get_and_clear_irq_status(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| match radio::get_and_clear_irq_status(h) {
        Ok(v) => (v & 0x7FFF_FFFF) as i32,
        Err(e) => e,
    })
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_clear_irq(hal: *const Hal, irq: i32) -> i32 {
    with_hal!(hal, |h| status_of(radio::clear_irq(h, irq as u32)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_get_rssi_inst(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| value_of(radio::get_rssi_inst(h)))
}

// ─── FIFO ────────────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn lr2021_write_tx_fifo(hal: *const Hal, data: *const u8, len: i32) -> i32 {
    if data.is_null() || len < 0 {
        return flrc::ERR_PARAM;
    }
    let buf = unsafe { core::slice::from_raw_parts(data, len as usize) };
    with_hal!(hal, |h| status_of(radio::write_tx_fifo(h, buf)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_read_rx_fifo(hal: *const Hal, out: *mut u8, len: i32) -> i32 {
    if out.is_null() || len < 0 {
        return flrc::ERR_PARAM;
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(out, len as usize) };
    with_hal!(hal, |h| status_of(radio::read_rx_fifo(h, buf)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_get_rx_fifo_level(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| value_of(radio::get_rx_fifo_level(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_get_tx_fifo_level(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| value_of(radio::get_tx_fifo_level(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_clear_rx_fifo(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| status_of(radio::clear_rx_fifo(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_clear_tx_fifo(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| status_of(radio::clear_tx_fifo(h)))
}

// ─── LoRa ────────────────────────────────────────────────────────────────────

/// Bandwidth in Hz for a code, or `ERR_PARAM`. Pure; needs no chip.
#[no_mangle]
pub extern "C" fn lr2021_lora_bw_hz(bw: i32) -> i32 {
    if !(0..=0xFF).contains(&bw) {
        return flrc::ERR_PARAM;
    }
    match cmd::lora_bw_hz(bw as u8) {
        Some(hz) => hz as i32,
        None => flrc::ERR_PARAM,
    }
}

/// Whether low data rate optimization is needed. Pure; needs no chip.
#[no_mangle]
pub extern "C" fn lr2021_lora_ldro_needed(sf: i32, bw: i32) -> i32 {
    if !(0..=0xFF).contains(&sf) || !(0..=0xFF).contains(&bw) {
        return flrc::ERR_PARAM;
    }
    lora::ldro_needed(sf as u8, bw as u8) as i32
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_lora_set_modulation_params(
    hal: *const Hal,
    sf: i32,
    bw: i32,
    cr: i32,
    ldro: i32,
    high_band: i32,
) -> i32 {
    if !(0..=0xFF).contains(&sf) || !(0..=0xFF).contains(&bw) || !(0..=0xFF).contains(&cr) {
        return flrc::ERR_PARAM;
    }
    with_hal!(hal, |h| status_of(lora::set_modulation_params(
        h,
        sf as u8,
        bw as u8,
        cr as u8,
        ldro as u8,
        high_band != 0
    )))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_lora_set_packet_params(
    hal: *const Hal,
    preamble_len: i32,
    header_type: i32,
    payload_len: i32,
    crc: i32,
    invert_iq: i32,
) -> i32 {
    if !(0..=0xFFFF).contains(&preamble_len) || !(0..=0xFF).contains(&payload_len) {
        return flrc::ERR_PARAM;
    }
    with_hal!(hal, |h| status_of(lora::set_packet_params(
        h,
        preamble_len as u16,
        header_type as u8,
        payload_len as u8,
        crc as u8,
        invert_iq as u8
    )))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_lora_set_sync_word(hal: *const Hal, sync_word: i32) -> i32 {
    with_hal!(hal, |h| status_of(lora::set_sync_word(h, sync_word as u8)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_lora_set_synch_timeout(hal: *const Hal, symbols: i32, mant_exp: i32) -> i32 {
    with_hal!(hal, |h| status_of(lora::set_synch_timeout(
        h,
        symbols as u8,
        mant_exp != 0
    )))
}

/// Writes cr, has_crc, len, snr, rssi_pkt, rssi_signal and detector into
/// `out[0..7]`.
///
/// The two RSSI fields and the SNR stay in raw register units: dBm is
/// `-rssi / 2` and dB is `snr / 4`, and the boundary carries no floats.
#[no_mangle]
pub unsafe extern "C" fn lr2021_lora_get_packet_status(hal: *const Hal, out: *mut i32) -> i32 {
    if out.is_null() {
        return ERR_NULL_HAL;
    }
    with_hal!(hal, |h| match lora::get_packet_status(h) {
        Ok(s) => {
            unsafe {
                *out = s.cr as i32;
                *out.add(1) = s.has_crc as i32;
                *out.add(2) = s.len as i32;
                *out.add(3) = s.snr_pkt as i32;
                *out.add(4) = s.rssi_pkt as i32;
                *out.add(5) = s.rssi_signal_pkt as i32;
                *out.add(6) = s.detector as i32;
            }
            OK
        }
        Err(e) => e,
    })
}

/// Writes received, CRC errors, header CRC errors and false syncs into
/// `out[0..4]`.
#[no_mangle]
pub unsafe extern "C" fn lr2021_lora_get_rx_stats(hal: *const Hal, out: *mut i32) -> i32 {
    if out.is_null() {
        return ERR_NULL_HAL;
    }
    with_hal!(hal, |h| match lora::get_rx_stats(h) {
        Ok((rx, crc, hdr, fs)) => {
            unsafe {
                *out = rx as i32;
                *out.add(1) = crc as i32;
                *out.add(2) = hdr as i32;
                *out.add(3) = fs as i32;
            }
            OK
        }
        Err(e) => e,
    })
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_reset_rx_stats(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| status_of(radio::reset_rx_stats(h)))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_lora_set_cad_params(
    hal: *const Hal,
    sym_num: i32,
    preamble_only: i32,
    pnr_delta: i32,
    exit_mode: i32,
    timeout: i32,
    det_peak: i32,
) -> i32 {
    if timeout < 0 {
        return flrc::ERR_PARAM;
    }
    with_hal!(hal, |h| status_of(lora::set_cad_params(
        h,
        sym_num as u8,
        preamble_only != 0,
        pnr_delta as u8,
        exit_mode as u8,
        timeout as u32,
        det_peak as u8
    )))
}

#[no_mangle]
pub unsafe extern "C" fn lr2021_lora_set_cad(hal: *const Hal) -> i32 {
    with_hal!(hal, |h| status_of(lora::set_cad(h)))
}

