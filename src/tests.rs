use core::ffi::c_void;

use crate::cmd;
use crate::flrc::{self, ERR_PARAM};
use crate::hal::*;
use crate::lora;
use crate::radio;
use crate::system as ops;
use crate::xfer;

#[test]
fn every_flrc_rate_code_resolves_and_nothing_past_the_table_does() {
    assert_eq!(flrc::bitrate_kbps(cmd::FLRC_BR_2600), Some(2600));
    assert_eq!(flrc::bandwidth_khz(cmd::FLRC_BR_2600), Some(2666));
    assert_eq!(flrc::bitrate_kbps(cmd::FLRC_BR_260), Some(260));
    assert_eq!(flrc::bandwidth_khz(cmd::FLRC_BR_260), Some(444));
    assert_eq!(flrc::bitrate_kbps(8), None);
}

#[test]
fn rates_are_listed_fastest_first() {
    for pair in flrc::RATES.windows(2) {
        assert!(pair[0].0 > pair[1].0, "{:?} then {:?}", pair[0], pair[1]);
    }
}

#[test]
fn coding_rate_codes_are_not_in_rate_order() {
    // Pins the trap: the codes run 1/2, 3/4, uncoded, 2/3, so anything that
    // assumes code order matches rate order is wrong.
    assert_eq!(flrc::coding_ratio(cmd::FLRC_CR_1_2), Some((1, 2)));
    assert_eq!(flrc::coding_ratio(cmd::FLRC_CR_3_4), Some((3, 4)));
    assert_eq!(flrc::coding_ratio(cmd::FLRC_CR_1_1), Some((1, 1)));
    assert_eq!(flrc::coding_ratio(cmd::FLRC_CR_2_3), Some((2, 3)));
    assert_eq!(flrc::coding_ratio(4), None);
}

#[test]
fn modulation_params_reject_codes_outside_the_tables() {
    assert!(flrc::modulation_params(8, cmd::FLRC_CR_2_3, cmd::SHAPING_GAUSS_BT_0_5).is_err());
    assert!(flrc::modulation_params(cmd::FLRC_BR_650, 4, cmd::SHAPING_GAUSS_BT_0_5).is_err());
    assert_eq!(
        flrc::modulation_params(cmd::FLRC_BR_650, cmd::FLRC_CR_2_3, 0x01),
        Err(ERR_PARAM)
    );
}

#[test]
fn modulation_params_pack_the_coding_rate_and_shaping_into_one_byte() {
    assert_eq!(
        flrc::modulation_params(cmd::FLRC_BR_1300, cmd::FLRC_CR_3_4, cmd::SHAPING_GAUSS_BT_1_0),
        Ok([0x02, 0x17])
    );
}

#[test]
fn goodput_of_a_full_rate_burst_is_in_the_right_order_of_magnitude() {
    // 1000 packets of 255 bytes in 1.0 s is a bit over 2 Mbps, which is the
    // ballpark the 2600 kbps rate should land in once framing is subtracted.
    assert_eq!(flrc::goodput_bps(255_000, 1000), 2_040_000);
}

#[test]
fn goodput_does_not_overflow_32_bits_mid_calculation() {
    // 255 kB * 8 * 1000 is 2.04e9, which still fits, but 600 kB does not.
    assert_eq!(flrc::goodput_bps(600_000, 1000), 4_800_000);
    assert_eq!(flrc::goodput_bps(0, 1000), 0);
}

#[test]
fn goodput_of_a_zero_length_burst_is_zero_not_a_division_trap() {
    assert_eq!(flrc::goodput_bps(1234, 0), 0);
}

#[test]
fn per_counts_losses_and_clamps_a_negative_loss_to_zero() {
    assert_eq!(flrc::per_permille(1000, 1000), 0);
    assert_eq!(flrc::per_permille(1000, 900), 100);
    assert_eq!(flrc::per_permille(1000, 0), 1000);
    // More received than sent (a duplicate) must not underflow.
    assert_eq!(flrc::per_permille(1000, 1001), 0);
    assert_eq!(flrc::per_permille(0, 0), 0);
}

