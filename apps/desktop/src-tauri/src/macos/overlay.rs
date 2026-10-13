//! Native AppKit pet. AppKit objects never leave the Tauri main thread.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
#[cfg(feature = "acceptance-hooks")]
use std::collections::HashSet;
#[cfg(feature = "acceptance-hooks")]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime};

use image::RgbaImage;
use objc2::rc::Retained;
use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::*;
#[cfg(feature = "acceptance-hooks")]
use objc2_foundation::NSDictionary;
use objc2_foundation::{ns_string, NSNumber, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use petsona_core::config::WindowPosition;
use petsona_core::pet::PetState;
use petsona_runtime::commands::RuntimeCommand;
use petsona_runtime::snapshot::RuntimeTextField;
use tauri::AppHandle;

use super::geometry::{legacy_points, resize_anchored, Display, LogicalRect, PhysicalRect};
use crate::Engine;

#[derive(Default)]
struct ViewData {
    image: RefCell<Option<Retained<NSImage>>>,
    fault: Cell<bool>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ViewData]
    struct PetView;
    unsafe impl NSObjectProtocol for PetView {}
    impl PetView {
        #[unsafe(method(isFlipped))]
        fn flipped(&self) -> bool { true }
        #[unsafe(method(isOpaque))]
        fn opaque(&self) -> bool { false }
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool { true }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            // Drop the ivar borrow before invoking drawing, which may reenter.
            let image = self.ivars().image.try_borrow().ok().and_then(|image| image.clone());
            let color = if self.ivars().fault.get() { NSColor::windowBackgroundColor() } else { NSColor::clearColor() };
            color.set(); NSRectFillUsingOperation(self.bounds(), NSCompositingOperation::Copy);
            if let Some(image) = image {
                unsafe { image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(self.bounds(), NSRect::ZERO, NSCompositingOperation::SourceOver, 1.0, true, None); }
            }
        }
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) { with_state(|state| state.mouse_down(event.clickCount())); }
        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, _event: &NSEvent) { with_state(State::mouse_dragged); }
        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) { with_state(State::mouse_up); }
        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) { self.addCursorRect_cursor(self.bounds(), &NSCursor::arrowCursor()); }
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) { with_state(|state| state.wheel_event(event)); }
    }
);

impl PetView {
    fn new(main: MainThreadMarker) -> Retained<Self> {
        let allocated = Self::alloc(main).set_ivars(ViewData::default());
        unsafe { msg_send![super(allocated), initWithFrame: NSRect::ZERO] }
    }
}

define_class!(
    #[unsafe(super = NSPanel)]
    #[thread_kind = MainThreadOnly]
    pub(super) struct PetPanel;
    unsafe impl NSObjectProtocol for PetPanel {}
    impl PetPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool { false }
        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool { false }
    }
);

#[derive(Clone, PartialEq)]
struct AtlasKey {
    path: String,
    cell_width: u32,
    cell_height: u32,
    modified: Option<SystemTime>,
}
struct Atlas {
    pixels: RgbaImage,
    columns: u32,
    rows: u32,
    cell_width: u32,
    cell_height: u32,
    idle_mask: Vec<bool>,
}
impl Atlas {
    fn load(key: &AtlasKey) -> Result<Self, String> {
        let pixels = image::open(&key.path)
            .map_err(|error| error.to_string())?
            .to_rgba8();
        if key.cell_width == 0
            || key.cell_height == 0
            || pixels.width() % key.cell_width != 0
            || pixels.height() % key.cell_height != 0
        {
            return Err("图集尺寸与运行时渲染格不匹配".into());
        }
        let columns = pixels.width() / key.cell_width;
        let rows = pixels.height() / key.cell_height;
        let mut idle_mask = vec![false; (key.cell_width * key.cell_height) as usize];
        for y in 0..key.cell_height {
            for x in 0..key.cell_width {
                idle_mask[(y * key.cell_width + x) as usize] = (0..columns)
                    .any(|column| pixels.get_pixel(column * key.cell_width + x, y)[3] >= 8);
            }
        }
        Ok(Self {
            pixels,
            columns,
            rows,
            cell_width: key.cell_width,
            cell_height: key.cell_height,
            idle_mask,
        })
    }
    fn crop(&self, index: u32) -> Option<RgbaImage> {
        if index >= self.columns * self.rows {
            return None;
        }
        Some(
            image::imageops::crop_imm(
                &self.pixels,
                index % self.columns * self.cell_width,
                index / self.columns * self.cell_height,
                self.cell_width,
                self.cell_height,
            )
            .to_image(),
        )
    }
    fn hit(&self, index: u32, x: i32, y: i32) -> bool {
        if index >= self.columns * self.rows {
            return false;
        }
        // One source-pixel dilation and the complete idle-row union prevent a
        // look pose's transparent edge from making the pet suddenly unclickable.
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let (x, y) = (x + dx, y + dy);
                if x < 0 || y < 0 || x >= self.cell_width as i32 || y >= self.cell_height as i32 {
                    return false;
                }
                let (x, y) = (x as u32, y as u32);
                self.idle_mask[(y * self.cell_width + x) as usize]
                    || self.pixels.get_pixel(
                        index % self.columns * self.cell_width + x,
                        index / self.columns * self.cell_height + y,
                    )[3] >= 8
            })
        })
    }
}

fn native_image(_main: MainThreadMarker, pixels: &RgbaImage) -> Result<Retained<NSImage>, String> {
    let (width, height) = (pixels.width() as isize, pixels.height() as isize);
    // AppKit allocates and owns the planes. Copy straight RGBA into its buffer;
    // no Rust allocation is retained by Objective-C and no PNG roundtrip occurs.
    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), width, height, 8, 4, true, false, NSDeviceRGBColorSpace, NSBitmapFormat::AlphaNonpremultiplied, width * 4, 32
        )
    }.ok_or("无法创建原生图像缓冲")?;
    let data = bitmap.bitmapData();
    if data.is_null() || bitmap.bytesPerRow() < width * 4 {
        return Err("原生图像缓冲无效".into());
    }
    for y in 0..height as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(
                pixels.as_raw().as_ptr().add(y * width as usize * 4),
                data.add(y * bitmap.bytesPerRow() as usize),
                width as usize * 4,
            );
        }
    }
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(width as f64, height as f64));
    image.addRepresentation(&bitmap);
    Ok(image)
}

