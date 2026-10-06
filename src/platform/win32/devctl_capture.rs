//! HWND WGC capture; no alternate capture backend.
use crate::devtools::{Error, Image, MAX_PIXELS, Result};
use std::{sync::mpsc, time::Duration};
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::*,
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::*,
        Graphics::{Direct3D::*, Direct3D11::*, Dwm::*, Dxgi::*},
        System::WinRT::{Direct3D11::*, Graphics::Capture::*, *},
        UI::WindowsAndMessaging::*,
    },
    core::Interface,
};
fn err(e: impl ToString) -> Error {
    Error::new("CAPTURE_FAILED", e)
}
struct DpiContext(windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT);
impl Drop for DpiContext {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(self.0);
        }
    }
}
pub struct CaptureRuntime;
impl CaptureRuntime {
    pub fn initialize() -> Result<Self> {
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED).map_err(err)?;
        }
        Ok(Self)
    }
}
impl Drop for CaptureRuntime {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}
struct Capture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    token: Option<i64>,
}
impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            let _ = self.pool.RemoveFrameArrived(t);
        }
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
struct Mapped<'a> {
    context: &'a ID3D11DeviceContext,
    texture: &'a ID3D11Texture2D,
}
impl Drop for Mapped<'_> {
    fn drop(&mut self) {
        unsafe {
            self.context.Unmap(self.texture, 0);
        }
    }
}
fn geometry(hwnd: HWND) -> Result<(RECT, POINT, RECT)> {
    unsafe {
        let mut client = RECT::default();
        GetClientRect(hwnd, &mut client).map_err(err)?;
        let origin: POINT = crate::platform::win32::space::client_to_screen(
            hwnd,
            crate::platform::win32::space::ClientPhysicalPoint(crate::geom::PhysicalPoint {
                x: 0,
                y: 0,
            }),
        )
        .ok_or_else(|| err("ClientToScreen failed"))?
        .0
        .into();
        let mut frame = RECT::default();
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut frame as *mut _ as _,
            std::mem::size_of::<RECT>() as u32,
        )
        .map_err(err)?;
        Ok((client, origin, frame))
    }
}
pub fn capture_client(raw: usize) -> Result<(Image, serde_json::Value)> {
    unsafe {
        let old = windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
        if old.0.is_null() {
            return Err(err("cannot establish PMv2 capture coordinates"));
        }
        let _dpi = DpiContext(old);
        if !GraphicsCaptureSession::IsSupported().map_err(err)? {
            return Err(err("Windows Graphics Capture unavailable"));
        }
        let hwnd = HWND(raw as *mut _);
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return Err(err("target window is hidden/minimized"));
        }
        let before = geometry(hwnd)?;
        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                .map_err(err)?;
        let item: GraphicsCaptureItem = interop.CreateForWindow(hwnd).map_err(err)?;
        let size = item.Size().map_err(err)?;
        if size.Width <= 0
            || size.Height <= 0
            || (size.Width as u64) * (size.Height as u64) > MAX_PIXELS as u64
        {
            return Err(err("WGC extent exceeds capture budget"));
        }
        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(err)?;
        let device = device.ok_or_else(|| err("missing D3D11 device"))?;
        let context = context.ok_or_else(|| err("missing D3D11 context"))?;
        let dxgi: IDXGIDevice = device.cast().map_err(err)?;
        let direct: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
            .map_err(err)?
            .cast()
            .map_err(err)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &direct,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .map_err(err)?;
        let session = match pool.CreateCaptureSession(&item) {
            Ok(s) => s,
            Err(e) => {
                let _ = pool.Close();
                return Err(err(e));
            }
        };
        let mut capture = Capture {
            pool,
            session,
            token: None,
        };
        capture
            .session
            .SetIsCursorCaptureEnabled(false)
            .map_err(err)?;
        let (tx, rx) = mpsc::sync_channel(1);
        capture.token = Some(
            capture
                .pool
                .FrameArrived(&TypedEventHandler::new(
                    move |sender: windows::core::Ref<Direct3D11CaptureFramePool>, _| {
                        if let Some(p) = sender.as_ref() {
                            if let Ok(f) = p.TryGetNextFrame() {
                                let _ = tx.try_send(f);
                            }
                        }
                        Ok(())
                    },
                ))
                .map_err(err)?,
        );
        capture.session.StartCapture().map_err(err)?;
        let frame = rx.recv_timeout(Duration::from_secs(5)).map_err(err)?;
        let actual = frame.ContentSize().map_err(err)?;
        if actual.Width != size.Width || actual.Height != size.Height {
            return Err(err("capture size changed during frame acquisition"));
        }
        let access: IDirect3DDxgiInterfaceAccess =
            frame.Surface().map_err(err)?.cast().map_err(err)?;
        let texture: ID3D11Texture2D = access.GetInterface().map_err(err)?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);
        if desc.Width < size.Width as u32 || desc.Height < size.Height as u32 {
            return Err(err("undersized WGC texture"));
        }
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .map_err(err)?;
        let staging = staging.ok_or_else(|| err("missing staging texture"))?;
        context.CopyResource(&staging, &texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(err)?;
        let guard = Mapped {
            context: &context,
            texture: &staging,
        };
        let w = size.Width as u32;
        let h = size.Height as u32;
        if mapped.pData.is_null() || mapped.RowPitch < (w * 4) {
            return Err(err("invalid mapped WGC surface"));
        }
        let mut pixels = vec![0; w as usize * h as usize * 4];
        for y in 0..h as usize {
            let row = std::slice::from_raw_parts(
                (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                w as usize * 4,
            );
            for x in 0..w as usize {
                let i = (y * w as usize + x) * 4;
                pixels[i..i + 4].copy_from_slice(&[
                    row[x * 4 + 2],
                    row[x * 4 + 1],
                    row[x * 4],
                    row[x * 4 + 3],
                ]);
            }
        }
        drop(guard);
        let _ = frame.Close();
        let after = geometry(hwnd)?;
        if before != after {
            return Err(err("window geometry changed during capture"));
        }
        let (client, origin, bounds) = before;
        if i64::from(bounds.right) - i64::from(bounds.left) != i64::from(w)
            || i64::from(bounds.bottom) - i64::from(bounds.top) != i64::from(h)
        {
            return Err(err("WGC item and DWM frame coordinates disagree"));
        }
        let x = i64::from(origin.x) - i64::from(bounds.left);
        let y = i64::from(origin.y) - i64::from(bounds.top);
        let cw = i64::from(client.right) - i64::from(client.left);
        let ch = i64::from(client.bottom) - i64::from(client.top);
        if x < 0 || y < 0 || cw <= 0 || ch <= 0 || x + cw > i64::from(w) || y + ch > i64::from(h) {
            return Err(err("client area outside WGC frame"));
        }
        let (image, _) = Image {
            width: w,
            height: h,
            rgba: pixels,
        }
        .crop(
            crate::devtools::Rect {
                x: x as f64,
                y: y as f64,
                width: cw as f64,
                height: ch as f64,
            },
            0,
        )?;
        Ok((
            image,
            serde_json::json!({"capture_backend":"Windows.Graphics.Capture","coordinate_space":"client-physical-pixels","frame_width":w,"frame_height":h,"client_crop":{"x":x,"y":y,"width":cw,"height":ch}}),
        ))
    }
}
