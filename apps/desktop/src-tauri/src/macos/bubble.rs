//! Glass text and progress in a strictly nonactivating native panel.
use super::composer::Actions;
use super::geometry::{bubble_rect, Display, PhysicalRect};
use super::overlay::PetPanel;
use crate::Engine;
use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::*;
use objc2_foundation::{ns_string, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use petsona_runtime::commands::RuntimeCommand;
use std::cell::Cell;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub remaining: u64,
    total: u64,
    generation: u64,
}
impl Timing {
    pub fn parse(value: &str) -> Option<Self> {
        let mut parts = value.split(',');
        let remaining = parts.next()?.parse::<i64>().ok()?.max(0) as u64;
        let total = parts.next()?.parse::<i64>().ok()?.max(0) as u64;
        let generation = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            remaining,
            total,
            generation,
        })
    }
    fn progress(self) -> f64 {
        if self.total == 0 {
            1.0
        } else {
            (self.remaining as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }
}

#[derive(Default)]
struct ProgressData {
    value: Cell<f64>,
}
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ProgressData]
    struct Progress;
    unsafe impl NSObjectProtocol for Progress {}
    impl Progress {
        #[unsafe(method(drawRect:))]
        fn draw(&self, _rect: NSRect) {
            NSColor::clearColor().set(); NSRectFillUsingOperation(self.bounds(), NSCompositingOperation::Copy);
            NSColor::secondaryLabelColor().colorWithAlphaComponent(0.18).set(); NSRectFill(self.bounds());
            NSColor::controlAccentColor().set();
            let mut rect = self.bounds(); rect.size.width *= self.ivars().value.get(); NSRectFill(rect);
        }
    }
);
impl Progress {
    fn new(main: MainThreadMarker) -> Retained<Self> {
        let allocated = Self::alloc(main).set_ivars(ProgressData::default());
        unsafe { msg_send![super(allocated),initWithFrame:NSRect::ZERO] }
    }
    fn set_value(&self, value: f64) {
        if (self.ivars().value.get() - value).abs() >= 1.0 / 120.0 {
            self.ivars().value.set(value);
            self.setNeedsDisplay(true);
        }
    }
}

