//! Commands that mean the same thing whatever modem is selected: frequency,
//! the front end, the FIFOs, interrupts, and entering Rx or Tx.
//!
//! The chip reinterprets the modulation and packet parameter blocks according
//! to the current packet type, so those live in the per-modem modules; nothing
//! here depends on what `set_packet_type` last selected.

use crate::cmd;
use crate::hal::{Hal, ERR_BUFFER};
use crate::xfer::{read_cmd, read_fifo, write_cmd};

/// Sets the carrier frequency. The tuning step is 1 Hz, so no scaling.
pub fn set_rf_frequency(hal: &Hal, hz: u32) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_RF_FREQUENCY, &hz.to_be_bytes())
}

/// Selects the modem.
///
/// Must be issued before the modulation and packet parameters, because the
/// chip decodes those according to whatever type is current. The wrong order
/// configures something plausible and reports no error.
pub fn set_packet_type(hal: &Hal, packet_type: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_PACKET_TYPE, &[packet_type])
}

pub fn get_packet_type(hal: &Hal) -> Result<u8, i32> {
    let mut buf = [0u8; 1];
    read_cmd(hal, cmd::CMD_GET_PACKET_TYPE, &[], &mut buf)?;
    Ok(buf[0])
}

/// Chooses the low- or high-band receive chain, and its LNA boost.
pub fn set_rx_path(hal: &Hal, path: u8, boost: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_RX_PATH, &[path & 0x01, boost & 0x07])
}

pub fn sel_pa(hal: &Hal, pa: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SEL_PA, &[pa])
}

/// Raw PA configuration. Prefer [`set_power_dbm`], which fills this from the
/// optimal-setting tables.
pub fn set_pa_config(
    hal: &Hal,
    pa: u8,
    lf_mode: u8,
    lf_duty_cycle: u8,
    lf_slices: u8,
    hf_duty_cycle: u8,
) -> Result<(), i32> {
    let params = [
        (pa << 7) | (lf_mode & 0x03),
        ((lf_duty_cycle & 0x0F) << 4) | (lf_slices & 0x0F),
        hf_duty_cycle & 0x1F,
    ];
    write_cmd(hal, cmd::CMD_SET_PA_CONFIG, &params)
}

pub fn set_tx_params(hal: &Hal, power: i8, ramp: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_TX_PARAMS, &[power as u8, ramp])
}

/// One row of a PA optimal-setting table.
#[derive(Clone, Copy)]
struct PaEntry {
    duty_cycle: u8,
    slices: u8,
    val: i8,
}

const fn pa(duty_cycle: u8, slices: u8, val: i8) -> PaEntry {
    PaEntry {
        duty_cycle,
        slices,
        val,
    }
}

/// Sub-GHz PA settings for -9..=+22 dBm, from RadioLib's `paOptTableLf`.
static PA_LF: [PaEntry; 32] = [
    pa(1, 1, 8),
    pa(2, 2, 1),
    pa(2, 2, 3),
    pa(2, 2, 5),
    pa(1, 2, 13),
    pa(2, 1, 13),
    pa(2, 2, 11),
    pa(2, 2, 13),
    pa(3, 1, 12),
    pa(1, 1, 18),
    pa(1, 1, 20),
    pa(1, 1, 23),
    pa(1, 1, 27),
    pa(1, 1, 33),
    pa(1, 2, 26),
    pa(1, 2, 31),
    pa(1, 3, 27),
    pa(1, 1, 37),
    pa(1, 2, 40),
    pa(2, 1, 38),
    pa(2, 2, 39),
    pa(2, 4, 40),
    pa(2, 7, 41),
    pa(3, 2, 39),
    pa(3, 3, 39),
    pa(3, 6, 38),
    pa(4, 3, 37),
    pa(4, 5, 37),
    pa(4, 7, 38),
    pa(5, 3, 37),
    pa(5, 6, 37),
    pa(6, 7, 35),
];

/// Lowest and highest power the sub-GHz PA table covers, in dBm.
pub const LF_POWER_MIN: i8 = -9;
pub const LF_POWER_MAX: i8 = 22;

/// Selects the sub-GHz PA and sets it up for `dbm`.
///
/// Only the low band is tabulated here: the 2.4 GHz path is FLRC's, and the
/// bring-up script owns it. Asking for a power the table does not cover is an
/// error rather than a clamp, because quietly transmitting at a different
/// power than requested is a regulatory problem, not a convenience.
pub fn set_power_dbm(hal: &Hal, dbm: i8, ramp: u8) -> Result<(), i32> {
    if !(LF_POWER_MIN..=LF_POWER_MAX).contains(&dbm) {
        return Err(ERR_BUFFER);
    }
    let entry = PA_LF[(dbm - LF_POWER_MIN) as usize];
    sel_pa(hal, cmd::PA_LF)?;
    set_pa_config(
        hal,
        cmd::PA_LF,
        cmd::PA_LF_MODE_FSM,
        entry.duty_cycle,
        entry.slices,
        cmd::PA_HF_DUTY_CYCLE_UNUSED,
    )?;
    set_tx_params(hal, entry.val, ramp)
}

