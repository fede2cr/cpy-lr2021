//! LR2021 command opcodes and the enum values this driver uses.
//!
//! Values are from RadioLib's `LR2021_commands.h` (mirrored under
//! `reference/lr2021/`). The opcode space has the same shape as the LR1121's —
//! 16-bit, big-endian, `0x01xx` for system and `0x02xx` for radio — and
//! `GET_STATUS`/`GET_VERSION` even share their values with that part, but most
//! others differ, which is why this table exists separately.

// ─── FIFO ────────────────────────────────────────────────────────────────────
// The LR2021 moves payload through a FIFO rather than the LR1121's buffer with
// an explicit offset. This is what makes back-to-back transmission possible.
pub const CMD_READ_RX_FIFO: u16 = 0x0001;
pub const CMD_WRITE_TX_FIFO: u16 = 0x0002;
pub const CMD_GET_RX_FIFO_LEVEL: u16 = 0x011C;
pub const CMD_GET_TX_FIFO_LEVEL: u16 = 0x011D;
pub const CMD_CLEAR_RX_FIFO: u16 = 0x011E;
pub const CMD_CLEAR_TX_FIFO: u16 = 0x011F;

/// Longest LoRa payload the explicit header can describe.
pub const MAX_PACKET_LEN: usize = 255;

// ─── System ──────────────────────────────────────────────────────────────────
pub const CMD_GET_STATUS: u16 = 0x0100;
pub const CMD_GET_VERSION: u16 = 0x0101;
pub const CMD_GET_ERRORS: u16 = 0x0110;
pub const CMD_CLEAR_ERRORS: u16 = 0x0111;
pub const CMD_SET_DIO_FUNCTION: u16 = 0x0112;
pub const CMD_SET_DIO_RF_SWITCH_CONFIG: u16 = 0x0113;
pub const CMD_SET_DIO_IRQ_CONFIG: u16 = 0x0115;
pub const CMD_CLEAR_IRQ: u16 = 0x0116;
pub const CMD_GET_AND_CLEAR_IRQ_STATUS: u16 = 0x0117;
pub const CMD_SET_TCXO_MODE: u16 = 0x0120;
pub const CMD_SET_REG_MODE: u16 = 0x0121;
pub const CMD_CALIBRATE: u16 = 0x0122;
pub const CMD_GET_TEMP: u16 = 0x0125;
pub const CMD_GET_RANDOM_NUMBER: u16 = 0x0126;
pub const CMD_SET_SLEEP: u16 = 0x0127;
pub const CMD_SET_STANDBY: u16 = 0x0128;
pub const CMD_SET_FS: u16 = 0x0129;
/// Loads the patch RAM. The LR1121 has no equivalent bring-up step.
pub const CMD_ACTIVATE_PRAM: u16 = 0x012D;

// ─── Radio ───────────────────────────────────────────────────────────────────
pub const CMD_SET_RF_FREQUENCY: u16 = 0x0200;
pub const CMD_SET_RX_PATH: u16 = 0x0201;
pub const CMD_SET_PA_CONFIG: u16 = 0x0202;
pub const CMD_SET_TX_PARAMS: u16 = 0x0203;
pub const CMD_SET_PACKET_TYPE: u16 = 0x0207;
pub const CMD_GET_PACKET_TYPE: u16 = 0x0208;
pub const CMD_RESET_RX_STATS: u16 = 0x020A;
pub const CMD_SET_RX: u16 = 0x020C;
pub const CMD_SET_TX: u16 = 0x020D;
pub const CMD_SEL_PA: u16 = 0x020F;
pub const CMD_GET_RSSI_INST: u16 = 0x020B;

// ─── LoRa ────────────────────────────────────────────────────────────────────
pub const CMD_SET_LORA_MODULATION_PARAMS: u16 = 0x0220;
pub const CMD_SET_LORA_PACKET_PARAMS: u16 = 0x0221;
pub const CMD_SET_LORA_SYNCH_TIMEOUT: u16 = 0x0222;
pub const CMD_SET_LORA_SYNCWORD: u16 = 0x0223;
pub const CMD_SET_LORA_CAD_PARAMS: u16 = 0x0227;
pub const CMD_SET_LORA_CAD: u16 = 0x0228;
pub const CMD_GET_LORA_RX_STATS: u16 = 0x0229;
pub const CMD_GET_LORA_PACKET_STATUS: u16 = 0x022A;

/// Packet type argument to [`CMD_SET_PACKET_TYPE`].
pub const PACKET_TYPE_LORA: u8 = 0x00;

pub const LORA_SF_MIN: u8 = 5;
pub const LORA_SF_MAX: u8 = 12;