fn logical(rect: NSRect) -> LogicalRect {
    LogicalRect {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    }
}
fn nsrect(rect: LogicalRect) -> NSRect {
    NSRect::new(
        NSPoint::new(rect.x, rect.y),
        NSSize::new(rect.width, rect.height),
    )
}
fn displays(main: MainThreadMarker) -> Vec<Display> {
    let mut screens: Vec<Display> = NSScreen::screens(main)
        .iter()
        .enumerate()
        .map(|(index, screen)| {
            let id = screen
                .deviceDescription()
                .objectForKey(ns_string!("NSScreenNumber"))
                .and_then(|value| value.downcast::<NSNumber>().ok())
                .map(|value| value.stringValue().to_string())
                .unwrap_or_else(|| format!("display:{index}"));
            Display::new(
                id,
                logical(screen.frame()),
                logical(screen.visibleFrame()),
                screen.backingScaleFactor(),
            )
        })
        .collect();
    if let Some(screen) = NSScreen::mainScreen(main) {
        if let Some(index) = screens
            .iter()
            .position(|display| display.bounds == logical(screen.frame()))
        {
            screens.swap(0, index);
        }
    }
    screens
}

#[derive(Default)]
struct Gaze {
    direction: Option<i32>,
    last: (f64, f64),
    active: bool,
}
impl Gaze {
    fn direction(&mut self, dx: f64, dy: f64) -> i32 {
        let angle = (dx.atan2(-dy).to_degrees() + 360.0) % 360.0;
        let next = (angle / 22.5).round() as i32 % 16;
        if let Some(held) = self.direction {
            if (dx - self.last.0).powi(2) + (dy - self.last.1).powi(2) < 4.0 {
                return held;
            }
            let delta = (angle - f64::from(held) * 22.5 + 540.0) % 360.0 - 180.0;
            if delta.abs() <= 18.25 {
                return held;
            }
        }
        self.direction = Some(next);
        self.last = (dx, dy);
        next
    }
}

struct Drag {
    cursor: NSPoint,
    original: LogicalRect,
    moved: bool,
    double: bool,
    direction: Option<PetState>,
    accumulated: f64,
    last_x: f64,
    sent: Instant,
}
struct Hover {
    active: bool,
    burst: u8,
    due: Instant,
    rng: u64,
}
impl Hover {
    fn new() -> Self {
        Self {
            active: false,
            burst: 0,
            due: Instant::now(),
            rng: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|time| time.as_nanos() as u64)
                .unwrap_or(1)
                | 1,
        }
    }
    fn random(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
    fn reaction(&mut self, on_pet: bool, paused: bool, now: Instant) -> Option<PetState> {
        if !on_pet {
            self.active = false;
            self.burst = 0;
            return None;
        }
        if !self.active {
            self.active = true;
            self.burst = 3;
            self.due = now;
        }
        if paused || now < self.due {
            return None;
        }
        let random = self.random();
        let (state, delay) = if self.burst > 0 {
            self.burst -= 1;
            (PetState::Jumping, 850)
        } else {
            (
                if random & 1 == 0 {
                    PetState::Jumping
                } else {
                    PetState::Waving
                },
                6000 + (random >> 1) % 6001,
            )
        };
        self.due = now + Duration::from_millis(delay);
        Some(state)
    }
}

struct State {
    main: MainThreadMarker,
    app: AppHandle,
    engine: Engine,
    panel: Retained<PetPanel>,
    view: Retained<PetView>,
    fault_label: Retained<NSTextField>,
    display: Option<Display>,
    rect: PhysicalRect,
    requested_frame: Option<NSRect>,
    composer: super::composer::Composer,
    bubble: super::bubble::Bubble,
    composer_side: Option<super::geometry::Side>,
    wheel: WheelTravel,
    position_applied: bool,
    key: Option<AtlasKey>,
    atlas: Option<Atlas>,
    images: HashMap<u32, Retained<NSImage>>,
    sender: mpsc::Sender<(AtlasKey, Result<Atlas, String>)>,
    receiver: mpsc::Receiver<(AtlasKey, Result<Atlas, String>)>,
    sprite: Option<u32>,
    visible: bool,
    ignoring: bool,
    drag: Option<Drag>,
    click_due: Option<Instant>,
    hover: Hover,
    gaze: Gaze,
    render_error: Option<String>,
    fault_since: Option<Instant>,
    fault_text: String,
    stamp_checked: Instant,
    #[cfg(feature = "acceptance-hooks")]
    diagnostic_at: Instant,
    #[cfg(feature = "acceptance-hooks")]
    frames: HashSet<u32>,
    #[cfg(feature = "acceptance-hooks")]
    draw_count: u64,
    #[cfg(feature = "acceptance-hooks")]
    shot_taken: bool,
    #[cfg(feature = "acceptance-hooks")]
    capture_alpha: Option<(usize, usize)>,
    alive: Arc<AtomicBool>,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }
fn with_state(action: impl FnOnce(&mut State)) {
    STATE.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut() {
            if let Some(state) = state.as_mut() {
                action(state);
            }
        }
    });
}

