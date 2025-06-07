use anyhow::Result;
use std::sync::mpsc; // Used for communication between threads
use std::thread;
use windows::{
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT,
            UpdateWindow,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW,
            GetClientRect, GetMessageW, GetSystemMetrics, IDC_ARROW, LWA_COLORKEY, LoadCursorW,
            MSG, PostMessageW, PostQuitMessage, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN,
            SW_SHOWNOACTIVATE, SetLayeredWindowAttributes, ShowWindow, TranslateMessage,
            WM_DESTROY, WM_ERASEBKGND, WM_PAINT, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
        },
    },
    core::PCWSTR,
};

const BORDER_THICKNESS: i32 = 8;

/// A thread-safe wrapper for HWND that implements Send and Sync.
/// This makes it safe to pass the window handle between threads.
#[derive(Clone, Copy)]
struct SendableHWND(HWND);
unsafe impl Send for SendableHWND {}
unsafe impl Sync for SendableHWND {}

impl SendableHWND {
    fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
    fn hwnd(&self) -> HWND {
        self.0
    }
}

/// An overlay window that draws a red border and can be safely closed.
pub struct BorderOverlay {
    /// The handle to the overlay window, wrapped for thread safety.
    hwnd: SendableHWND,
    /// The handle to the thread that manages the window.
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl BorderOverlay {
    /// Creates and displays the border overlay window.
    ///
    /// This function spawns a new thread to handle the window's message loop.
    /// It waits until the window is successfully created before returning.
    pub fn new() -> Result<Self> {
        // Create a channel to receive the SendableHWND from the spawned thread.
        let (tx, rx) = mpsc::channel();

        let thread_handle = thread::spawn(move || {
            // This closure runs on the new thread.
            if let Err(e) = Self::window_thread_main(tx) {
                eprintln!("Window thread failed: {}", e);
            }
        });

        // Block and wait for the spawned thread to send the SendableHWND.
        let hwnd = rx.recv()?;

        Ok(Self {
            hwnd,
            thread_handle: Some(thread_handle),
        })
    }

    /// The main function for the windowing thread.
    fn window_thread_main(tx: mpsc::Sender<SendableHWND>) -> Result<()> {
        // 1. Register the window class.
        let class_name = format!("RustBorderOverlay_{}", std::process::id());
        let wname: Vec<u16> = class_name.encode_utf16().chain(Some(0)).collect();
        let hinst: HINSTANCE = unsafe { GetModuleHandleW(None)? }.into();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(Self::wnd_proc),
            hInstance: hinst,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
            lpszClassName: PCWSTR(wname.as_ptr()),
            ..Default::default()
        };
        unsafe { RegisterClassExW(&wc) };

        // 2. Create a fullscreen overlay window.
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                PCWSTR(wname.as_ptr()),
                PCWSTR::null(),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                screen_w,
                screen_h,
                None,
                None,
                Some(hinst),
                None,
            )?
        };

        // 3. Configure window attributes for transparency.
        unsafe {
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_COLORKEY)?;
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = UpdateWindow(hwnd);
        }

        // Wrap the HWND and send it back to the main thread.
        tx.send(SendableHWND::new(hwnd))
            .expect("Main thread disconnected");

        // 4. Run the message loop.
        let mut msg = MSG::default();
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }

    /// Posts a message to destroy the window and joins the thread.
    pub fn stop(mut self) {
        if let Some(handle) = self.thread_handle.take() {
            unsafe {
                let _ = PostMessageW(Some(self.hwnd.hwnd()), WM_DESTROY, WPARAM(0), LPARAM(0));
            }
            let _ = handle.join();
        }
    }

    /// The Window Procedure (WndProc) to handle messages for our window class.
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => unsafe {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);

                let brush_bkg = CreateSolidBrush(COLORREF(0));
                FillRect(hdc, &rc, brush_bkg);
                let _ = DeleteObject(brush_bkg.into());

                let brush_red = CreateSolidBrush(COLORREF(0x000000FF));
                let borders = [
                    RECT {
                        top: rc.top,
                        bottom: rc.top + BORDER_THICKNESS,
                        ..rc
                    },
                    RECT {
                        top: rc.bottom - BORDER_THICKNESS,
                        bottom: rc.bottom,
                        ..rc
                    },
                    RECT {
                        left: rc.left,
                        right: rc.left + BORDER_THICKNESS,
                        ..rc
                    },
                    RECT {
                        left: rc.right - BORDER_THICKNESS,
                        right: rc.right,
                        ..rc
                    },
                ];
                for b in &borders {
                    FillRect(hdc, b, brush_red);
                }
                let _ = DeleteObject(brush_red.into());

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            },
            WM_ERASEBKGND => LRESULT(1),
            WM_DESTROY => unsafe {
                PostQuitMessage(0);
                LRESULT(0)
            },
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}

impl Drop for BorderOverlay {
    fn drop(&mut self) {
        if let Some(handle) = self.thread_handle.take() {
            unsafe {
                let _ = PostMessageW(Some(self.hwnd.hwnd()), WM_DESTROY, WPARAM(0), LPARAM(0));
            }
            let _ = handle.join();
        }
    }
}