/// Bandwidth codes. **Not monotonic in bandwidth**: 31.25 kHz is 0x02 but
/// 41.67 kHz is 0x0A, so ordering these by code sorts them wrongly.
pub const LORA_BW_31: u8 = 0x02;
pub const LORA_BW_41: u8 = 0x0A;
pub const LORA_BW_62: u8 = 0x03;
pub const LORA_BW_83: u8 = 0x0B;
pub const LORA_BW_101: u8 = 0x0C;
pub const LORA_BW_125: u8 = 0x04;
pub const LORA_BW_203: u8 = 0x0D;
pub const LORA_BW_250: u8 = 0x05;
pub const LORA_BW_406: u8 = 0x0E;
pub const LORA_BW_500: u8 = 0x06;
pub const LORA_BW_812: u8 = 0x0F;
pub const LORA_BW_1000: u8 = 0x07;

/// Bandwidth in Hz for a code, or `None` if the code is not one.
///
/// The fractional widths are the SX128x-derived 2.4 GHz set; using one of them
/// sub-GHz gives a radio that configures cleanly and hears nothing, which is
/// why [`bw_needs_high_band`] exists alongside this.
pub fn lora_bw_hz(bw: u8) -> Option<u32> {
    Some(match bw {
        LORA_BW_31 => 31_250,
        LORA_BW_41 => 41_670,
        LORA_BW_62 => 62_500,
        LORA_BW_83 => 83_340,
        LORA_BW_101 => 101_000,
        LORA_BW_125 => 125_000,
        LORA_BW_203 => 203_000,
        LORA_BW_250 => 250_000,
        LORA_BW_406 => 406_000,
        LORA_BW_500 => 500_000,
        LORA_BW_812 => 812_000,
        LORA_BW_1000 => 1_000_000,
        _ => return None,
    })
}

/// Whether a bandwidth code is only available on the 2.4 GHz path.
pub fn bw_needs_high_band(bw: u8) -> bool {
    matches!(
        bw,
        LORA_BW_41 | LORA_BW_83 | LORA_BW_101 | LORA_BW_203 | LORA_BW_406 | LORA_BW_812
    )
}

/// Coding rate codes. 0 is not one: the numbering starts at 4/5 = 1.
pub const LORA_CR_4_5: u8 = 0x01;
pub const LORA_CR_4_6: u8 = 0x02;
pub const LORA_CR_4_7: u8 = 0x03;
pub const LORA_CR_4_8: u8 = 0x04;
pub const LORA_CR_4_5_LI: u8 = 0x05;
pub const LORA_CR_4_6_LI: u8 = 0x06;
pub const LORA_CR_4_8_LI: u8 = 0x07;
pub const LORA_CR_4_6_LI_CONV: u8 = 0x08;
pub const LORA_CR_4_8_LI_CONV: u8 = 0x09;
pub const LORA_CR_MAX: u8 = LORA_CR_4_8_LI_CONV;

pub const LORA_LDRO_OFF: u8 = 0x00;
pub const LORA_LDRO_ON: u8 = 0x01;

pub const LORA_HEADER_EXPLICIT: u8 = 0x00;
pub const LORA_HEADER_IMPLICIT: u8 = 0x01;

pub const LORA_CRC_OFF: u8 = 0x00;
pub const LORA_CRC_ON: u8 = 0x01;

pub const LORA_IQ_STANDARD: u8 = 0x00;
pub const LORA_IQ_INVERTED: u8 = 0x01;

/// Meshtastic uses the private sync word, not the LoRaWAN one.
pub const LORA_SYNC_WORD_PRIVATE: u8 = 0x12;
pub const LORA_SYNC_WORD_LORAWAN: u8 = 0x34;

// ─── Interrupts ──────────────────────────────────────────────────────────────
// Bit 6 is LoRa header-valid *or* sync-word-valid depending on packet type, so
// there is deliberately no combined name for it.
pub const IRQ_PREAMBLE_DETECTED: u32 = 1 << 5;
pub const IRQ_LORA_HEADER_VALID: u32 = 1 << 6;
pub const IRQ_CAD_DETECTED: u32 = 1 << 7;
pub const IRQ_LORA_HDR_CRC_ERROR: u32 = 1 << 9;
pub const IRQ_ERROR: u32 = 1 << 16;
pub const IRQ_CMD_ERROR: u32 = 1 << 17;
pub const IRQ_RX_DONE: u32 = 1 << 18;
pub const IRQ_TX_DONE: u32 = 1 << 19;
pub const IRQ_CAD_DONE: u32 = 1 << 20;
pub const IRQ_TIMEOUT: u32 = 1 << 21;
pub const IRQ_CRC_ERROR: u32 = 1 << 22;
pub const IRQ_LEN_ERROR: u32 = 1 << 23;