impl State {
    fn send(&self, command: RuntimeCommand) {
        if let Ok(engine) = self.engine.lock() {
            let _ = engine.send(command);
        }
    }
    fn apply_rect(&mut self, display: Display, rect: PhysicalRect) {
        let frame = nsrect(display.to_points(rect));
        if self.requested_frame != Some(frame) {
            self.requested_frame = Some(frame);
            self.panel.setFrame_display(frame, false);
            self.view.setFrame(NSRect::new(NSPoint::ZERO, frame.size));
            self.view.setNeedsDisplay(true);
        }
        if self.composer.open {
            let (side, companion) = super::geometry::panel_rect(
                &display,
                rect,
                296.0 * display.backing,
                self.composer.height * display.backing,
                self.composer_side,
            );
            self.composer_side = Some(side);
            self.composer
                .set_frame(nsrect(display.to_points(companion)));
        }
        self.display = Some(display);
        self.rect = rect;
    }
    fn clear_gaze(&mut self) {
        if self.gaze.active {
            self.send(RuntimeCommand::ClearGaze);
        }
        self.gaze = Gaze::default();
    }
    fn save_position(&self) {
        if let Some(display) = &self.display {
            self.send(RuntimeCommand::SetWindowPosition(display.save(self.rect)));
            // Keep the old global config position alongside the display sidecar.
            let global_top = displays(self.main)
                .iter()
                .map(|screen| screen.bounds.y + screen.bounds.height)
                .fold(f64::NEG_INFINITY, f64::max);
            let point = display.to_points(self.rect);
            self.send(RuntimeCommand::SetPosition {
                x: (point.x * display.backing) as f32,
                y: ((global_top - point.y - point.height) * display.backing) as f32,
            });
            crate::logging::log(&format!(
                "macos: pet position saved display={} x={} y={}",
                display.id, self.rect.x, self.rect.y
            ));
        }
    }
    fn set_visible(&mut self, visible: bool) {
        if visible == self.visible {
            return;
        }
        if !visible {
            self.composer.hide();
            self.composer_side = None;
            self.bubble.hide(&self.engine);
            self.click_due = None;
            self.hover.active = false;
            if self.drag.take().is_some() {
                NSCursor::arrowCursor().set();
            }
        }
        if visible {
            self.panel.orderFrontRegardless();
        } else {
            self.panel.orderOut(None);
        }
        self.visible = visible;
        crate::logging::log(&format!("macos: pet visible={visible}"));
    }
    fn set_ignoring(&mut self, ignoring: bool) {
        if self.ignoring != ignoring {
            self.panel.setIgnoresMouseEvents(ignoring);
            self.ignoring = ignoring;
        }
    }
    fn fault(&mut self, message: &str, locked: bool) {
        if self.fault_text != message {
            self.fault_text = message.to_string();
            self.fault_since = Some(Instant::now());
            self.clear_gaze();
            self.composer.hide();
            self.bubble.hide(&self.engine);
            self.view.ivars().image.replace(None);
            self.view.ivars().fault.set(true);
            self.view.setNeedsDisplay(true);
            self.fault_label
                .setStringValue(&NSString::from_str(message));
            self.fault_label.setHidden(false);
            self.panel
                .setBackgroundColor(Some(&NSColor::windowBackgroundColor()));
            if let Some(display) = displays(self.main).into_iter().next() {
                let rect = display.default_rect(360.0 * display.backing, 100.0 * display.backing);
                self.apply_rect(display, rect);
                self.fault_label.setFrame(NSRect::new(
                    NSPoint::new(16.0, 14.0),
                    NSSize::new(328.0, 72.0),
                ));
            }
            self.set_ignoring(true);
            self.set_visible(true);
            crate::logging::log(&format!(
                "macos: native fault notice locked={locked} message={message}"
            ));
        }
        if locked
            && self
                .fault_since
                .is_some_and(|time| time.elapsed() >= Duration::from_secs(3))
        {
            crate::stop_and_exit(&self.app, &self.engine);
        }
    }
    fn tick(&mut self) {
        let (snapshot, path, saved_text, legacy, error, bubble_text, bubble_timing, state_name) = {
            let Ok(engine) = self.engine.lock() else {
                return;
            };
            (
                engine.snapshot(),
                engine.text(RuntimeTextField::AtlasPath),
                engine.text(RuntimeTextField::WindowPosition),
                engine.text(RuntimeTextField::Position),
                engine.text(RuntimeTextField::Error),
                engine.text(RuntimeTextField::Bubble),
                engine.text(RuntimeTextField::BubbleTiming),
                engine.text(RuntimeTextField::State),
            )
        };
        if snapshot.faulted {
            let locked = error.to_lowercase().contains("lock");
            let message = if locked {
                "已有一个 Petsona 实例使用同一数据目录，本窗口将在 3 秒后退出。".to_string()
            } else {
                format!("Petsona 无法继续：{error}")
            };
            self.fault(&message, locked);
            self.diagnostics("fault");
            return;
        }
        if !snapshot.ready || !snapshot.has_pet || path.is_empty() {
            self.set_visible(false);
            self.clear_gaze();
            self.hover.active = false;
            self.diagnostics("empty");
            return;
        }
        let screens = displays(self.main);
        if screens.is_empty() {
            return;
        }
        let saved: Option<WindowPosition> =
            serde_json::from_str(&saved_text)
                .ok()
                .filter(|p: &WindowPosition| {
                    p.x.is_finite() && p.y.is_finite() && p.backing_scale.is_finite()
                });
        let chosen = if !self.position_applied {
            saved
                .as_ref()
                .and_then(|position| position.display_id.as_ref())
                .and_then(|id| screens.iter().find(|screen| &screen.id == id))
        } else {
            self.display
                .as_ref()
                .and_then(|current| screens.iter().find(|screen| screen.id == current.id))
        };
        let display = chosen.unwrap_or(&screens[0]).clone();
        let width = f64::from(snapshot.cell_width) * f64::from(snapshot.scale);
        let height = f64::from(snapshot.cell_height) * f64::from(snapshot.scale);
        let (display, rect) = if !self.position_applied {
            self.position_applied = true;
            if let Some(saved) = saved {
                let rect = display.restore(&saved, width, height);
                (display, rect)
            } else if let Some((x, y)) = parse_legacy(&legacy) {
                let top = screens
                    .iter()
                    .map(|screen| screen.bounds.y + screen.bounds.height)
                    .fold(f64::NEG_INFINITY, f64::max);
                let points = legacy_points(x, y, width, height, display.backing, top);
                let target = screens
                    .iter()
                    .find(|screen| {
                        screen.bounds.contains(
                            points.x + points.width / 2.0,
                            points.y + points.height / 2.0,
                        )
                    })
                    .unwrap_or(&display)
                    .clone();
                let origin = target.physical_from_points(points);
                let rect = target.clamp(PhysicalRect {
                    width,
                    height,
                    ..origin
                });
                (target, rect)
            } else {
                let rect = display.default_rect(width, height);
                (display, rect)
            }
        } else {
            let current =
                if self.display.as_ref().is_some_and(|old| {
                    old.backing != display.backing || old.bounds != display.bounds
                }) {
                    display.physical_from_points(logical(self.panel.frame()))
                } else {
                    self.rect
                };
            let rect = display.clamp(resize_anchored(current, width, height));
            (display, rect)
        };
        self.apply_rect(display, rect);
        let modified = if self.stamp_checked.elapsed() >= Duration::from_secs(1)
            || self.key.as_ref().is_none_or(|key| key.path != path)
        {
            self.stamp_checked = Instant::now();
            std::fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok()
        } else {
            self.key.as_ref().and_then(|key| key.modified)
        };
        let key = AtlasKey {
            path,
            cell_width: snapshot.cell_width,
            cell_height: snapshot.cell_height,
            modified,
        };
        if self.key.as_ref() != Some(&key) {
            self.click_due = None;
            self.hover.active = false;
            if self.drag.take().is_some() {
                NSCursor::arrowCursor().set();
            }
            self.clear_gaze();
            self.key = Some(key.clone());
            self.atlas = None;
            self.images.clear();
            self.sprite = None;
            self.render_error = None;
            #[cfg(feature = "acceptance-hooks")]
            {
                self.frames.clear();
                self.shot_taken = false;
            }
            let sender = self.sender.clone();
            std::thread::spawn(move || {
                let result = Atlas::load(&key);
                let _ = sender.send((key, result));
            });
        }
        while let Ok((key, result)) = self.receiver.try_recv() {
            if self.key.as_ref() == Some(&key) {
                match result {
                    Ok(atlas) => {
                        self.atlas = Some(atlas);
                        self.fault_text.clear();
                    }
                    Err(error) => self.render_error = Some(error),
                }
            }
        }
        if let Some(error) = self.render_error.clone() {
            self.fault(&format!("宠物图集无法显示：{error}"), false);
            self.diagnostics("render-error");
            return;
        }
        if self.atlas.is_none() {
            self.set_visible(false);
            return;
        }
        if self.view.ivars().fault.replace(false) {
            self.fault_label.setHidden(true);
            self.panel.setBackgroundColor(Some(&NSColor::clearColor()));
            self.view.setNeedsDisplay(true);
        }
        if self.sprite != Some(snapshot.sprite_index) {
            if !self.images.contains_key(&snapshot.sprite_index) {
                // A pet switch can temporarily pair a new V1 atlas with an
                // old V2 pose because snapshot/text publish separately. Wait
                // for the next projection rather than latching a render fault.
                let Some(pixels) = self
                    .atlas
                    .as_ref()
                    .and_then(|atlas| atlas.crop(snapshot.sprite_index))
                else {
                    return;
                };
                match native_image(self.main, &pixels) {
                    Ok(image) => {
                        if self.images.len() >= 256 {
                            self.images.clear();
                        }
                        self.images.insert(snapshot.sprite_index, image);
                    }
                    Err(error) => {
                        self.render_error = Some(error);
                        return;
                    }
                }
            }
            self.view
                .ivars()
                .image
                .replace(self.images.get(&snapshot.sprite_index).cloned());
            self.view.setNeedsDisplay(true);
            self.sprite = Some(snapshot.sprite_index);
            #[cfg(feature = "acceptance-hooks")]
            {
                self.frames.insert(snapshot.sprite_index);
                self.draw_count += 1;
            }
        }
        self.set_visible(snapshot.pet_visible);
        if !self.visible {
            self.clear_gaze();
            self.hover.active = false;
            self.diagnostics("pet");
            return;
        }
        let mouse = NSEvent::mouseLocation();
        let on_pet = self.hit(mouse);
        self.set_ignoring(!on_pet && self.drag.is_none());
        if self.click_due.is_some_and(|time| Instant::now() >= time) {
            self.click_due = None;
            self.send(RuntimeCommand::SetState {
                state: PetState::Waving,
                ttl: None,
            });
            self.send(RuntimeCommand::ShowBubble {
                text: "你好，我在这里".into(),
                ttl: Duration::from_secs(5),
            });
            crate::logging::log("macos: pet clicked -> waving");
        }
        if let Some(display) = self.display.clone() {
            self.bubble.update(
                &bubble_text,
                &bubble_timing,
                &state_name,
                &display,
                self.rect,
                &self.engine,
            );
            self.composer.update_focus(self.main);
            if !self.composer.open {
                self.composer_side = None;
            }
            self.composer.update_height();
            let (side, rect) = super::geometry::panel_rect(
                &display,
                self.rect,
                296.0 * display.backing,
                self.composer.height * display.backing,
                self.composer_side,
            );
            if self.composer.open {
                self.composer_side = Some(side);
            }
            self.composer.set_frame(nsrect(display.to_points(rect)));
        }
        if self.drag.is_some() {
            self.clear_gaze();
        } else if self.composer.open {
            self.aim_gaze(self.composer.caret());
        } else {
            self.update_gaze(mouse);
            self.update_hover(on_pet);
        }
        self.capture();
        self.diagnostics("pet");
    }
    fn open_composer(&mut self) {
        if !self.visible || self.atlas.is_none() || !self.fault_text.is_empty() {
            return;
        }
        self.click_due = None;
        self.hover.burst = 0;
        self.composer_side = None;
        if let Some(display) = &self.display {
            let (side, rect) = super::geometry::panel_rect(
                display,
                self.rect,
                296.0 * display.backing,
                self.composer.height * display.backing,
                None,
            );
            self.composer_side = Some(side);
            self.composer.set_frame(nsrect(display.to_points(rect)));
        }
        self.composer.show(self.main);
    }
    fn wheel_event(&mut self, event: &NSEvent) {
        if self.drag.is_some() || !self.hit(NSEvent::mouseLocation()) {
            return;
        }
        let mut delta = event.scrollingDeltaY();
        if event.isDirectionInvertedFromDevice() {
            delta = -delta;
        }
        if let Some(open) =
            self.wheel
                .action(delta, event.hasPreciseScrollingDeltas(), Instant::now())
        {
            if open && !self.composer.open {
                self.open_composer();
            } else if !open && self.composer.open {
                self.composer.hide();
                self.composer_side = None;
            }
        }
    }
    fn aim_gaze(&mut self, target: NSPoint) {
        let frame = self.panel.frame();
        let backing = self.display.as_ref().map(|d| d.backing).unwrap_or(1.0);
        let dx = (target.x - frame.origin.x - frame.size.width / 2.0) * backing;
        let dy = (frame.origin.y + frame.size.height * 0.68 - target.y) * backing;
        let direction = self.gaze.direction(dx, dy);
        let angle = f64::from(direction) * std::f64::consts::PI / 8.0;
        self.send(RuntimeCommand::SetGazeTarget {
            dx: angle.sin() as f32,
            dy: -angle.cos() as f32,
        });
        self.gaze.active = true;
    }
    fn hit(&self, mouse: NSPoint) -> bool {
        let Some(index) = self.sprite else {
            return false;
        };
        let Some(atlas) = self.atlas.as_ref() else {
            return false;
        };
        let frame = self.panel.frame();
        if !logical(frame).contains(mouse.x, mouse.y) {
            return false;
        }
        let x = ((mouse.x - frame.origin.x) / frame.size.width * f64::from(atlas.cell_width))
            .floor() as i32;
        let y = ((frame.origin.y + frame.size.height - mouse.y) / frame.size.height
            * f64::from(atlas.cell_height))
        .floor() as i32;
        atlas.hit(index, x, y)
    }
    fn mouse_down(&mut self, count: isize) {
        if !self.visible || !self.fault_text.is_empty() {
            return;
        }
        let cursor = NSEvent::mouseLocation();
        if !self.hit(cursor) {
            return;
        }
        self.hover.active = true;
        self.hover.burst = 0;
        self.hover.due = Instant::now() + Duration::from_secs(8);
        let double = count >= 2;
        if double {
            self.click_due = None;
            self.send(RuntimeCommand::SetState {
                state: PetState::Jumping,
                ttl: None,
            });
            crate::logging::log("macos: pet double-clicked -> jumping");
        }
        self.drag = Some(Drag {
            cursor,
            original: logical(self.panel.frame()),
            moved: false,
            double,
            direction: None,
            accumulated: 0.0,
            last_x: cursor.x,
            sent: Instant::now(),
        });
    }
    fn mouse_dragged(&mut self) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        if drag.double {
            return;
        }
        let cursor = NSEvent::mouseLocation();
        let backing = self
            .display
            .as_ref()
            .map(|display| display.backing)
            .unwrap_or(1.0);
        let dx = (cursor.x - drag.cursor.x) * backing;
        let dy = (cursor.y - drag.cursor.y) * backing;
        if !drag.moved && dx.abs() + dy.abs() < 4.0 {
            return;
        }
        drag.moved = true;
        self.click_due = None;
        NSCursor::closedHandCursor().set();
        let points = LogicalRect {
            x: drag.original.x + cursor.x - drag.cursor.x,
            y: drag.original.y + cursor.y - drag.cursor.y,
            ..drag.original
        };
        let delta = (cursor.x - drag.last_x) * backing;
        drag.last_x = cursor.x;
        let direction = drag_direction(&mut drag.direction, &mut drag.accumulated, delta);
        let update = if drag.sent.elapsed() >= Duration::from_millis(80) {
            drag.sent = Instant::now();
            direction
        } else {
            None
        };
        let screens = displays(self.main);
        if let Some(display) = screens
            .iter()
            .find(|display| {
                display.bounds.contains(
                    points.x + points.width / 2.0,
                    points.y + points.height / 2.0,
                )
            })
            .or_else(|| screens.first())
        {
            self.apply_rect(
                display.clone(),
                display.clamp(display.physical_from_points(points)),
            );
        }
        if let Some(state) = update {
            self.send(RuntimeCommand::SetState {
                state,
                ttl: Some(Duration::from_millis(300)),
            });
        }
    }
    fn mouse_up(&mut self) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        NSCursor::arrowCursor().set();
        if drag.moved {
            self.save_position();
            self.send(RuntimeCommand::SetState {
                state: PetState::Idle,
                ttl: Some(Duration::from_millis(1)),
            });
        } else if !drag.double {
            self.click_due =
                Some(Instant::now() + Duration::from_secs_f64(NSEvent::doubleClickInterval()));
        }
    }
    fn update_gaze(&mut self, mouse: NSPoint) {
        let frame = self.panel.frame();
        let backing = self
            .display
            .as_ref()
            .map(|display| display.backing)
            .unwrap_or(1.0);
        let dx = (mouse.x - frame.origin.x - frame.size.width / 2.0) * backing;
        let dy = (frame.origin.y + frame.size.height / 2.0 - mouse.y) * backing;
        let short = self.rect.width.min(self.rect.height);
        let margin = short * if self.gaze.active { 1.0 } else { 0.8 };
        if dx.powi(2) / (self.rect.width / 2.0 + margin).powi(2)
            + dy.powi(2) / (self.rect.height / 2.0 + margin).powi(2)
            > 1.0
            || dx.hypot(dy) <= short * 0.35
        {
            self.clear_gaze();
            return;
        }
        let face_y = (frame.origin.y + frame.size.height * 0.68 - mouse.y) * backing;
        let direction = self.gaze.direction(dx, face_y);
        let angle = f64::from(direction) * std::f64::consts::PI / 8.0;
        self.send(RuntimeCommand::SetGazeTarget {
            dx: angle.sin() as f32,
            dy: -angle.cos() as f32,
        });
        self.gaze.active = true;
    }
    fn update_hover(&mut self, on_pet: bool) {
        if let Some(state) = self.hover.reaction(
            on_pet,
            self.click_due.is_some() || self.drag.is_some() || self.composer.open,
            Instant::now(),
        ) {
            self.send(RuntimeCommand::SetState { state, ttl: None });
            crate::logging::log(&format!("macos: pet hover -> {}", state.name()));
        }
    }
    #[cfg(not(feature = "acceptance-hooks"))]
    fn capture(&mut self) {}

    #[cfg(feature = "acceptance-hooks")]
    fn capture(&mut self) {
        if self.shot_taken {
            return;
        }
        let Some(path) = std::env::var_os("PETSONA_OVERLAY_SHOT") else {
            return;
        };
        if let Some(bitmap) = self
            .view
            .bitmapImageRepForCachingDisplayInRect(self.view.bounds())
        {
            self.view
                .cacheDisplayInRect_toBitmapImageRep(self.view.bounds(), &bitmap);
            if let Some(data) = unsafe {
                bitmap.representationUsingType_properties(
                    NSBitmapImageFileType::PNG,
                    &NSDictionary::new(),
                )
            } {
                let bytes = data.to_vec();
                if let Ok(image) = image::load_from_memory(&bytes) {
                    let rgba = image.to_rgba8();
                    self.capture_alpha = Some((
                        rgba.pixels().filter(|pixel| pixel[3] == 0).count(),
                        rgba.pixels().filter(|pixel| pixel[3] >= 8).count(),
                    ));
                }
                match std::fs::write(path, bytes) {
                    Ok(()) => self.shot_taken = true,
                    Err(error) => {
                        crate::logging::log(&format!("macos: pet capture failed: {error}"))
                    }
                }
            }
        }
    }
    #[cfg(not(feature = "acceptance-hooks"))]
    fn diagnostics(&mut self, _kind: &str) {}

    #[cfg(feature = "acceptance-hooks")]
    fn diagnostics(&mut self, kind: &str) {
        if self.diagnostic_at.elapsed() < Duration::from_millis(200) {
            return;
        }
        self.diagnostic_at = Instant::now();
        let Some(path) = std::env::var_os("PETSONA_OVERLAY_DIAGNOSTICS") else {
            return;
        };
        let frame = self.panel.frame();
        let data = serde_json::json!({ "composer":self.composer.diagnostics(), "bubble":self.bubble.diagnostics(), "kind": kind, "visible": self.panel.isVisible(), "key": self.panel.isKeyWindow(), "opaque": self.panel.isOpaque(), "shadow": self.panel.hasShadow(), "level": self.panel.level(), "physical": {"x":self.rect.x,"y":self.rect.y,"width":self.rect.width,"height":self.rect.height}, "points": {"x":frame.origin.x,"y":frame.origin.y,"width":frame.size.width,"height":frame.size.height}, "backing":self.display.as_ref().map(|display| display.backing), "displayId":self.display.as_ref().map(|display| &display.id), "sprite":self.sprite, "distinctFrames":self.frames.len(), "drawCount":self.draw_count, "ignoresMouse":self.panel.ignoresMouseEvents(), "captureAlpha":self.capture_alpha, "work":self.display.as_ref().map(|d| serde_json::json!({"x":d.work.x,"y":d.work.y,"width":d.work.width,"height":d.work.height})), "idleOpaque":self.atlas.as_ref().map(|atlas| atlas.idle_mask.iter().filter(|hit| **hit).count()) });
        let path = PathBuf::from(path);
        let temporary = path.with_extension("tmp");
        if let Ok(bytes) = serde_json::to_vec_pretty(&data) {
            if std::fs::write(&temporary, bytes).is_ok() {
                let _ = std::fs::rename(temporary, path);
            }
        }
    }
}

