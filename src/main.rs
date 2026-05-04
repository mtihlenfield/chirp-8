// http://devernay.free.fr/hacks/chip8/C8TECH10.HTM
// https://github.com/Timendus/chip8-test-suite
// https://johnearnest.github.io/chip8Archive/
// TODO: organize as frontend and backend. Frontend uses backend as lib
// bus that has control of everything and routes reads/writes?
// cpu with registers
// ram ?
//     - 4KB
// display
//    - eventually sprites
// timers
// sound
// - TODO: gotta remember to handle endianess correctly for not 8 bit ints and instructions.
// Need to read instructions from ram as a big endian int

// What should the backend api look like? Frontend needs: emulator interface that allows:
//     - loading a program in to ram at an address
//     - exposing the display buffer in some way
//     - passing through key events
//     - sound register
//     - all ram and register access probably needs to be threadsafe? So that the frontend can run
//     the timers, cpu, and display on different threads?
//         - Realistically we may only need one extra thread: the timer thread
// How to handle the clock? The delay and sound registers need to be able to dec at 60hz
//     - Naive implementation would be to just poll the monotonic clock until we hit 1/60th of a
//     second
//     - Could use timer_create() with a monotonic clock to get a signal at 60hz
//         - From what I'm reading this is painful to do in rust
//     - Middle ground appears to be keeping a separate thread that just handles the timing
//         - Passing the delay and sound registers as Arc<AtomicU8> and let the thread update them
//         - Would be a good way to practice with threads and
//      - Generally it's not good for libraries to start their own thread. Maybe we just expose
//      the ability to decrement the registers (in a threadsafe way) and leave it up to the front
//      end to handle how often they are decremented
// The display:
//     - 64 x 32 *pixels*
//     - Display updates done through the dxyn instruction where:
//         - x: the register holding the x coord
//         - y: the registor holding the y coord
//         - n: the number of rows the sprite takes up (aka the height of the sprite in pixels, aka
//         the length of the sprite in bytes)
//         - The address of the sprite that is to be drawn is read from the I register
//         - Sprites are drawn by xor'ing the sprite on to the current screen!
//             - pixel_on XOR existing_on → pixel turns off
//             - pixel_on XOR existing_off → pixel turns on
//             - pixel_off XOR anything → no change
//         - If a draw turns a pixel off, the VF is set to one. Otherwise it is set to 0
//         - Sprites wrap around the screen if they go past the boundaries
//    - Address range 0x000 - 0x04f is supposed to hold the systems default font
use std::error::Error;

mod emu;

fn main() -> Result<(), Box<dyn Error>> {
    let mut emu = emu::Emu::new();
    let program: [u8; 8] = [0x00, 0xe0, 0x00, 0xe0, 0x00, 0xe0, 0x00, 0xe0];
    emu.load(0x200, program.to_vec())?;
    emu.jump(0x200)?;

    for _ in 0..4 {
        emu.step()?;
    }

    Ok(())
}
