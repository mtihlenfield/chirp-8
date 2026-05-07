// http://devernay.free.fr/hacks/chip8/C8TECH10.HTM
// https://github.com/Timendus/chip8-test-suite
// https://johnearnest.github.io/chip8Archive/

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
//
// The UI:
//      - us pixels and winit lib?
//
use pixels::{Pixels, SurfaceTexture};
use std::env;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

mod emu;

const SCALE: u32 = 10;

struct App<'win> {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'win>>,
    emu: emu::Emu,
}

impl<'win> Default for App<'win> {
    fn default() -> App<'win> {
        App {
            window: None,
            pixels: None,
            emu: emu::Emu::default(),
        }
    }
}

impl<'win> ApplicationHandler for App<'win> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("CHIP-8")
                        .with_inner_size(winit::dpi::LogicalSize::new(
                            (emu::DISPLAY_COLS as u32) * SCALE,
                            (emu::DISPLAY_ROWS as u32) * SCALE,
                        )),
                )
                .unwrap(),
        );

        let size = window.inner_size();
        let surface_texture = SurfaceTexture::new(size.width, size.height, window.clone());
        let pixels = Pixels::new(
            emu::DISPLAY_COLS as u32,
            emu::DISPLAY_ROWS as u32,
            surface_texture,
        )
        .unwrap();

        self.window = Some(window);
        self.pixels = Some(pixels);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // TODO: one instruction per redraw is very slow.
                self.emu.step().expect("Program step failed!");

                let emu_frame_buff = self.emu.frame_buffer();

                if let Some(pixels) = &mut self.pixels {
                    let frame = pixels.frame_mut();
                    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
                        let value = match emu_frame_buff[i] {
                            0 => 0x00,
                            _ => 0xff,
                        };

                        // These are r, g, b, a values
                        pixel.copy_from_slice(&[value, value, value, 0xFF]);
                    }
                    pixels.render().unwrap();
                }

                // Request the next frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => (),
        }
    }
}

impl<'win> App<'win> {
    pub fn load(&mut self, data: Vec<u8>) -> Result<(), emu::EmuError> {
        self.emu.load(emu::UNRESERVED_START, data)?;
        self.emu.jump(emu::UNRESERVED_START)
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();

    // v0 = sprite index
    // v1 = col
    // v2 = row
    // let program: [u8; 32] = [
    //     0x60, 0x01, // Load the index of the hex sprite in to v0
    //     0x61, 0x00, // Load 0x00 in to v1 - the x coord for the sprite
    //     0x62, 0x00, // Load 0x00 in to v2 - the y coord for the sprite
    //     0xf0, 0x29, // Set I to the sprite V0
    //     0xd1, 0x25, // Display the 5 byte sprite at pixel (v1, v2)
    //     0x70, 0x01, // bump v0 to the next index
    //     0x40, 0x11, // skip the next instr if  v0 =! 17
    //     0x12, 0x1e, // jump to the infinite loop
    //     0x40, 0x0a, // skip the next instr if v0 =! 10
    //     0x12, 0x18, // jump 4 instrs forward (0x218)
    //     0x71, 0x05, // move the col over to the next char
    //     0x12, 0x04, // Loop back to the point where we set I and start again
    //     0x72, 0x06, // incr v2 by 6 (chars are 5 rows, add 1 row for spacing)
    //     0x61, 0x00, // set v1 to 0
    //     0x12, 0x06, // Loop back to the point where we set I and start again
    //     0x12, 0x1e, // Loop forever
    // ];

    let mut args = env::args();
    args.next();

    let program = if let Some(path) = args.next() {
        std::fs::read(path).unwrap()
    } else {
        [
            0x60, 0x0f, // Load the index of the hex sprite in to v0
            0x61, 0x00, // Load 0x00 in to v1 - the x coord for the sprite
            0x62, 0x00, // Load 0x00 in to v2 - the y coord for the sprite
            0xf0, 0x29, // Set I to the sprite V0
            0xd1, 0x25, // Display the 5 byte sprite at pixel (v1, v2)
            0x12, 0x00, // Loop forever
        ]
        .to_vec()
    };

    app.load(program).expect("Failed to load in program");

    event_loop
        .run_app(&mut app)
        .expect("Panic'd during app run!");
}
