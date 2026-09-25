"""Just enough LR2021 to put FLRC packets on the air, over bare SPI.

This is bring-up scaffolding, not the driver. Every command layout here comes
from RadioLib's `LR2021_cmds_*.cpp` (mirrored under `reference/lr2021/`), and
all of the arithmetic is deferred to the native `lr2021` module so that the
numbers in a report come from the code that will still be there when the Rust
burst engine replaces this loop.

What it cannot do is measure the radio. A Python per-packet loop at 2.6 Mbps
spends all its time in the interpreter -- a 255-byte packet is under a
millisecond on air -- so treat the goodput it reports as a floor on the link,
not a measurement of the part.
"""

import time

import board
import busio
import digitalio

import lr2021

# ─── commands ────────────────────────────────────────────────────────────────
CMD_READ_RX_FIFO = 0x0001
CMD_WRITE_TX_FIFO = 0x0002
CMD_GET_RX_FIFO_LEVEL = 0x011C
CMD_CLEAR_RX_FIFO = 0x011E
CMD_CLEAR_TX_FIFO = 0x011F
CMD_GET_VERSION = 0x0101
CMD_SET_DIO_FUNCTION = 0x0112
CMD_SET_DIO_RF_SWITCH_CONFIG = 0x0113
CMD_SET_DIO_IRQ_CONFIG = 0x0115
CMD_GET_AND_CLEAR_IRQ_STATUS = 0x0117
CMD_CALIBRATE = 0x0122
CMD_SET_STANDBY = 0x0128
CMD_ACTIVATE_PRAM = 0x012D
CMD_SET_RF_FREQUENCY = 0x0200
CMD_SET_PA_CONFIG = 0x0202
CMD_SET_TX_PARAMS = 0x0203
CMD_SET_PACKET_TYPE = 0x0207
CMD_SET_RX = 0x020C
CMD_SET_TX = 0x020D
CMD_SEL_PA = 0x020F
CMD_SET_FLRC_MODULATION_PARAMS = 0x0248
CMD_SET_FLRC_PACKET_PARAMS = 0x0249
CMD_SET_FLRC_SYNCWORD = 0x024C

PACKET_TYPE_FLRC = 0x05
STANDBY_RC = 0x00

#: Replies carry two status bytes on this part, against the LR1121's one.
STATUS_LEN = 2

IRQ_RX_DONE = 1 << 18
IRQ_TX_DONE = 1 << 19
IRQ_TIMEOUT = 1 << 21
IRQ_CRC_ERROR = 1 << 22

RX_CONTINUOUS = 0xFFFFFF

DIO_FUNCTION_RF_SWITCH = 0x02 << 4
DIO_SLEEP_PULL_UP = 0x02
DIO_SLEEP_PULL_AUTO = 0x03

#: Which modes assert an RF-switch DIO, as a bitmask: standby, Rx, Tx, then the
#: high-band Rx and Tx. Both the plain and the HF bit are set below because the
#: split between them is undocumented outside the datasheet we do not have.
RF_SW_RX = (1 << 1) | (1 << 3)
RF_SW_TX = (1 << 2) | (1 << 4)

#: 48 us, the ramp the LR1121 driver settled on. Affects the spectral mask
#: rather than whether the link closes.
PA_RAMP_48U = 0x05

#: (paDutyCycle, paVal) for -19..+12 dBm on the high band, from RadioLib's
#: `paOptTableHf`. The 2.4 GHz PA is the only one FLRC can use: its narrowest
#: bandwidth is 444 kHz, which puts every FLRC rate above 2 GHz.
_PA_HF = (
    (0, -38), (0, -36), (0, -34), (0, -32), (0, -30), (0, -28), (0, -26),
    (0, -24), (0, -22), (0, -20), (0, -18), (0, -16), (0, -14), (0, -12),
    (0, -10), (0, -8), (0, -6), (0, -4), (0, -2), (14, 4), (14, 6), (12, 7),
    (9, 8), (9, 10), (15, 15), (14, 16), (14, 18), (15, 21), (14, 22),
    (14, 24), (10, 24), (0, 24),
)
MIN_POWER_DBM = -19
MAX_POWER_DBM = 12


class RadioError(RuntimeError):
    pass


