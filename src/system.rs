//! Bring-up: reset-time state, patch RAM, calibration, and the DIO wiring that
//! connects the chip to the board's front end.

use crate::cmd;
use crate::hal::Hal;
use crate::xfer::{read_cmd, write_cmd};

/// Firmware identification from GetVersion.
///
/// Two bytes, and no hardware or device field: unlike the LR1121, this part
/// offers nothing in its version reply that identifies the chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub fw_major: u8,
    pub fw_minor: u8,
}

pub fn get_version(hal: &Hal) -> Result<Version, i32> {
    let mut buf = [0u8; 2];
    read_cmd(hal, cmd::CMD_GET_VERSION, &[], &mut buf)?;
    Ok(Version {
        fw_major: buf[0],
        fw_minor: buf[1],
    })
}

/// A word from the hardware entropy source.
pub fn get_random_number(hal: &Hal) -> Result<u32, i32> {
    let mut buf = [0u8; 4];
    read_cmd(hal, cmd::CMD_GET_RANDOM_NUMBER, &[], &mut buf)?;
    Ok(u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

pub fn get_errors(hal: &Hal) -> Result<u16, i32> {
    let mut buf = [0u8; 2];
    read_cmd(hal, cmd::CMD_GET_ERRORS, &[], &mut buf)?;
    Ok(u16::from_be_bytes([buf[0], buf[1]]))
}

pub fn clear_errors(hal: &Hal) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_CLEAR_ERRORS, &[])
}

pub fn set_standby(hal: &Hal, mode: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_STANDBY, &[mode])
}

pub fn set_fs(hal: &Hal) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_FS, &[])
}

pub fn set_sleep(hal: &Hal, config: u8, wakeup_ticks: u32) -> Result<(), i32> {
    let params = [
        config,
        (wakeup_ticks >> 16) as u8,
        (wakeup_ticks >> 8) as u8,
        wakeup_ticks as u8,
    ];
    write_cmd(hal, cmd::CMD_SET_SLEEP, &params)
}

/// Loads the patch RAM.
///
/// Must run before any radio configuration. The LR1121 has no equivalent step,
/// and skipping it here leaves a chip that answers every command and never
/// transmits — which looks like an antenna or PA fault, not a missing patch.
pub fn activate_pram(hal: &Hal) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_ACTIVATE_PRAM, &[0x00])
}

pub fn calibrate(hal: &Hal, blocks: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_CALIBRATE, &[blocks])
}

/// Assigns a DIO to a function such as the RF switch.
pub fn set_dio_function(hal: &Hal, dio: u8, function: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_DIO_FUNCTION, &[dio, function])
}

/// Which chip modes drive a given RF-switch DIO high.
pub fn set_dio_rf_switch_config(hal: &Hal, dio: u8, modes: u8) -> Result<(), i32> {
    write_cmd(hal, cmd::CMD_SET_DIO_RF_SWITCH_CONFIG, &[dio, modes])
}

/// Hands a pair of DIOs to the RF switch logic, one for Tx and one for Rx.
///
/// Until this runs the front end stays shut down and the antenna is
/// disconnected in both directions, with nothing about the failure resembling
/// a switch problem: the radio configures cleanly and hears silence.
///
/// `tx_sleep_pull` is separate because DIO5 rejects anything but a sleep
/// pull-up on this part.
///
/// SCOPE: this is the two-line shape, which is what a simple TX/RX-enable
/// front end such as the W12's 2.4 GHz RFX2402E wants (DIO5 = TXEN,
/// DIO6 = RXEN). It is NOT enough for that board's sub-GHz path, where a
/// GC1109 takes three lines -- CSD on DIO11, CPS on DIO10, CTX on DIO9 -- and
/// the mode-to-level truth table is not in any reference mirrored here. Drive
/// those with [`set_dio_function`] and [`set_dio_rf_switch_config`] directly
/// rather than bending a two-line helper around them.
pub fn set_rf_switch(
    hal: &Hal,
    tx_dio: u8,
    tx_sleep_pull: u8,
    rx_dio: u8,
    rx_sleep_pull: u8,
) -> Result<(), i32> {
    // Written out rather than looped over an array: iterating an array by
    // value instantiates core's IndexRange iterator out of line, and the
    // natmod linker cannot resolve it.
    set_dio_function(hal, tx_dio, cmd::DIO_FUNCTION_RF_SWITCH | tx_sleep_pull)?;
    set_dio_rf_switch_config(hal, tx_dio, cmd::RF_SW_TX)?;
    set_dio_function(hal, rx_dio, cmd::DIO_FUNCTION_RF_SWITCH | rx_sleep_pull)?;
    set_dio_rf_switch_config(hal, rx_dio, cmd::RF_SW_RX)
}