fn parse_legacy(text: &str) -> Option<(f64, f64)> {
    let (x, y) = text.split_once(',')?;
    let (x, y) = (x.parse::<f64>().ok()?, y.parse::<f64>().ok()?);
    (x.is_finite() && y.is_finite()).then_some((x, y))
}
fn drag_direction(
    held: &mut Option<PetState>,
    accumulator: &mut f64,
    delta: f64,
) -> Option<PetState> {
    if delta.abs() < 0.001 {
        return *held;
    }
    let next = if delta < 0.0 {
        PetState::RunningLeft
    } else {
        PetState::RunningRight
    };
    if held.is_none() || *held == Some(next) {
        *held = Some(next);
        *accumulator = 0.0;
    } else {
        *accumulator += delta;
        if accumulator.abs() >= 3.0 {
            *held = Some(next);
            *accumulator = 0.0;
        }
    }
    *held
}

pub fn start(app: &AppHandle, engine: Engine) -> Result<(), String> {
    let main = MainThreadMarker::new().ok_or("宠物窗口必须在主线程创建")?;
    let panel: Retained<PetPanel> = unsafe {
        msg_send![PetPanel::alloc(main), initWithContentRect: NSRect::ZERO, styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel, backing: NSBackingStoreType::Buffered, defer: false]
    };
    unsafe {
        panel.setReleasedWhenClosed(false);
    }
    panel.setTitle(ns_string!("Petsona Pet"));
    panel.setOpaque(false);
    panel.setHasShadow(false);
    panel.setHidesOnDeactivate(false);
    panel.setExcludedFromWindowsMenu(true);
    panel.setCanHide(false);
    panel.setFloatingPanel(true);
    panel.setBecomesKeyOnlyIfNeeded(true);
    panel.setLevel(NSFloatingWindowLevel);
    panel.setIgnoresMouseEvents(true);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary,
    );
    let view = PetView::new(main);
    panel.setContentView(Some(&view));
    let label = NSTextField::wrappingLabelWithString(ns_string!(""), main);
    label.setHidden(true);
    label.setFont(Some(&NSFont::systemFontOfSize(14.0)));
    view.addSubview(&label);
    let composer = super::composer::Composer::new(main)?;
    let bubble = super::bubble::Bubble::new(main);
    let (sender, receiver) = mpsc::channel();
    let alive = Arc::new(AtomicBool::new(true));
    STATE.with(|cell| {
        cell.replace(Some(State {
            main,
            app: app.clone(),
            engine,
            panel,
            view,
            fault_label: label,
            display: None,
            rect: PhysicalRect::default(),
            requested_frame: None,
            composer,
            bubble,
            composer_side: None,
            wheel: WheelTravel::default(),
            position_applied: false,
            key: None,
            atlas: None,
            images: HashMap::new(),
            sender,
            receiver,
            sprite: None,
            visible: false,
            ignoring: true,
            drag: None,
            click_due: None,
            hover: Hover::new(),
            gaze: Gaze::default(),
            render_error: None,
            fault_since: None,
            fault_text: String::new(),
            stamp_checked: Instant::now(),
            #[cfg(feature = "acceptance-hooks")]
            diagnostic_at: Instant::now(),
            #[cfg(feature = "acceptance-hooks")]
            frames: HashSet::new(),
            #[cfg(feature = "acceptance-hooks")]
            draw_count: 0,
            #[cfg(feature = "acceptance-hooks")]
            shot_taken: false,
            #[cfg(feature = "acceptance-hooks")]
            capture_alpha: None,
            alive: Arc::clone(&alive),
        }))
    });
    let handle = app.clone();
    let queued = Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || {
        while alive.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(16));
            if queued.swap(true, Ordering::Relaxed) {
                continue;
            }
            let pending = Arc::clone(&queued);
            if handle
                .run_on_main_thread(move || {
                    with_state(State::tick);
                    pending.store(false, Ordering::Relaxed);
                })
                .is_err()
            {
                break;
            }
        }
    });
    crate::logging::log("macos: native pet panel created on main thread");
    Ok(())
}