class Flrc:
    """One LR2021 configured for FLRC."""

    def __init__(self, baudrate=8_000_000):
        self._spi = busio.SPI(board.LORA_SCK, board.LORA_MOSI, board.LORA_MISO)
        while not self._spi.try_lock():
            pass
        self._spi.configure(baudrate=baudrate, polarity=0, phase=0)
        self._cs = digitalio.DigitalInOut(board.LORA_CS)
        self._cs.switch_to_output(value=True)
        self._busy = digitalio.DigitalInOut(board.LORA_BUSY)
        self._busy.switch_to_input()
        self._reset = digitalio.DigitalInOut(board.LORA_RESET)
        self._reset.switch_to_output(value=True)
        # Active low, and left enabled: whether the radio shares this rail with
        # the OLED is unconfirmed, so the safe state is the one that powers it.
        self._vext = digitalio.DigitalInOut(board.VEXT_ENABLE)
        self._vext.switch_to_output(value=False)
        self._pa = digitalio.DigitalInOut(board.PA_EN_2G4)
        self._pa.switch_to_output(value=True)
        self._payload_len = 0

    def deinit(self):
        for pin in (self._cs, self._busy, self._reset, self._vext, self._pa):
            pin.deinit()
        self._spi.unlock()
        self._spi.deinit()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.deinit()

    # ─── transport ───────────────────────────────────────────────────────────
    def _wait_busy(self, timeout_ms=200):
        deadline = time.monotonic_ns() + timeout_ms * 1_000_000
        while self._busy.value:
            if time.monotonic_ns() > deadline:
                raise RadioError("BUSY stuck high")

    def _cmd(self, opcode, args=b""):
        self._wait_busy()
        self._cs.value = False
        self._spi.write(bytes((opcode >> 8, opcode & 0xFF)) + bytes(args))
        self._cs.value = True

    def _read(self, opcode, length, args=b""):
        self._cmd(opcode, args)
        self._wait_busy()
        buf = bytearray(STATUS_LEN + length)
        self._cs.value = False
        self._spi.readinto(buf)
        self._cs.value = True
        if all(b == 0x00 for b in buf) or all(b == 0xFF for b in buf):
            raise RadioError("no reply to 0x%04X; MISO stuck" % opcode)
        return bytes(buf[STATUS_LEN:])

    def reset(self):
        self._reset.value = False
        time.sleep(0.005)
        self._reset.value = True
        time.sleep(0.020)
        self._wait_busy(timeout_ms=500)

    def version(self):
        return self._read(CMD_GET_VERSION, 4)

    def irq(self):
        b = self._read(CMD_GET_AND_CLEAR_IRQ_STATUS, 4)
        return (b[0] << 24) | (b[1] << 16) | (b[2] << 8) | b[3]

    # ─── configuration ───────────────────────────────────────────────────────
    def begin(self, frequency_hz, br_bw, cr, shaping, sync_word, power_dbm,
              payload_len, preamble=8):
        """Brings the chip up and configures one FLRC channel.

        Raises rather than returning a code: on a bench link a misconfigured
        radio that keeps going is worse than one that stops.
        """
        if not MIN_POWER_DBM <= power_dbm <= MAX_POWER_DBM:
            raise ValueError("power must be %d..%d dBm on the 2.4 GHz PA"
                             % (MIN_POWER_DBM, MAX_POWER_DBM))
        self.reset()
        # Patch RAM before anything else: on this part the radio blocks are not
        # necessarily complete without it, and a link that configures cleanly
        # and then never transmits is what skipping it looks like.
        self._cmd(CMD_ACTIVATE_PRAM, b"\x00")
        self._cmd(CMD_SET_STANDBY, bytes((STANDBY_RC,)))
        self._cmd(CMD_SET_PACKET_TYPE, bytes((PACKET_TYPE_FLRC,)))

        packed = lr2021.flrc_modulation_params(br_bw, cr, shaping)
        self._cmd(CMD_SET_FLRC_MODULATION_PARAMS,
                  bytes(((packed >> 8) & 0xFF, packed & 0xFF)))
        self._set_packet_params(preamble, payload_len)
        self._cmd(CMD_SET_FLRC_SYNCWORD,
                  bytes((0,
                         (sync_word >> 24) & 0xFF, (sync_word >> 16) & 0xFF,
                         (sync_word >> 8) & 0xFF, sync_word & 0xFF)))

        self._cmd(CMD_SET_RF_FREQUENCY,
                  bytes(((frequency_hz >> 24) & 0xFF, (frequency_hz >> 16) & 0xFF,
                         (frequency_hz >> 8) & 0xFF, frequency_hz & 0xFF)))
        self._cmd(CMD_CALIBRATE, b"\x7F")
        self._set_power(power_dbm)
        self._set_rf_switch()
        self._cmd(CMD_SET_DIO_IRQ_CONFIG, b"\x00\xFF\xFF\xFF\xFF")
        self._payload_len = payload_len

    def _set_rf_switch(self):
        """Hands DIO5 and DIO6 to the radio's RF switch logic.

        On the W12 these are the RFX2402E's TXEN and RXEN, so until this runs
        the 2.4 GHz front end stays shut down and the antenna is disconnected
        in both directions. Nothing about that failure looks like a switch
        problem: the radio configures cleanly and hears silence.
        """
        # DIO5 rejects anything but a sleep pull-up, per RadioLib's note.
        for dio, pull, modes in ((5, DIO_SLEEP_PULL_UP, RF_SW_TX),
                                 (6, DIO_SLEEP_PULL_AUTO, RF_SW_RX)):
            self._cmd(CMD_SET_DIO_FUNCTION,
                      bytes((dio, DIO_FUNCTION_RF_SWITCH | pull)))
            self._cmd(CMD_SET_DIO_RF_SWITCH_CONFIG, bytes((dio, modes)))

    def _set_packet_params(self, preamble, payload_len):
        # Fixed length, so both ends agree without a header: a bench link that
        # has to negotiate length has one more way to look broken.
        sync_word_len, sync_word_tx, sync_match, fixed, crc = 4, 1, 1, 1, 2
        self._cmd(CMD_SET_FLRC_PACKET_PARAMS, bytes((
            ((preamble & 0x0F) << 2) | (sync_word_len // 2),
            ((sync_word_tx & 0x03) << 6) | ((sync_match & 0x07) << 3)
            | (fixed << 2) | (crc & 0x03),
            (payload_len >> 8) & 0xFF, payload_len & 0xFF,
        )))

    def _set_power(self, dbm):
        duty, pa_val = _PA_HF[dbm - MIN_POWER_DBM]
        self._cmd(CMD_SEL_PA, b"\x01")
        self._cmd(CMD_SET_PA_CONFIG, bytes((0x80, 0x67, (duty + 0x10) & 0x1F)))
        self._cmd(CMD_SET_TX_PARAMS, bytes((pa_val & 0xFF, PA_RAMP_48U)))

    # ─── traffic ─────────────────────────────────────────────────────────────
    def transmit(self, data, timeout_ms=1000):
        """Sends one packet and waits for TxDone. True if it went out."""
        self._cmd(CMD_CLEAR_TX_FIFO)
        self._cmd(CMD_WRITE_TX_FIFO, data)
        self._cmd(CMD_SET_TX, b"\x00\x00\x00")
        deadline = time.monotonic_ns() + timeout_ms * 1_000_000
        while time.monotonic_ns() < deadline:
            flags = self.irq()
            if flags & IRQ_TX_DONE:
                return True
            if flags & IRQ_TIMEOUT:
                return False
        return False

    def start_receive(self):
        self._cmd(CMD_CLEAR_RX_FIFO)
        self._cmd(CMD_SET_RX, bytes(((RX_CONTINUOUS >> 16) & 0xFF,
                                     (RX_CONTINUOUS >> 8) & 0xFF,
                                     RX_CONTINUOUS & 0xFF)))

    def poll(self):
        """Returns a packet, `False` for a CRC failure, or None if nothing came.

        A CRC failure is reported rather than dropped: it is the difference
        between a link that is not there and one that is there and marginal.
        """
        flags = self.irq()
        if flags & IRQ_CRC_ERROR:
            self._cmd(CMD_CLEAR_RX_FIFO)
            return False
        if not flags & IRQ_RX_DONE:
            return None
        level = self._read(CMD_GET_RX_FIFO_LEVEL, 2)
        length = min((level[0] << 8) | level[1], self._payload_len)
        if length == 0:
            return None
        return self._read_fifo(length)

    def _read_fifo(self, length):
        # The one command whose reply has no status bytes in front of it.
        self._wait_busy()
        buf = bytearray(length)
        self._cs.value = False
        self._spi.write(bytes((CMD_READ_RX_FIFO >> 8, CMD_READ_RX_FIFO & 0xFF)))
        self._spi.readinto(buf)
        self._cs.value = True
        return bytes(buf)
