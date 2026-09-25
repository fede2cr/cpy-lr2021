//! LR2021 SPI transaction shape.
//!
//! The framing is shared with the LR1121 and lives in `lrxxxx-hal`. This module
//! adds only what is specific to this part.
//!
//! GAP, deliberate: the LR2021's status byte is *not* decoded here. RadioLib's
//! headers carry no bit layout for it, and the LR1121's (command status in bits
//! 3:1) must not be assumed to carry over — a wrong mapping would turn healthy
//! replies into errors, or worse, hide real ones. Until the datasheet or the
//! hardware settles it, only the two values that cannot be valid on any part
//! are rejected. Resolve this before trusting any error path.

use crate::hal::{Hal, ERR_NO_CHIP};

pub use lrxxxx_hal::xfer::{get_status, write_cmd};

/// Issues a command and clocks back `out.len()` bytes of reply.
///
/// Returns the raw status byte; see the module note on why it is not decoded.
pub fn read_cmd(hal: &Hal, opcode: u16, params: &[u8], out: &mut [u8]) -> Result<u8, i32> {
    // Two status bytes, not the LR11xx's one: confirmed on hardware, where
    // GetVersion's payload only lands in the same place either side of a PRAM
    // activate once two are stripped. The second is undecoded; see the note.
    let [stat, _stat2] = lrxxxx_hal::xfer::read_cmd::<2>(hal, opcode, params, out)?;
    check_status(stat)?;
    Ok(stat)
}

/// Rejects a status byte that means nothing is driving MISO.
pub fn check_status(stat: u8) -> Result<(), i32> {
    // A floating or undriven MISO reads as 0x00 or 0xFF. Neither is a plausible
    // status byte on a part that is answering, whatever the field layout is.
    if stat == 0x00 || stat == 0xFF {
        return Err(ERR_NO_CHIP);
    }
    Ok(())
}

/// Clocks the Rx FIFO out in the same NSS assertion that sent the opcode.
///
/// ReadRxFifo is framed unlike every other read on this part: the payload
/// follows the opcode within one transaction and is *not* preceded by status
/// bytes. Reading it through [`read_cmd`] silently loses the first two payload
/// bytes and shifts the rest, which looks like a corrupt packet rather than a
/// framing mistake.
pub fn read_fifo(hal: &Hal, opcode: u16, out: &mut [u8]) -> Result<(), i32> {
    hal.wait_busy()?;
    let op = [(opcode >> 8) as u8, opcode as u8];
    hal.select(0)?;
    let mut r = hal.write(&op);
    if r.is_ok() {
        r = hal.read(out);
    }
    let deselect = hal.select(1);
    r?;
    deselect?;
    hal.wait_busy()
}