/// Scale/visibility/persistence acceptance uses runtime commands and native
/// queries, never synthesizes input. The caller enforces isolated test state.
#[cfg(feature = "acceptance-hooks")]
pub fn verify_layout(app: AppHandle, engine: Engine) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        for (scale, visible) in [(1.5f32, true), (1.5, false), (1.5, true), (1.0, true)] {
            if let Ok(engine) = engine.lock() {
                let _ = engine.send(RuntimeCommand::SetScale(scale));
                let _ = engine.send(RuntimeCommand::SetVisibility(visible));
            }
            let mut passed = false;
            for _ in 0..80 {
                std::thread::sleep(Duration::from_millis(50));
                let (sender, receiver) = mpsc::sync_channel(1);
                if app
                    .run_on_main_thread(move || {
                        with_state(|state| {
                            let dimensions = (state.rect.width - 192.0 * f64::from(scale)).abs()
                                < 0.01
                                && (state.rect.height - 208.0 * f64::from(scale)).abs() < 0.01;
                            let pass = dimensions
                                && state.panel.isVisible() == visible
                                && !state.panel.isKeyWindow()
                                && !state.panel.isOpaque()
                                && !state.panel.hasShadow()
                                && state.sprite.is_some();
                            let _ = sender.send(pass);
                        });
                    })
                    .is_err()
                {
                    break;
                }
                if receiver.recv_timeout(Duration::from_secs(1)).ok() == Some(true) {
                    passed = true;
                    break;
                }
            }
            crate::logging::log(&format!(
                "macos: pet layout scale={scale} visible={visible} pass={passed}"
            ));
            if !passed {
                crate::logging::log("macos: pet layout FAIL");
                return;
            }
        }
        let _ = app.run_on_main_thread(|| {
            with_state(|state| {
                state.save_position();
                crate::logging::log("macos: pet layout PASS");
            })
        });
    });
}

