"""Bare-SPI bring-up checks for the LR2021 on a Meshnology W12.

Nothing here transmits, and nothing here imports the radio half of the native
module -- there isn't one yet. These are the checks that have to pass before
`flrc_link` is worth running, in the order that keeps one failure from being
blamed on the next thing along.

The rail check is first on purpose. If the radio sits behind the same switched
supply as the OLED, every later probe reads all-0x00 or all-0xFF and looks
exactly like a dead chip.

Copy to CIRCUITPY and run `import probe_lr2021`.
"""

import time

import board
import busio
import digitalio

#: 16-bit, big-endian, same shape as the LR1121's.
CMD_GET_VERSION = 0x0101
CMD_GET_STATUS = 0x0100
CMD_ACTIVATE_PRAM = 0x012D
CMD_NOP = 0x00

#: RadioLib configures the LR2021's reply status as *two* bytes, where the
#: LR1121 uses one. Probe 4 is what confirms that on silicon, and the answer
#: decides whether `lrxxxx-hal`'s shared `read_cmd` can be used unchanged.
STATUS_LEN = 2


def _sense(pin):
    """Returns (with_pullup, with_pulldown), which says whether a pin is driven."""
    d = digitalio.DigitalInOut(pin)
    d.switch_to_input(pull=digitalio.Pull.UP)
    time.sleep(0.002)
    up = d.value
    d.switch_to_input(pull=digitalio.Pull.DOWN)
    time.sleep(0.002)
    down = d.value
    d.deinit()
    return up, down


def _verdict(up, down):
    if up and not down:
        return "FLOATING - nothing is driving it"
    if not up and not down:
        return "driven LOW"
    if up and down:
        return "driven HIGH"
    return "inconsistent (noise?)"


class _Bus:
    """CS, BUSY and reset around a configured SPI bus."""

    def __init__(self, baudrate=2_000_000):
        self.spi = busio.SPI(board.LORA_SCK, board.LORA_MOSI, board.LORA_MISO)
        while not self.spi.try_lock():
            pass
        self.spi.configure(baudrate=baudrate, polarity=0, phase=0)
        self.cs = digitalio.DigitalInOut(board.LORA_CS)
        self.cs.switch_to_output(value=True)
        self.busy = digitalio.DigitalInOut(board.LORA_BUSY)
        self.busy.switch_to_input()
        self.reset = digitalio.DigitalInOut(board.LORA_RESET)
        self.reset.switch_to_output(value=True)

    def close(self):
        for pin in (self.cs, self.busy, self.reset):
            pin.deinit()
        self.spi.unlock()
        self.spi.deinit()

    def pulse_reset(self):
        self.reset.value = False
        time.sleep(0.005)
        self.reset.value = True
        time.sleep(0.020)

    def wait_busy(self, timeout_ms=100):
        deadline = time.monotonic_ns() + timeout_ms * 1_000_000
        while self.busy.value:
            if time.monotonic_ns() > deadline:
                return False
        return True

    def write_cmd(self, opcode, args=b""):
        self.wait_busy()
        self.cs.value = False
        self.spi.write(bytes((opcode >> 8, opcode & 0xFF)) + bytes(args))
        self.cs.value = True

    def read_reply(self, length):
        """The whole reply transaction, status bytes included and undecoded."""
        self.wait_busy()
        buf = bytearray(length)
        self.cs.value = False
        self.spi.readinto(buf)
        self.cs.value = True
        return bytes(buf)


def probe_module():
    """Probe 0: the native module answers, so the toolchain is not the problem.

    Worth doing before touching a pin. If this fails the .mpy is wrong for this
    CPU, and no amount of wiring work will help.
    """
    print("probe 0: native module")
    try:
        import lr2021
    except ImportError as err:
        print("  FAIL: %s" % err)
        print("  build lr2021.mpy for xtensawin and put it in lib/")
        return False
    kbps = lr2021.flrc_bitrate_kbps(0)
    print("  flrc_bitrate_kbps(0) = %s (expect 2600)" % kbps)
    return kbps == 2600


