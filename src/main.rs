// Reference - http://devernay.free.fr/hacks/chip8/C8TECH10.HTM
// Test Suite: https://github.com/Timendus/chip8-test-suite
// Some games to try: https://johnearnest.github.io/chip8Archive/

use pixels::{Pixels, SurfaceTexture};
use std::env;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowId};

mod emu;

const SCALE: u32 = 10;

// This sets the speed of the emulator.
const STEPS_PER_FRAME: usize = 100;

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
                        .with_title("CHIRP-8")
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
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                let key = match event.key_without_modifiers().as_ref() {
                    keyboard::Key::Character("1") => emu::Key::One,
                    keyboard::Key::Character("2") => emu::Key::Two,
                    keyboard::Key::Character("3") => emu::Key::Three,
                    keyboard::Key::Character("4") => emu::Key::C,
                    keyboard::Key::Character("q") => emu::Key::Four,
                    keyboard::Key::Character("w") => emu::Key::Five,
                    keyboard::Key::Character("e") => emu::Key::Six,
                    keyboard::Key::Character("r") => emu::Key::D,
                    keyboard::Key::Character("a") => emu::Key::Seven,
                    keyboard::Key::Character("s") => emu::Key::Eight,
                    keyboard::Key::Character("d") => emu::Key::Nine,
                    keyboard::Key::Character("f") => emu::Key::E,
                    keyboard::Key::Character("z") => emu::Key::A,
                    keyboard::Key::Character("x") => emu::Key::Zero,
                    keyboard::Key::Character("c") => emu::Key::B,
                    keyboard::Key::Character("v") => emu::Key::F,
                    _ => return,
                };

                let state = if event.state.is_pressed() {
                    emu::KeyState::Pressed
                } else {
                    emu::KeyState::Released
                };

                self.emu.set_key(key, state);
            }
            WindowEvent::RedrawRequested => {
                // TODO: I need to figure out how to make redraws regular (like at 60hz) so that
                // this is somewhat reliable. But I still need to check the time for the timers
                // because redraws can also be triggered by things like resizing the window
                self.emu.tick_timers();
                for _ in 0..STEPS_PER_FRAME {
                    self.emu.step().expect("Program step failed!");
                }

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

    let mut args = env::args();
    args.next();

    let program = if let Some(path) = args.next() {
        std::fs::read(path).unwrap()
    } else {
        // v0 = sprite index
        // v1 = col
        // v2 = row
        // [
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
        // ]
        // .to_vec()

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