pub fn open_composer() {
    with_state(State::open_composer);
}
pub fn close_composer() {
    with_state(|state| {
        state.composer.hide();
        state.composer_side = None;
    });
}
pub fn send_composer() {
    with_state(|state| {
        let text = state.composer.text();
        let text = text.trim();
        if !text.is_empty() {
            state.send(RuntimeCommand::SendConversation(text.into()));
            state.composer.clear();
            crate::logging::log("macos: composer sent");
        }
    });
}

#[derive(Default)]
struct WheelTravel {
    amount: f64,
    direction: i8,
    last: Option<Instant>,
}
impl WheelTravel {
    fn action(&mut self, delta: f64, precise: bool, now: Instant) -> Option<bool> {
        if delta == 0.0 || !delta.is_finite() {
            return None;
        }
        let direction = if delta < 0.0 { -1 } else { 1 };
        if direction != self.direction
            || self
                .last
                .is_some_and(|last| now.duration_since(last) > Duration::from_millis(700))
        {
            self.amount = 0.0;
        }
        self.direction = direction;
        self.last = Some(now);
        self.amount += if precise { delta.abs() / 20.0 } else { 1.0 };
        if self.amount < 3.0 {
            return None;
        }
        self.amount = 0.0;
        Some(direction < 0)
    }
}

