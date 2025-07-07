use anyhow::Result;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    platform::windows::EventLoopBuilderExtWindows,
    window::{Window, WindowId, WindowLevel},
};

const BORDER_THICKNESS: u32 = 8;
const BORDER_COLOR: u32 = 0x00FF0000; // Red

#[derive(Debug)]
enum UserEvent {
    Shutdown,
}

/// An overlay window that draws a red border and can be safely closed.
pub struct BorderOverlay {
    /// The handle to the thread that manages the window.
    thread_handle: Option<thread::JoinHandle<()>>,
    proxy: Option<EventLoopProxy<UserEvent>>,
}

impl BorderOverlay {
    /// Creates and displays the border overlay window.
    ///
    /// This function spawns a new thread to handle the window's message loop.
    /// It waits until the window is successfully created before returning.
    pub fn new() -> Result<Self> {
        // Create a channel to receive a signal from the spawned thread.
        let (tx, rx) = mpsc::channel();

        let thread_handle = thread::spawn(move || {
            // This closure runs on the new thread.
            if let Err(e) = Self::window_thread_main(tx) {
                eprintln!("Window thread failed: {e}");
            }
        });

        // Block and wait for the spawned thread to send the signal.
        let proxy = rx.recv()?;

        Ok(Self {
            thread_handle: Some(thread_handle),
            proxy: Some(proxy),
        })
    }

    /// The main function for the windowing thread.
    fn window_thread_main(tx: mpsc::Sender<EventLoopProxy<UserEvent>>) -> Result<()> {
        let event_loop = EventLoop::with_user_event()
            .with_any_thread(true)
            .build()?;
        let proxy = event_loop.create_proxy();
        let mut state = State::default();
        // Send signal once the event loop is created
        tx.send(proxy).expect("Main thread disconnected");
        let _ = event_loop.run_app(&mut state);
        Ok(())
    }

    /// Posts a message to destroy the window and joins the thread.
    pub fn stop(mut self) {
        if let Some(proxy) = self.proxy.take() {
            proxy.send_event(UserEvent::Shutdown).ok();
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Default)]
struct State {
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
}

impl ApplicationHandler<UserEvent> for State {
    // This is a common indicator that you can create a window.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let primary_monitor = event_loop.primary_monitor().unwrap();
        let monitor_size = primary_monitor.size();
        let monitor_pos = primary_monitor.position();

        let window_attributes = Window::default_attributes()
            .with_decorations(false)
            .with_transparent(true)
            .with_position(monitor_pos)
            .with_inner_size(monitor_size)
            .with_active(false)
            .with_window_level(WindowLevel::AlwaysOnTop);
        let window = Rc::new(event_loop.create_window(window_attributes).unwrap());
        if let Err(e) = window.set_cursor_hittest(false) {
            eprintln!("Failed to set cursor hittest: {e}");
        }
        let context = Context::new(window.clone()).unwrap();
        let surface = Surface::new(&context, window.clone()).unwrap();
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Shutdown => {
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_ref() {
            Some(window) => window,
            None => return,
        };

        if window.id() != window_id {
            return;
        }
        let surface = self.surface.as_mut().unwrap();
        match event {
            WindowEvent::RedrawRequested => {
                let (width, height) = {
                    let size = window.inner_size();
                    (size.width, size.height)
                };

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(width), NonZeroU32::new(height))
                {
                    surface.resize(width, height).unwrap();

                    let mut buffer = surface.buffer_mut().unwrap();
                    for y in 0..height.get() {
                        for x in 0..width.get() {
                            let color = if x < BORDER_THICKNESS
                                || x >= width.get() - BORDER_THICKNESS
                                || y < BORDER_THICKNESS
                                || y >= height.get() - BORDER_THICKNESS
                            {
                                BORDER_COLOR
                            } else {
                                0x00000000 // Transparent
                            };
                            buffer[(y * width.get() + x) as usize] = color;
                        }
                    }

                    buffer.present().unwrap();
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }
}

impl Drop for BorderOverlay {
    fn drop(&mut self) {
        if let Some(proxy) = self.proxy.take() {
            proxy.send_event(UserEvent::Shutdown).ok();
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}
