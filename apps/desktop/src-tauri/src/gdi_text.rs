//! Minimal GDI+ text helper for the native overlays.
//!
//! The old frontends rendered bubble/strip copy with System.Drawing (GDI+):
//! Microsoft YaHei UI 14pt, `TextRenderingHint.AntiAliasGridFit`, drawn into a
//! premultiplied ARGB bitmap. This module exposes just enough of the GDI+
//! flat API to measure and draw text into our premultiplied BGRA buffers.

use std::ptr;
use std::sync::OnceLock;

use windows_sys::Win32::Graphics::GdiPlus::{
    GdipCreateBitmapFromScan0, GdipCreateFont, GdipCreateFontFamilyFromName, GdipCreateSolidFill,
    GdipCreateStringFormat, GdipDeleteBrush, GdipDeleteFont, GdipDeleteFontFamily,
    GdipDeleteGraphics, GdipDeleteStringFormat, GdipDisposeImage, GdipDrawString,
    GdipGetImageGraphicsContext, GdipMeasureString, GdipSetSmoothingMode, GdipSetStringFormatAlign,
    GdipSetStringFormatLineAlign, GdipSetTextRenderingHint, GdiplusStartup, GdiplusStartupInput,
    GpBitmap, GpBrush, GpFont, GpFontFamily, GpGraphics, GpImage, GpSolidFill, GpStringFormat,
    RectF, SmoothingModeAntiAlias, StringAlignmentNear, TextRenderingHintAntiAliasGridFit,
    UnitPixel,
};

/// Documented `PixelFormat32bppPARGB` (windows-sys omits the GDI+ format
/// constants).
const PIXEL_FORMAT_32BPP_PARGB: i32 = 0x000E_200B;
const FONT_FAMILY: &str = "Microsoft YaHei UI";
const FONT_FAMILY_FALLBACK: &str = "Segoe UI";

static GDIPLUS_TOKEN: OnceLock<Option<usize>> = OnceLock::new();

fn ensure_gdiplus() -> bool {
    match GDIPLUS_TOKEN.get_or_init(|| unsafe {
        let mut token = 0usize;
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: 0,
            SuppressExternalCodecs: 0,
        };
        let status = GdiplusStartup(&mut token, &input, ptr::null_mut());
        if status == 0 {
            Some(token)
        } else {
            None
        }
    }) {
        Some(_) => true,
        None => false,
    }
}

unsafe fn create_font_pair(size_px: f32) -> Option<(*mut GpFontFamily, *mut GpFont)> {
    for name in [FONT_FAMILY, FONT_FAMILY_FALLBACK] {
        let wide = wide(name);
        let mut family: *mut GpFontFamily = ptr::null_mut();
        if GdipCreateFontFamilyFromName(wide.as_ptr(), ptr::null_mut(), &mut family) != 0
            || family.is_null()
        {
            continue;
        }
        let mut font: *mut GpFont = ptr::null_mut();
        let status = GdipCreateFont(family, size_px, 0, UnitPixel, &mut font);
        if status == 0 && !font.is_null() {
            return Some((family, font));
        }
        GdipDeleteFontFamily(family);
    }
    None
}

unsafe fn create_format() -> Option<*mut GpStringFormat> {
    let mut format: *mut GpStringFormat = ptr::null_mut();
    if GdipCreateStringFormat(0, 0, &mut format) != 0 || format.is_null() {
        return None;
    }
    GdipSetStringFormatAlign(format, StringAlignmentNear);
    GdipSetStringFormatLineAlign(format, StringAlignmentNear);
    Some(format)
}

unsafe fn create_measure_graphics() -> Option<(*mut GpBitmap, *mut GpGraphics)> {
    let mut bitmap: *mut GpBitmap = ptr::null_mut();
    if GdipCreateBitmapFromScan0(1, 1, 0, PIXEL_FORMAT_32BPP_PARGB, ptr::null(), &mut bitmap) != 0
        || bitmap.is_null()
    {
        return None;
    }
    let mut graphics: *mut GpGraphics = ptr::null_mut();
    if GdipGetImageGraphicsContext(bitmap as *mut GpImage, &mut graphics) != 0 || graphics.is_null()
    {
        GdipDisposeImage(bitmap as *mut GpImage);
        return None;
    }
    GdipSetTextRenderingHint(graphics, TextRenderingHintAntiAliasGridFit);
    GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);
    Some((bitmap, graphics))
}