#[cfg(feature = "acceptance-hooks")]
pub fn request_open_composer(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        let _ = app.run_on_main_thread(open_composer);
    });
}

#[cfg(feature = "acceptance-hooks")]
pub fn verify_composer(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(8));
        for open in [true, false, true, false] {
            let _ = app.run_on_main_thread(move || {
                with_state(|state| {
                    if open {
                        state.open_composer();
                    } else {
                        state.composer.hide();
                        state.composer_side = None;
                    }
                })
            });
            let mut passed = false;
            for _ in 0..35 {
                std::thread::sleep(Duration::from_millis(50));
                let (sender, receiver) = mpsc::sync_channel(1);
                if app
                    .run_on_main_thread(move || {
                        with_state(|state| {
                            let diagnostic = state.composer.diagnostics();
                            let pass = diagnostic["visible"] == open
                                && (!open
                                    || (diagnostic["key"] == true
                                        && diagnostic["appActive"] == true
                                        && diagnostic["editorFirstResponder"] == true));
                            if pass {
                                state.composer.capture();
                                crate::logging::log(&format!(
                                    "macos: composer verify open={open} pass={pass} {diagnostic}"
                                ));
                            }
                            let _ = sender.send(pass);
                        })
                    })
                    .is_err()
                {
                    break;
                }
                if receiver.recv_timeout(Duration::from_secs(1)).ok() == Some(true) {
                    passed = true;
                    break;
                }
            }
            if !passed {
                let _ = app.run_on_main_thread(|| {
                    with_state(|state| {
                        crate::logging::log(&format!(
                            "macos: composer failure diagnostics {}",
                            state.composer.diagnostics()
                        ))
                    })
                });
                crate::logging::log("macos: composer verify FAIL");
                return;
            }
        }
        // Exercise both window-level and application-level focus loss using
        // only this application's own public APIs, with no input injection.
        for deactivate in [false, true] {
            let _ = app.run_on_main_thread(|| with_state(State::open_composer));
            let mut acquired = false;
            for _ in 0..35 {
                std::thread::sleep(Duration::from_millis(50));
                let (sender, receiver) = mpsc::sync_channel(1);
                let _ = app.run_on_main_thread(move || {
                    with_state(|state| {
                        let diagnostic = state.composer.diagnostics();
                        let _ = sender.send(
                            diagnostic["key"] == true
                                && diagnostic["appActive"] == true
                                && diagnostic["editorFirstResponder"] == true,
                        );
                    });
                });
                if receiver.recv_timeout(Duration::from_secs(1)).ok() == Some(true) {
                    acquired = true;
                    break;
                }
            }
            if !acquired {
                crate::logging::log("macos: composer verify FAIL (blur precondition)");
                return;
            }
            let handle = app.clone();
            let (sender, receiver) = mpsc::sync_channel(1);
            let _ = app.run_on_main_thread(move || {
                with_state(|state| {
                    let _ = sender.send(state.composer.text());
                });
                if deactivate {
                    if let Some(main) = MainThreadMarker::new() {
                        NSApplication::sharedApplication(main).deactivate();
                    }
                } else {
                    crate::show_settings_window(&handle);
                }
            });
            let Ok(before) = receiver.recv_timeout(Duration::from_secs(3)) else {
                crate::logging::log("macos: composer verify FAIL (draft snapshot)");
                return;
            };
            let mut closed = false;
            for _ in 0..35 {
                std::thread::sleep(Duration::from_millis(50));
                let (sender, receiver) = mpsc::sync_channel(1);
                let expected = before.clone();
                let _ = app.run_on_main_thread(move || {
                    with_state(|state| {
                        let diagnostic = state.composer.diagnostics();
                        let pass = !state.composer.open
                            && diagnostic["visible"] == false
                            && state.composer.text() == expected
                            && state.composer_side.is_none();
                        if pass {
                            crate::logging::log(&format!(
                                "macos: composer blur deactivate={deactivate} pass=true draftPreserved=true {diagnostic}"
                            ));
                        }
                        let _ = sender.send(pass);
                    });
                });
                if receiver.recv_timeout(Duration::from_secs(1)).ok() == Some(true) {
                    closed = true;
                    break;
                }
            }
            if !closed {
                crate::logging::log("macos: composer verify FAIL (blur close)");
                return;
            }
        }
        crate::logging::log("macos: composer verify PASS (no input)");
    });
}