/// Everything a receive loop needs to see to know a frame attempt finished.
pub const IRQ_RX_SET: u32 =
    IRQ_RX_DONE | IRQ_CRC_ERROR | IRQ_LEN_ERROR | IRQ_TIMEOUT | IRQ_LORA_HDR_CRC_ERROR;

// ─── Modes, PA and front end ─────────────────────────────────────────────────
pub const STANDBY_RC: u8 = 0x00;
pub const STANDBY_XOSC: u8 = 0x01;

/// Timeouts are 24-bit; this value means "stay until told otherwise".
pub const RX_CONTINUOUS: u32 = 0x00FF_FFFF;
pub const TIMEOUT_MAX: u32 = 0x00FF_FFFF;

pub const RX_PATH_LF: u8 = 0x00;
pub const RX_PATH_HF: u8 = 0x01;

pub const PA_LF: u8 = 0x00;
pub const PA_HF: u8 = 0x01;
pub const PA_LF_MODE_FSM: u8 = 0x00;
pub const PA_LF_DUTY_CYCLE_UNUSED: u8 = 0x06;
pub const PA_LF_SLICES_UNUSED: u8 = 0x07;
pub const PA_HF_DUTY_CYCLE_UNUSED: u8 = 0x10;

/// 48 us ramp, matching what the LR1121 driver settled on.
pub const PA_RAMP_48U: u8 = 0x05;

/// Calibrate every block.
pub const CALIBRATE_ALL: u8 = 0x7F;

/// `SetDioFunction` argument: hand the pin to the RF switch logic.
pub const DIO_FUNCTION_RF_SWITCH: u8 = 0x02 << 4;
pub const DIO_SLEEP_PULL_UP: u8 = 0x02;
pub const DIO_SLEEP_PULL_AUTO: u8 = 0x03;

/// Which modes assert an RF-switch DIO: standby, Rx, Tx, then high-band Rx/Tx.
pub const RF_SW_RX: u8 = (1 << 1) | (1 << 3);
pub const RF_SW_TX: u8 = (1 << 2) | (1 << 4);


// ─── FLRC ────────────────────────────────────────────────────────────────────
pub const CMD_SET_FLRC_MODULATION_PARAMS: u16 = 0x0248;
pub const CMD_SET_FLRC_PACKET_PARAMS: u16 = 0x0249;
pub const CMD_GET_FLRC_RX_STATS: u16 = 0x024A;
pub const CMD_GET_FLRC_PACKET_STATUS: u16 = 0x024B;
pub const CMD_SET_FLRC_SYNCWORD: u16 = 0x024C;

/// Packet type argument to [`CMD_SET_PACKET_TYPE`].
pub const PACKET_TYPE_FLRC: u8 = 0x05;

/// Combined bitrate/bandwidth code, argument 0 of SetFlrcModulationParams.
///
/// Bitrate and bandwidth are one knob on this part, as on the SX128x, so these
/// are the only eight FLRC rates that exist. See [`crate::flrc::RATES`].
pub const FLRC_BR_2600: u8 = 0x00;
pub const FLRC_BR_2080: u8 = 0x01;
pub const FLRC_BR_1300: u8 = 0x02;
pub const FLRC_BR_1040: u8 = 0x03;
pub const FLRC_BR_650: u8 = 0x04;
pub const FLRC_BR_520: u8 = 0x05;
pub const FLRC_BR_325: u8 = 0x06;
pub const FLRC_BR_260: u8 = 0x07;

/// FLRC coding rate. **These are not in increasing order of rate**: 1/2, 3/4,
/// uncoded, 2/3. Mapping them by arithmetic instead of by name gets it wrong.
pub const FLRC_CR_1_2: u8 = 0x00;
pub const FLRC_CR_3_4: u8 = 0x01;
pub const FLRC_CR_1_1: u8 = 0x02;
pub const FLRC_CR_2_3: u8 = 0x03;

/// Pulse shaping, argument 2 of SetFlrcModulationParams. Shared with GFSK,
/// BPSK and OOK, which is why the codes are not contiguous from zero.
pub const SHAPING_NONE: u8 = 0x00;
pub const SHAPING_GAUSS_BT_0_5: u8 = 0x05;
pub const SHAPING_GAUSS_BT_1_0: u8 = 0x07;
pub const SHAPING_RRC_ROLLOFF_0_5: u8 = 0x09;