#[test]
fn a_silent_miso_is_rejected_rather_than_decoded() {
    assert_eq!(xfer::check_status(0x00), Err(ERR_NO_CHIP));
    assert_eq!(xfer::check_status(0xFF), Err(ERR_NO_CHIP));
    assert_eq!(xfer::check_status(0x42), Ok(()));
}

// ─── Fake chip ───────────────────────────────────────────────────────────────
// The byte order and parameter packing below are the parts that fail silently
// on hardware: a swapped nibble still configures a radio, it just configures a
// different one. Pinning them here means a mistake shows up as a failed test
// rather than as a link that never comes up.

#[derive(Debug, PartialEq, Eq, Clone)]
enum Ev {
    Select(i32),
    Write(Vec<u8>),
    Read(Vec<u8>),
}

#[derive(Default)]
struct Mock {
    log: Vec<Ev>,
    /// One entry per `read` call, in order. Short entries are zero-padded.
    replies: Vec<Vec<u8>>,
    busy_polls: u32,
    ticks: i32,
    tick_step: i32,
}

unsafe extern "C" fn cb_write(ctx: *mut c_void, buf: *const u8, len: usize) -> i32 {
    let m = &mut *(ctx as *mut Mock);
    m.log
        .push(Ev::Write(core::slice::from_raw_parts(buf, len).to_vec()));
    0
}

unsafe extern "C" fn cb_read(ctx: *mut c_void, buf: *mut u8, len: usize) -> i32 {
    let m = &mut *(ctx as *mut Mock);
    let reply = if m.replies.is_empty() {
        Vec::new()
    } else {
        m.replies.remove(0)
    };
    let dst = core::slice::from_raw_parts_mut(buf, len);
    for (i, b) in dst.iter_mut().enumerate() {
        *b = reply.get(i).copied().unwrap_or(0);
    }
    m.log.push(Ev::Read(dst.to_vec()));
    0
}

unsafe extern "C" fn cb_cs(ctx: *mut c_void, level: i32) -> i32 {
    let m = &mut *(ctx as *mut Mock);
    m.log.push(Ev::Select(level));
    0
}

unsafe extern "C" fn cb_busy(ctx: *mut c_void) -> i32 {
    let m = &mut *(ctx as *mut Mock);
    if m.busy_polls > 0 {
        m.busy_polls -= 1;
        1
    } else {
        0
    }
}

unsafe extern "C" fn cb_ticks(ctx: *mut c_void) -> i32 {
    let m = &mut *(ctx as *mut Mock);
    m.ticks = m.ticks.wrapping_add(m.tick_step) & (TICKS_PERIOD - 1);
    m.ticks
}

struct Rig {
    mock: Box<Mock>,
    hal: Hal,
}

impl Rig {
    fn new(replies: Vec<Vec<u8>>) -> Rig {
        let mut mock = Box::new(Mock {
            replies,
            tick_step: 1,
            ..Default::default()
        });
        let hal = Hal {
            ctx: &mut *mock as *mut Mock as *mut c_void,
            spi_write: Some(cb_write),
            spi_read: Some(cb_read),
            cs_set: Some(cb_cs),
            busy_get: Some(cb_busy),
            ticks_ms: Some(cb_ticks),
            busy_timeout_ms: 100,
        };
        Rig { mock, hal }
    }

    fn log(&self) -> &[Ev] {
        &self.mock.log
    }

    /// One entry per command, opcode and parameters joined.
    ///
    /// The opcode and the parameters go out as two separate `write` calls
    /// inside one chip-select, so this splits on the selects rather than on
    /// the writes.
    fn commands(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut cur: Option<Vec<u8>> = None;
        for ev in &self.mock.log {
            match ev {
                Ev::Select(0) => cur = Some(Vec::new()),
                Ev::Select(_) => {
                    if let Some(bytes) = cur.take() {
                        if !bytes.is_empty() {
                            out.push(bytes);
                        }
                    }
                }
                Ev::Write(b) => {
                    if let Some(c) = cur.as_mut() {
                        c.extend_from_slice(b);
                    }
                }
                Ev::Read(_) => {}
            }
        }
        out
    }