pub fn shutdown() {
    STATE.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut() {
            if let Some(state) = state.take() {
                state.alive.store(false, Ordering::Relaxed);
                state.panel.orderOut(None);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_hit_uses_current_and_idle_union_with_edge_dilation() {
        let mut pixels = RgbaImage::new(8, 4);
        pixels.put_pixel(1, 1, image::Rgba([20, 30, 40, 255]));
        pixels.put_pixel(6, 2, image::Rgba([20, 30, 40, 255]));
        let atlas = Atlas {
            pixels,
            columns: 2,
            rows: 1,
            cell_width: 4,
            cell_height: 4,
            idle_mask: vec![
                true, false, false, false, false, true, false, false, false, false, false, false,
                false, false, false, false,
            ],
        };
        assert!(atlas.hit(1, 0, 0));
        assert!(!atlas.hit(1, 3, 0));
        assert!(atlas.hit(1, 3, 2));
        assert!(!atlas.hit(1, -3, 0));
        assert_eq!(atlas.crop(1).unwrap().get_pixel(2, 2)[3], 255);
    }
    #[test]
    fn gaze_covers_sixteen_directions_and_holds_small_motion() {
        for direction in 0..16 {
            let angle = f64::from(direction) * std::f64::consts::PI / 8.0;
            assert_eq!(
                Gaze::default().direction(angle.sin() * 100.0, -angle.cos() * 100.0),
                direction
            );
        }
        let mut gaze = Gaze::default();
        assert_eq!(gaze.direction(0.0, -100.0), 0);
        assert_eq!(gaze.direction(1.0, -100.0), 0);
    }
    #[test]
    fn drag_reverses_only_after_three_pixels_and_legacy_invalid_is_rejected() {
        let mut held = None;
        let mut accumulator = 0.0;
        assert_eq!(
            drag_direction(&mut held, &mut accumulator, 4.0),
            Some(PetState::RunningRight)
        );
        assert_eq!(
            drag_direction(&mut held, &mut accumulator, -1.0),
            Some(PetState::RunningRight)
        );
        assert_eq!(
            drag_direction(&mut held, &mut accumulator, -2.0),
            Some(PetState::RunningLeft)
        );
        assert!(parse_legacy("NaN,20").is_none());
        assert_eq!(parse_legacy("200,300"), Some((200.0, 300.0)));
    }
    #[test]
    fn hover_burst_pause_and_click_cancellation_preserve_reactions() {
        let mut hover = Hover::new();
        let now = Instant::now();
        assert_eq!(hover.reaction(true, false, now), Some(PetState::Jumping));
        assert_eq!(
            hover.reaction(true, false, now + Duration::from_millis(849)),
            None
        );
        assert_eq!(
            hover.reaction(true, false, now + Duration::from_millis(850)),
            Some(PetState::Jumping)
        );
        assert_eq!(
            hover.reaction(true, true, now + Duration::from_millis(1700)),
            None
        );
        assert_eq!(
            hover.reaction(true, false, now + Duration::from_millis(1700)),
            Some(PetState::Jumping)
        );
        assert_eq!(hover.burst, 0);
        let due = hover.due;
        assert!(hover.reaction(true, false, due).is_some());
        assert!((Duration::from_secs(6)..=Duration::from_secs(12))
            .contains(&hover.due.duration_since(due)));
        hover.active = true;
        hover.burst = 0;
        hover.due = now + Duration::from_secs(8);
        assert_eq!(
            hover.reaction(true, false, now + Duration::from_millis(501)),
            None
        );
        assert_eq!(hover.reaction(false, false, now), None);
        assert_eq!(hover.reaction(true, false, now), Some(PetState::Jumping));
    }
    #[test]
    fn atlas_formats_decode_without_changing_source_cell_geometry() {
        let directory = std::env::temp_dir().join(format!(
            "petsona-atlas-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut rgba = RgbaImage::new(8, 4);
        rgba.put_pixel(1, 1, image::Rgba([50, 100, 200, 255]));
        for (extension, format) in [
            ("png", image::ImageFormat::Png),
            ("webp", image::ImageFormat::WebP),
            ("jpg", image::ImageFormat::Jpeg),
        ] {
            let path = directory.join(format!("sheet.{extension}"));
            let picture = image::DynamicImage::ImageRgba8(rgba.clone());
            if format == image::ImageFormat::Jpeg {
                picture.to_rgb8().save_with_format(&path, format).unwrap();
            } else {
                picture.save_with_format(&path, format).unwrap();
            }
            let atlas = Atlas::load(&AtlasKey {
                path: path.to_string_lossy().into_owned(),
                cell_width: 4,
                cell_height: 4,
                modified: None,
            })
            .unwrap();
            assert_eq!((atlas.columns, atlas.rows), (2, 1));
            assert_eq!(atlas.crop(0).unwrap().dimensions(), (4, 4));
            assert!(atlas.crop(2).is_none());
            if format != image::ImageFormat::Jpeg {
                assert_eq!(atlas.pixels, rgba);
            }
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn wheel_requires_three_notches_or_sixty_precise_points() {
        let mut wheel = WheelTravel::default();
        let now = Instant::now();
        assert_eq!(wheel.action(-3.0, false, now), None);
        assert_eq!(wheel.action(-3.0, false, now), None);
        assert_eq!(wheel.action(-3.0, false, now), Some(true));
        assert_eq!(wheel.action(20.0, true, now), None);
        assert_eq!(wheel.action(20.0, true, now + Duration::from_secs(1)), None);
        assert_eq!(
            wheel.action(40.0, true, now + Duration::from_secs(1)),
            Some(false)
        );
    }
}
