"""Two-node FLRC link test for the LR2021.

Copy this, `config.py` and `lr2021_flrc.py` to the root of CIRCUITPY on both
boards, set `NODE` to "A" on one and "B" on the other, and put `lr2021.mpy` in
`lib/` on both. Attach an antenna to the 2.4 GHz u.FL connector on each: FLRC
is a 2.4 GHz mode, and transmitting into an open circuit can damage the PA.

Start B first, then A within a second or two. There is no control channel, so
the two sides stay together only by giving every rate the same fixed window;
`WINDOW_S` is the slack that buys.

What comes out is a floor, not a measurement. The per-packet loop is Python, so
at the top rate the interpreter is slower than the radio by a wide margin. The
sweep is still worth running: the *shape* of goodput and PER against rate is
what says whether the link is sound, and that survives a slow transmitter.
"""

import time

import config
import lr2021
from lr2021_flrc import Flrc

#: Seconds each rate gets. Long enough to cover the burst plus the drift
#: between two boards started by hand.
WINDOW_S = 12

#: Marks the start of every payload, so a stray packet from something else on
#: the band is not counted as ours.
MAGIC = b"LR21"


def rates():
    return range(8) if config.SWEEP else (config.BR_BW,)


def _describe(br_bw):
    return "%d kbps / %d kHz" % (lr2021.flrc_bitrate_kbps(br_bw),
                                 lr2021.flrc_bandwidth_khz(br_bw))


def _packet(seq):
    body = MAGIC + bytes(((seq >> 8) & 0xFF, seq & 0xFF))
    return body + bytes(config.PAYLOAD_LEN - len(body))


def _seq_of(packet):
    if len(packet) < 6 or packet[:4] != MAGIC:
        return None
    return (packet[4] << 8) | packet[5]


def transmit(radio, br_bw):
    radio.begin(config.FREQUENCY_HZ, br_bw, config.CODING_RATE, config.SHAPING,
                config.SYNC_WORD, config.POWER_DBM, config.PAYLOAD_LEN)
    print("  %s: sending %d packets" % (_describe(br_bw), config.PACKETS))
    started = time.monotonic()
    sent = 0
    for seq in range(config.PACKETS):
        if radio.transmit(_packet(seq)):
            sent += 1
    elapsed_ms = int((time.monotonic() - started) * 1000)
    # The Rust module does the arithmetic even here, where Python could: it is
    # the same code the burst engine will report with, so the two agree.
    bps = lr2021.flrc_goodput_bps(sent * config.PAYLOAD_LEN, elapsed_ms)
    print("  sent %d/%d in %d ms, %d kbps offered"
          % (sent, config.PACKETS, elapsed_ms, bps // 1000))
    return sent


def receive(radio, br_bw):
    radio.begin(config.FREQUENCY_HZ, br_bw, config.CODING_RATE, config.SHAPING,
                config.SYNC_WORD, config.POWER_DBM, config.PAYLOAD_LEN)
    radio.start_receive()
    print("  %s: listening" % _describe(br_bw))
    deadline = time.monotonic() + WINDOW_S
    got, crc_errors, first_ms, last_ms = 0, 0, None, None
    while time.monotonic() < deadline:
        packet = radio.poll()
        if packet is None:
            continue
        if packet is False:
            crc_errors += 1
            continue
        if _seq_of(packet) is None:
            continue
        got += 1
        last_ms = int(time.monotonic() * 1000)
        if first_ms is None:
            first_ms = last_ms
    span = (last_ms - first_ms) if (first_ms is not None and last_ms) else 0
    per = lr2021.flrc_per_permille(config.PACKETS, got)
    bps = lr2021.flrc_goodput_bps(got * config.PAYLOAD_LEN, span)
    print("  got %d/%d, %d CRC errors, PER %d.%d%%, %d kbps"
          % (got, config.PACKETS, crc_errors, per // 10, per % 10, bps // 1000))
    return got, per, bps


def run():
    role = config.NODE.upper()
    if role not in ("A", "B"):
        raise ValueError("NODE must be 'A' or 'B'")
    print("FLRC link test, node %s" % role)

    with Flrc() as radio:
        radio.reset()
        print("version: %s" % " ".join("%02x" % b for b in radio.version()))
        results = []
        for br_bw in rates():
            window_end = time.monotonic() + WINDOW_S
            if role == "A":
                transmit(radio, br_bw)
                # Idle out the rest of the window so both sides step together.
                while time.monotonic() < window_end:
                    pass
            else:
                results.append((br_bw,) + receive(radio, br_bw))

    if role == "B" and len(results) > 1:
        print()
        print("rate            PER     goodput")
        for br_bw, _got, per, bps in results:
            print("%-14s  %2d.%d%%  %5d kbps"
                  % (_describe(br_bw), per // 10, per % 10, bps // 1000))


run()
