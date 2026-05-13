// Reference - http://devernay.free.fr/hacks/chip8/C8TECH10.HTM
// Test Suite: https://github.com/Timendus/chip8-test-suite
// Some games to try: https://johnearnest.github.io/chip8Archive/

use pixels::{Pixels, SurfaceTexture};
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowId};

mod emu;

const SCALE: u32 = 10;

const TIMER_HZ: u64 = 60;
const TIMER_INTERVAL: Duration = Duration::from_micros(1_000_000 / TIMER_HZ);
const STEPS_PER_FRAME: usize = 100;

struct App<'win> {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'win>>,
    emu: emu::Emu,
    next_tick: Instant,
}

impl<'win> Default for App<'win> {
    fn default() -> App<'win> {
        App {
            window: None,
            pixels: None,
            emu: emu::Emu::default(),
            next_tick: Instant::now(),
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();

        if now >= self.next_tick {
            self.emu.tick_timers();

            // TODO: handle sound

            for _ in 0..STEPS_PER_FRAME {
                self.emu.step().expect("Program step failed!");
            }

            // Request the next frame
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            self.next_tick += TIMER_INTERVAL;
            if self.next_tick < Instant::now() {
                self.next_tick = Instant::now() + TIMER_INTERVAL;
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_tick));
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
        [
            // Little program that lets you move the sprite with the a/w/s/d keys
            0x60, 0x00, // 0x00 - Load the index of the hex sprite in to v0
            0x61, 0x1c, // 0x02 - Set the x coord for the sprite
            0x62, 0x0e, // 0x04 - Set the y coord for the sprite
            0x63, 0x01, // 0x06 - Set the amount by which we will jump when a key is pressed
            0xf0, 0x29, // 0x08 - Set I to the sprite V0
            0xd1, 0x25, // 0x0a - Display the 5 byte sprite at pixel (v1, v2)
            0xf7, 0x0a, // 0x0c - Wait for any key press before continuing
            0xd1, 0x25, // 0x0e - Display the sprite again to clear it
            0x47, 0x09, // 0x10 - skip the next instr if the key was not keypad 9
            0x81, 0x34, // 0x12 - Bump the x coord to the right
            0x47, 0x07, // 0x14 - skip the next instr if the key was not keypad 7
            0x81, 0x35, // 0x16 - Bump the x coord to the left
            0x47, 0x05, // 0x18 - skip the next instr if the key was not keypad 5
            0x82, 0x35, // 0x1a - Bump the y coord up
            0x47, 0x08, // 0x1c - skip the next instr if the key was not keypad 8
            0x82, 0x34, // 0x1e - Bump the y coord down
            0x12, 0x0a, // 0x20 - Jump back to the draw instr
        ]
        .to_vec()
    };

    app.load(program).expect("Failed to load in program");

    event_loop
        .run_app(&mut app)
        .expect("Panic'd during app run!");
}