/// Measures wrapped text within `max_width` × `max_height` (pixels).
pub fn measure(text: &str, size_px: f32, max_width: f32, max_height: f32) -> Option<(f32, f32)> {
    if text.is_empty() || !ensure_gdiplus() {
        return None;
    }
    unsafe {
        let (family, font) = create_font_pair(size_px)?;
        let format = create_format();
        let graphics_pair = create_measure_graphics();
        if let (Some(format), Some((bitmap, graphics))) = (format, graphics_pair) {
            let layout = RectF {
                X: 0.0,
                Y: 0.0,
                Width: max_width,
                Height: max_height,
            };
            let mut bounding = RectF {
                X: 0.0,
                Y: 0.0,
                Width: 0.0,
                Height: 0.0,
            };
            let mut fitted = 0i32;
            let mut lines = 0i32;
            let wide_text = wide(text);
            let status = GdipMeasureString(
                graphics,
                wide_text.as_ptr(),
                (wide_text.len() - 1) as i32,
                font,
                &layout,
                format,
                &mut bounding,
                &mut fitted,
                &mut lines,
            );
            GdipDeleteGraphics(graphics);
            GdipDisposeImage(bitmap as *mut GpImage);
            GdipDeleteStringFormat(format);
            GdipDeleteFont(font);
            GdipDeleteFontFamily(family);
            if status == 0 {
                return Some((bounding.Width, bounding.Height));
            }
        } else {
            if let Some(format) = format {
                GdipDeleteStringFormat(format);
            }
            if let Some((bitmap, graphics)) = graphics_pair {
                GdipDeleteGraphics(graphics);
                GdipDisposeImage(bitmap as *mut GpImage);
            }
        }
        GdipDeleteFont(font);
        GdipDeleteFontFamily(family);
        None
    }
}

/// Draws text into a premultiplied BGRA buffer (`width * 4` stride).
#[allow(clippy::too_many_arguments)]
pub fn draw(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    x: f32,
    y: f32,
    layout_width: f32,
    layout_height: f32,
    text: &str,
    size_px: f32,
    argb: u32,
) -> bool {
    if text.is_empty() || !ensure_gdiplus() {
        return false;
    }
    unsafe {
        let Some((family, font)) = create_font_pair(size_px) else {
            return false;
        };
        let Some(format) = create_format() else {
            GdipDeleteFont(font);
            GdipDeleteFontFamily(family);
            return false;
        };

        let mut bitmap: *mut GpBitmap = ptr::null_mut();
        let stride = width * 4;
        let status = GdipCreateBitmapFromScan0(
            width,
            height,
            stride,
            PIXEL_FORMAT_32BPP_PARGB,
            buffer.as_ptr(),
            &mut bitmap,
        );
        if status != 0 || bitmap.is_null() {
            GdipDeleteStringFormat(format);
            GdipDeleteFont(font);
            GdipDeleteFontFamily(family);
            return false;
        }

        let mut graphics: *mut GpGraphics = ptr::null_mut();
        let status = GdipGetImageGraphicsContext(bitmap as *mut GpImage, &mut graphics);
        if status != 0 || graphics.is_null() {
            GdipDisposeImage(bitmap as *mut GpImage);
            GdipDeleteStringFormat(format);
            GdipDeleteFont(font);
            GdipDeleteFontFamily(family);
            return false;
        }
        GdipSetTextRenderingHint(graphics, TextRenderingHintAntiAliasGridFit);
        GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        let mut brush: *mut GpSolidFill = ptr::null_mut();
        let mut drawn = false;
        if GdipCreateSolidFill(argb, &mut brush) == 0 && !brush.is_null() {
            let layout = RectF {
                X: x,
                Y: y,
                Width: layout_width,
                Height: layout_height,
            };
            let wide_text = wide(text);
            drawn = GdipDrawString(
                graphics,
                wide_text.as_ptr(),
                (wide_text.len() - 1) as i32,
                font,
                &layout,
                format,
                brush as *mut GpBrush,
            ) == 0;
            GdipDeleteBrush(brush as *mut GpBrush);
        }

        GdipDeleteGraphics(graphics);
        GdipDisposeImage(bitmap as *mut GpImage);
        GdipDeleteStringFormat(format);
        GdipDeleteFont(font);
        GdipDeleteFontFamily(family);
        drawn
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
