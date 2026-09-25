//! Driver core for the Semtech LR2021, built to be linked into a CircuitPython
//! native module.
//!
//! The crate is `no_std` and allocation-free. Command framing, the BUSY
//! handshake and the host vtable come from `lrxxxx-hal`, which this shares with
//! the LR1121 driver; what lives here is the LR2021's own command set — LoRa,
//! for the mesh, and FLRC, from the bring-up that came before it.
//!
//! Scope note: opcode values and parameter layouts are taken from RadioLib's
//! `LR2021_commands.h` and `LR2021_cmds_*.cpp`, mirrored under
//! `reference/lr2021/`. Anything those do not pin down is marked as such
//! rather than guessed.

#![cfg_attr(not(test), no_std)]

pub mod cmd;
pub mod ffi;
pub mod flrc;
pub mod lora;
pub mod radio;
pub mod system;
pub mod xfer;

//: The vtable is chip-agnostic and shared with the LR1121 driver; re-exported
//: here so this crate can spell it `crate::hal`.
pub use lrxxxx_hal::hal;

#[cfg(test)]
mod tests;

/// Required to link the `no_std` staticlib.
///
/// Everything, `core` included, is compiled with `panic = "immediate-abort"`,
/// so every panic site becomes a trap and this is never reached; it exists only
/// to satisfy the linker.
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