    /// The bytes of the one command this rig was given.
    fn written(&self) -> Vec<u8> {
        let cmds = self.commands();
        assert_eq!(cmds.len(), 1, "expected exactly one command");
        cmds.into_iter().next().unwrap()
    }
}

/// An expected command: opcode big-endian, then its parameters.
fn frame(opcode: u16, params: &[u8]) -> Vec<u8> {
    let mut v = vec![(opcode >> 8) as u8, opcode as u8];
    v.extend_from_slice(params);
    v
}

/// Two status bytes that are neither all-zeros nor all-ones.
const STATUS: [u8; 2] = [0x80, 0x00];

#[test]
fn a_read_strips_two_status_bytes_not_one() {
    let rig = Rig::new(vec![STATUS.to_vec(), vec![0x01, 0x18]]);
    let v = ops::get_version(&rig.hal).unwrap();

    // Firmware 1.24, which is what the W12's part reports.
    assert_eq!(
        v,
        ops::Version {
            fw_major: 0x01,
            fw_minor: 0x18,
        }
    );
    assert_eq!(
        rig.log(),
        &[
            Ev::Select(0),
            Ev::Write(vec![0x01, 0x01]),
            Ev::Select(1),
            Ev::Select(0),
            Ev::Read(STATUS.to_vec()),
            Ev::Read(vec![0x01, 0x18]),
            Ev::Select(1),
        ]
    );
}

#[test]
fn get_version_asks_for_two_bytes_because_that_is_all_there_is() {
    // Over-reading here is how a firmware version got reported as a hardware
    // and device byte with the version itself falling off the end.
    let rig = Rig::new(vec![STATUS.to_vec(), vec![0x01, 0x18]]);
    ops::get_version(&rig.hal).unwrap();
    match &rig.log()[5] {
        Ev::Read(b) => assert_eq!(b.len(), 2),
        other => panic!("expected a payload read, got {:?}", other),
    }
}

#[test]
fn opcodes_that_hardware_disproved_stay_disproved() {
    // GetRssiInst was 0x0205 here for as long as it took to run it on a board:
    // the wrong opcode returns zeros, which reads as a plausible 0 dBm rather
    // than as an error.
    assert_eq!(cmd::CMD_GET_RSSI_INST, 0x020B);
    assert_eq!(cmd::CMD_GET_RANDOM_NUMBER, 0x0126);
}