pub struct Bubble {
    panel: Retained<PetPanel>,
    glass: Retained<NSGlassEffectView>,
    content: Retained<NSView>,
    label: Retained<NSTextField>,
    progress: Retained<Progress>,
    button: Retained<NSButton>,
    _actions: Retained<Actions>,
    text: String,
    generation: Option<u64>,
    fade: Instant,
    visible: bool,
    frame: Option<NSRect>,
    hovered: bool,
    width: f64,
    height: f64,
    tint: String,
}
impl Bubble {
    pub fn new(main: MainThreadMarker) -> Self {
        let panel: Retained<PetPanel> = unsafe {
            msg_send![PetPanel::alloc(main),initWithContentRect:NSRect::ZERO,styleMask:NSWindowStyleMask::Borderless|NSWindowStyleMask::NonactivatingPanel,backing:NSBackingStoreType::Buffered,defer:false]
        };
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setTitle(ns_string!("Petsona Bubble"));
        panel.setOpaque(false);
        panel.setHasShadow(false);
        panel.setHidesOnDeactivate(false);
        panel.setCanHide(false);
        panel.setExcludedFromWindowsMenu(true);
        panel.setLevel(NSFloatingWindowLevel);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary,
        );
        let glass = NSGlassEffectView::initWithFrame(NSGlassEffectView::alloc(main), NSRect::ZERO);
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setCornerRadius(12.0);
        let content = NSView::initWithFrame(NSView::alloc(main), NSRect::ZERO);
        glass.setContentView(Some(&content));
        panel.setContentView(Some(&glass));
        let label = NSTextField::wrappingLabelWithString(ns_string!(""), main);
        label.setFont(Some(&NSFont::systemFontOfSize(14.0)));
        label.setMaximumNumberOfLines(10);
        label.setPreferredMaxLayoutWidth(280.0);
        content.addSubview(&label);
        let progress = Progress::new(main);
        content.addSubview(&progress);
        let actions = Actions::new(main);
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!(""),
                Some(&actions),
                Some(sel!(openComposer:)),
                main,
            )
        };
        button.setBordered(false);
        button.setTransparent(true);
        content.addSubview(&button);
        Self {
            panel,
            glass,
            content,
            label,
            progress,
            button,
            _actions: actions,
            text: String::new(),
            generation: None,
            fade: Instant::now(),
            visible: false,
            frame: None,
            hovered: false,
            width: 120.0,
            height: 44.0,
            tint: String::new(),
        }
    }
    pub fn hide(&mut self, engine: &Engine) {
        if self.hovered {
            if let Ok(engine) = engine.lock() {
                let _ = engine.send(RuntimeCommand::SetBubblePaused(false));
            }
            self.hovered = false;
        }
        if self.visible {
            self.panel.orderOut(None);
            self.visible = false;
        }
    }
    pub fn update(
        &mut self,
        text: &str,
        timing: &str,
        state: &str,
        display: &Display,
        pet: PhysicalRect,
        engine: &Engine,
    ) {
        let Some(timing) = Timing::parse(timing).filter(|timing| timing.remaining > 0) else {
            self.hide(engine);
            return;
        };
        if text.is_empty() {
            self.hide(engine);
            return;
        }
        if self.generation != Some(timing.generation) {
            self.generation = Some(timing.generation);
            self.fade = Instant::now();
            self.hovered = false;
        }
        if self.text != text {
            self.text = text.to_string();
            self.label.setStringValue(&NSString::from_str(text));
            self.label.setToolTip(Some(&NSString::from_str(text)));
            let size = self.label.sizeThatFits(NSSize::new(280.0, 200.0));
            self.width = (size.width + 32.0).clamp(120.0, 312.0);
            self.height = (size.height + 28.0).clamp(44.0, 228.0);
        }
        if self.tint != state {
            self.tint = state.into();
            let color = match state {
                "failed" => NSColor::systemRedColor(),
                "waiting" => NSColor::systemOrangeColor(),
                "review" => NSColor::systemPurpleColor(),
                _ => NSColor::controlAccentColor(),
            };
            self.glass
                .setTintColor(Some(&color.colorWithAlphaComponent(0.12)));
        }
        let rect = bubble_rect(
            display,
            pet,
            self.width * display.backing,
            self.height * display.backing,
        );
        let point = display.to_points(rect);
        let frame = NSRect::new(
            NSPoint::new(point.x, point.y),
            NSSize::new(point.width, point.height),
        );
        if self.frame != Some(frame) {
            self.frame = Some(frame);
            self.panel.setFrame_display(frame, false);
            self.content
                .setFrame(NSRect::new(NSPoint::ZERO, frame.size));
            self.label.setFrame(NSRect::new(
                NSPoint::new(16.0, 12.0),
                NSSize::new(
                    (frame.size.width - 32.0).max(40.0),
                    (frame.size.height - 24.0).max(18.0),
                ),
            ));
            self.progress.setFrame(NSRect::new(
                NSPoint::new(14.0, 5.0),
                NSSize::new((frame.size.width - 28.0).max(1.0), 2.0),
            ));
            self.button.setFrame(NSRect::new(NSPoint::ZERO, frame.size));
        }
        let mouse = NSEvent::mouseLocation();
        let hovered = point.contains(mouse.x, mouse.y);
        if hovered != self.hovered {
            self.hovered = hovered;
            if let Ok(engine) = engine.lock() {
                let _ = engine.send(RuntimeCommand::SetBubblePaused(hovered));
            }
        }
        let fade_in = (self.fade.elapsed().as_secs_f64() / 0.15).min(1.0);
        let fade_out = if self.hovered {
            1.0
        } else {
            (timing.remaining as f64 / 700.0).min(1.0)
        };
        let alpha = fade_in.min(fade_out);
        if (self.panel.alphaValue() - alpha).abs() > 0.01 {
            self.panel.setAlphaValue(alpha);
        }
        self.progress.set_value(timing.progress());
        if !self.visible {
            self.panel.orderFrontRegardless();
            self.visible = true;
        }
    }
    #[cfg(feature = "acceptance-hooks")]
    pub fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({"visible":self.panel.isVisible(),"key":self.panel.isKeyWindow(),"progress":self.progress.ivars().value.get(),"hovered":self.hovered,"alpha":self.panel.alphaValue(),"generation":self.generation})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timing_is_bounded_and_rejects_malformed() {
        assert_eq!(Timing::parse("20,100,7").unwrap().progress(), 0.2);
        assert_eq!(Timing::parse("-1,100,7").unwrap().remaining, 0);
        assert_eq!(Timing::parse("120,100,7").unwrap().progress(), 1.0);
        assert!(Timing::parse("1,2").is_none());
        assert!(Timing::parse("1,2,3,4").is_none());
    }
}