/// Timeouts are 24-bit counts of 1/32768 s.
fn timeout_bytes(timeout: u32) -> Result<[u8; 3], i32> {
    if timeout > cmd::TIMEOUT_MAX {
        return Err(ERR_BUFFER);
    }
    Ok([(timeout >> 16) as u8, (timeout >> 8) as u8, timeout as u8])
}

/// Enters receive. `0` listens once; [`cmd::RX_CONTINUOUS`] stays there.
pub fn set_rx(hal: &Hal, timeout: u32) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_RX, &timeout_bytes(timeout)?)
}

pub fn set_tx(hal: &Hal, timeout: u32) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_TX, &timeout_bytes(timeout)?)
}

/// Enables interrupt sources on one of the two DIO vectors.
///
/// The flags latch in GetStatus whether or not a DIO is wired, which is what
/// the polled receive path relies on.
pub fn set_dio_irq_config(hal: &Hal, dio: u8, irq: u32) -> Result<(), i32> {
    // Written out byte by byte rather than with copy_from_slice: that emits an
    // out-of-line core instantiation the natmod linker cannot resolve.
    let params = [
        dio,
        (irq >> 24) as u8,
        (irq >> 16) as u8,
        (irq >> 8) as u8,
        irq as u8,
    ];
    write_cmd(hal, cmd::CMD_SET_DIO_IRQ_CONFIG, &params)
}

/// Reads the pending interrupt vector and clears it in the same command.
pub fn get_and_clear_irq_status(hal: &Hal) -> Result<u32, i32> {
    let mut buf = [0u8; 4];
    read_cmd(hal, cmd::CMD_GET_AND_CLEAR_IRQ_STATUS, &[], &mut buf)?;
    Ok(u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

pub fn clear_irq(hal: &Hal, irq: u32) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_CLEAR_IRQ, &irq.to_be_bytes())
}

// ─── FIFO ────────────────────────────────────────────────────────────────────
// This part streams payload through a FIFO instead of the LR1121's addressed
// buffer, so there is no offset anywhere below and the order of operations
// matters: clear, fill, transmit.

pub fn write_tx_fifo(hal: &Hal, data: &[u8]) -> Result<(), i32> {
    if data.len() > cmd::MAX_PACKET_LEN {
        return Err(ERR_BUFFER);
    }
    write_cmd(hal, cmd::CMD_WRITE_TX_FIFO, data)
}

/// Drains `out.len()` bytes of the Rx FIFO. See [`read_fifo`] for the framing.
pub fn read_rx_fifo(hal: &Hal, out: &mut [u8]) -> Result<(), i32> {
    if out.len() > cmd::MAX_PACKET_LEN {
        return Err(ERR_BUFFER);
    }
    read_fifo(hal, cmd::CMD_READ_RX_FIFO, out)
}

/// How many bytes are waiting in the Rx FIFO.
pub fn get_rx_fifo_level(hal: &Hal) -> Result<u16, i32> {
    let mut buf = [0u8; 2];
    read_cmd(hal, cmd::CMD_GET_RX_FIFO_LEVEL, &[], &mut buf)?;
    Ok(u16::from_be_bytes([buf[0], buf[1]]))
}

pub fn get_tx_fifo_level(hal: &Hal) -> Result<u16, i32> {
    let mut buf = [0u8; 2];
    read_cmd(hal, cmd::CMD_GET_TX_FIFO_LEVEL, &[], &mut buf)?;
    Ok(u16::from_be_bytes([buf[0], buf[1]]))
}

pub fn clear_rx_fifo(hal: &Hal) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_CLEAR_RX_FIFO, &[])
}

pub fn clear_tx_fifo(hal: &Hal) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_CLEAR_TX_FIFO, &[])
}

/// Instantaneous RSSI as a 9-bit half-dBm count; dBm is `-raw / 2`.
pub fn get_rssi_inst(hal: &Hal) -> Result<u16, i32> {
    let mut buf = [0u8; 2];
    read_cmd(hal, cmd::CMD_GET_RSSI_INST, &[], &mut buf)?;
    Ok(((buf[0] as u16) << 1) | ((buf[1] as u16 >> 7) & 0x01))
}

pub fn reset_rx_stats(hal: &Hal) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_RESET_RX_STATS, &[])
}