#[test]
fn reading_the_rx_fifo_uses_one_chip_select_and_takes_no_status_bytes() {
    // The framing exception that would otherwise shift every received packet
    // two bytes and look like corruption.
    let rig = Rig::new(vec![vec![0xDE, 0xAD, 0xBE, 0xEF]]);
    let mut out = [0u8; 4];
    radio::read_rx_fifo(&rig.hal, &mut out).unwrap();

    assert_eq!(out, [0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(
        rig.log(),
        &[
            Ev::Select(0),
            Ev::Write(vec![0x00, 0x01]),
            Ev::Read(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            Ev::Select(1),
        ]
    );
}

#[test]
fn the_carrier_frequency_goes_out_big_endian_in_hertz() {
    let rig = Rig::new(vec![]);
    radio::set_rf_frequency(&rig.hal, 906_875_000).unwrap();
    assert_eq!(
        rig.written(),
        frame(cmd::CMD_SET_RF_FREQUENCY, &[0x36, 0x0D, 0xD0, 0x78])
    );
}

#[test]
fn lora_modulation_params_pack_two_fields_per_byte() {
    // SF9 / 125 kHz / 4-5 / no LDRO. SF and BW share the first byte, CR and
    // LDRO the second; a swap here still programs a working-looking radio.
    assert_eq!(
        lora::modulation_params(9, cmd::LORA_BW_125, cmd::LORA_CR_4_5, cmd::LORA_LDRO_OFF),
        [0x94, 0x10]
    );
    assert_eq!(
        lora::modulation_params(12, cmd::LORA_BW_250, cmd::LORA_CR_4_8, cmd::LORA_LDRO_ON),
        [0xC5, 0x41]
    );
}

#[test]
fn lora_packet_params_put_the_length_before_the_flags_byte() {
    // Unlike the LR1121's six-byte block: the payload length is third, and
    // header type, CRC and IQ share bits 2:0 of the fourth byte.
    assert_eq!(
        lora::packet_params(
            16,
            cmd::LORA_HEADER_EXPLICIT,
            255,
            cmd::LORA_CRC_ON,
            cmd::LORA_IQ_STANDARD
        ),
        [0x00, 0x10, 0xFF, 0x02]
    );
    assert_eq!(
        lora::packet_params(
            0x0102,
            cmd::LORA_HEADER_IMPLICIT,
            32,
            cmd::LORA_CRC_OFF,
            cmd::LORA_IQ_INVERTED
        ),
        [0x01, 0x02, 0x20, 0x05]
    );
}

#[test]
fn modulation_params_reject_what_the_chip_would_silently_accept() {
    // SF4 and SF13 are outside the part's range.
    assert!(lora::validate(4, cmd::LORA_BW_125, cmd::LORA_CR_4_5, false).is_err());
    assert!(lora::validate(13, cmd::LORA_BW_125, cmd::LORA_CR_4_5, false).is_err());
    // Coding rate 0 does not exist: the numbering starts at 4/5.
    assert!(lora::validate(9, cmd::LORA_BW_125, 0, false).is_err());
    assert!(lora::validate(9, cmd::LORA_BW_125, cmd::LORA_CR_MAX + 1, false).is_err());
    // A code that is not in the table at all.
    assert!(lora::validate(9, 0x09, cmd::LORA_CR_4_5, false).is_err());
    assert!(lora::validate(9, cmd::LORA_BW_125, cmd::LORA_CR_4_5, false).is_ok());
}

#[test]
fn the_fractional_bandwidths_are_refused_below_a_gigahertz() {
    // They are the 2.4 GHz set. Used sub-GHz they configure cleanly and hear
    // nothing, which is the hardest kind of fault to find on a board.
    assert!(lora::validate(9, cmd::LORA_BW_203, cmd::LORA_CR_4_5, false).is_err());
    assert!(lora::validate(9, cmd::LORA_BW_203, cmd::LORA_CR_4_5, true).is_ok());
    assert!(lora::validate(9, cmd::LORA_BW_500, cmd::LORA_CR_4_5, false).is_ok());
}

#[test]
fn bandwidth_codes_are_not_in_bandwidth_order() {
    // Pins the trap: 0x02 is 31.25 kHz but 0x0A is 41.67 kHz, so sorting by
    // code does not sort by bandwidth.
    assert_eq!(cmd::lora_bw_hz(cmd::LORA_BW_31), Some(31_250));
    assert_eq!(cmd::lora_bw_hz(cmd::LORA_BW_41), Some(41_670));
    assert!(cmd::lora_bw_hz(cmd::LORA_BW_41) < cmd::lora_bw_hz(cmd::LORA_BW_125));
    assert_eq!(cmd::lora_bw_hz(0x09), None);
}

#[test]
fn ldro_switches_on_where_a_symbol_reaches_sixteen_milliseconds() {
    // SF11 at 125 kHz is 16.4 ms, SF10 is 8.2 ms.
    assert!(lora::ldro_needed(11, cmd::LORA_BW_125));
    assert!(!lora::ldro_needed(10, cmd::LORA_BW_125));
    // Widening the channel shortens the symbol, so the threshold moves.
    assert!(!lora::ldro_needed(11, cmd::LORA_BW_250));
    assert!(lora::ldro_needed(12, cmd::LORA_BW_250));
}

#[test]
fn packet_status_reassembles_the_ninth_rssi_bit_from_the_flags_byte() {
    // buf[5] carries bit 0 of each RSSI plus the one-hot detector.
    let s = lora::decode_packet_status(&[0x14, 0x20, 0xF8, 0x5A, 0x50, 0x0B]);
    assert_eq!(s.cr, 4);
    assert!(s.has_crc);
    assert_eq!(s.len, 0x20);
    // -8 quarter-dB, i.e. -2 dB.
    assert_eq!(s.snr_pkt, -8);
    assert_eq!(s.rssi_pkt, (0x5A << 1) | 1);
    assert_eq!(s.rssi_signal_pkt, (0x50 << 1) | 1);
    assert_eq!(s.detector, 1);
}

#[test]
fn a_timeout_wider_than_twenty_four_bits_is_refused_rather_than_truncated() {
    let rig = Rig::new(vec![]);
    assert_eq!(radio::set_rx(&rig.hal, 0x0100_0000), Err(ERR_BUFFER));

    let rig = Rig::new(vec![]);
    radio::set_rx(&rig.hal, cmd::RX_CONTINUOUS).unwrap();
    assert_eq!(rig.written(), frame(cmd::CMD_SET_RX, &[0xFF, 0xFF, 0xFF]));
}

#[test]
fn the_interrupt_vector_goes_out_after_the_dio_index() {
    let rig = Rig::new(vec![]);
    radio::set_dio_irq_config(&rig.hal, 1, cmd::IRQ_RX_DONE | cmd::IRQ_TIMEOUT).unwrap();
    assert_eq!(
        rig.written(),
        frame(cmd::CMD_SET_DIO_IRQ_CONFIG, &[0x01, 0x00, 0x24, 0x00, 0x00])
    );
}

#[test]
fn the_power_tables_cover_the_documented_range_and_refuse_the_rest() {
    let rig = Rig::new(vec![]);
    assert_eq!(
        radio::set_power_dbm(&rig.hal, 23, cmd::PA_RAMP_48U),
        Err(ERR_BUFFER)
    );
    let rig = Rig::new(vec![]);
    assert_eq!(
        radio::set_power_dbm(&rig.hal, -10, cmd::PA_RAMP_48U),
        Err(ERR_BUFFER)
    );

    // +22 dBm is the last table row, {6, 7, 35}.
    let rig = Rig::new(vec![]);
    radio::set_power_dbm(&rig.hal, 22, cmd::PA_RAMP_48U).unwrap();
    assert_eq!(
        rig.commands(),
        vec![
            frame(cmd::CMD_SEL_PA, &[cmd::PA_LF]),
            frame(cmd::CMD_SET_PA_CONFIG, &[0x00, 0x67, 0x10]),
            frame(cmd::CMD_SET_TX_PARAMS, &[35, cmd::PA_RAMP_48U]),
        ]
    );
}

#[test]
fn a_payload_longer_than_the_header_can_describe_is_refused() {
    let rig = Rig::new(vec![]);
    let long = [0u8; cmd::MAX_PACKET_LEN + 1];
    assert_eq!(radio::write_tx_fifo(&rig.hal, &long), Err(ERR_BUFFER));
}

#[test]
fn the_rf_switch_takes_two_commands_per_pin() {
    // Without this pair the front end never opens: the radio configures
    // cleanly and hears silence.
    let rig = Rig::new(vec![]);
    ops::set_rf_switch(
        &rig.hal,
        5,
        cmd::DIO_SLEEP_PULL_UP,
        6,
        cmd::DIO_SLEEP_PULL_AUTO,
    )
    .unwrap();
    assert_eq!(
        rig.commands(),
        vec![
            frame(
                cmd::CMD_SET_DIO_FUNCTION,
                &[5, cmd::DIO_FUNCTION_RF_SWITCH | cmd::DIO_SLEEP_PULL_UP]
            ),
            frame(cmd::CMD_SET_DIO_RF_SWITCH_CONFIG, &[5, cmd::RF_SW_TX]),
            frame(
                cmd::CMD_SET_DIO_FUNCTION,
                &[6, cmd::DIO_FUNCTION_RF_SWITCH | cmd::DIO_SLEEP_PULL_AUTO]
            ),
            frame(cmd::CMD_SET_DIO_RF_SWITCH_CONFIG, &[6, cmd::RF_SW_RX]),
        ]
    );
}

