"""Which board this is, and what the two of them must agree on.

Edit `NODE` and nothing else, then copy this file, `code.py` and
`lr2021_flrc.py` to both boards. Everything below has to match on both: an FLRC
receiver only hears a transmitter it is configured to hear, and one wrong rate
code makes a working link look like an empty band.
"""

#: "A" transmits the burst; "B" listens and reports. The only line that differs.
NODE = "A"

#: FLRC lives at 2.4 GHz. Its narrowest bandwidth is 444 kHz, so there is no
#: sub-GHz FLRC to choose -- the band is decided by the modulation.
FREQUENCY_HZ = 2_450_000_000

#: Combined bitrate and bandwidth: one knob, eight values, fastest first.
#: 0 = 2600 kbps/2666 kHz ... 7 = 260 kbps/444 kHz. `SWEEP` walks all eight.
BR_BW = 0

#: 0 = 1/2, 1 = 3/4, 2 = uncoded, 3 = 2/3. **Not in rate order**; picking by
#: arithmetic instead of by name gets it wrong.
CODING_RATE = 3

#: 0x00 none, 0x05 Gaussian BT 0.5, 0x07 Gaussian BT 1.0, 0x09 RRC 0.5.
SHAPING = 0x05

#: Both ends must use the same one. Any value; this is not a Meshtastic mesh.
SYNC_WORD = 0x12345678

#: Well under the +12 dBm this PA can do. Two radios on one bench do not need
#: power, and a loud transmitter at close range saturates the other front end
#: and makes a good link look like a bad one. Attach antennas to both u.FL
#: connectors before transmitting.
POWER_DBM = 0

#: Fixed for the run, so neither end needs a length header.
PAYLOAD_LEN = 64

#: One burst. Enough that the whole burst is timed rather than each packet:
#: at the top rate a packet is under a millisecond on air, and `ticks_ms` has
#: nothing useful to say about that.
PACKETS = 500

#: Run every rate in turn instead of just `BR_BW`, producing the goodput and
#: PER table. Slower, and the point of the exercise.
SWEEP = False