def probe_rail():
    """Probe 1: does the radio come up with VEXT off, on, or either?

    VEXT_ENABLE is active low. Both states are tried because which one the
    radio needs is not established -- the schematic shows the rail feeding the
    OLED, and whether the LR2021 shares it has never been confirmed.
    """
    print("probe 1: VEXT rail")
    vext = digitalio.DigitalInOut(board.VEXT_ENABLE)
    results = {}
    for state, label in ((True, "rail OFF"), (False, "rail ON")):
        vext.switch_to_output(value=state)
        time.sleep(0.050)
        up, down = _sense(board.LORA_BUSY)
        results[label] = _verdict(up, down)
        print("  %-8s BUSY: %s" % (label, results[label]))
    # Left on: everything after this needs whatever the radio needs, and "on"
    # is the state that can only be wrong by wasting current.
    vext.switch_to_output(value=False)
    time.sleep(0.050)
    return results


def probe_busy():
    """Probe 2: BUSY is driven, so something is powered and running."""
    print("probe 2: BUSY presence")
    up, down = _sense(board.LORA_BUSY)
    verdict = _verdict(up, down)
    print("  BUSY: %s" % verdict)
    if "FLOATING" in verdict:
        print("  a powered LR2021 drives BUSY push-pull; this looks unpowered")
        return False
    return True


def probe_version(bus):
    """Probe 3: reset, then GetVersion. The real test of the shared transport."""
    print("probe 3: reset and GetVersion")
    bus.pulse_reset()
    if not bus.wait_busy(timeout_ms=200):
        print("  FAIL: BUSY never went low after reset")
        return None
    bus.write_cmd(CMD_GET_VERSION)
    reply = bus.read_reply(STATUS_LEN + 4)
    print("  raw reply: %s" % " ".join("%02x" % b for b in reply))
    if all(b == 0x00 for b in reply) or all(b == 0xFF for b in reply):
        print("  FAIL: MISO is stuck; the chip is not answering")
        return None
    return reply


def probe_status_shape(bus):
    """Probe 4: how many status bytes precede a reply, and what do they mean?

    RadioLib says two for this part against the LR1121's one, and the driver's
    `xfer.rs` leaves the decode as a documented gap. Reading a known-good
    command beside a deliberately invalid one is what fills it: the bits that
    differ are the ones that carry the command result.
    """
    print("probe 4: status byte shape")
    bus.write_cmd(CMD_GET_VERSION)
    good = bus.read_reply(STATUS_LEN + 4)
    # 0xFFFF is not a command, so the chip should flag it rather than answer.
    bus.write_cmd(0xFFFF)
    bad = bus.read_reply(STATUS_LEN + 4)
    print("  after GetVersion : %s" % " ".join("%02x" % b for b in good))
    print("  after bad opcode : %s" % " ".join("%02x" % b for b in bad))
    differing = [i for i in range(min(len(good), len(bad))) if good[i] != bad[i]]
    print("  bytes that differ: %s" % (differing or "none - status may be elsewhere"))
    return good, bad


def probe_pram(bus):
    """Probe 5: ActivatePram, which the LR1121 has no equivalent of.

    If the LR2021 needs its patch RAM loaded before the radio blocks work, a
    link that configures cleanly and then never transmits is the symptom.
    """
    print("probe 5: ActivatePram")
    bus.write_cmd(CMD_ACTIVATE_PRAM, bytes((CMD_NOP,)))
    ok = bus.wait_busy(timeout_ms=200)
    print("  BUSY returned low: %s" % ok)
    bus.write_cmd(CMD_GET_STATUS)
    print("  status after: %s"
          % " ".join("%02x" % b for b in bus.read_reply(STATUS_LEN + 2)))
    return ok


def run():
    if not probe_module():
        return
    print()
    probe_rail()
    print()
    if not probe_busy():
        print("stopping: without a powered chip the SPI probes prove nothing")
        return
    print()
    bus = _Bus()
    try:
        if probe_version(bus) is None:
            return
        print()
        probe_status_shape(bus)
        print()
        probe_pram(bus)
    finally:
        bus.close()
    print()
    print("done. If probe 3 and 4 look sane, flrc_link is worth trying.")


run()
