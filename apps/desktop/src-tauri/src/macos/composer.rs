//! Native glass input. TextKit owns editing, IME, selection and scrolling.
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::*;
use objc2_foundation::{ns_string, NSObject, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub enum KeyAction {
    Send,
    Close,
    Edit,
}
pub fn key_action(code: u16, shift: bool, marked: bool) -> KeyAction {
    if marked {
        return KeyAction::Edit;
    }
    match code {
        36 | 76 if !shift => KeyAction::Send,
        53 => KeyAction::Close,
        _ => KeyAction::Edit,
    }
}

define_class!(
    #[unsafe(super = NSTextView)]
    #[thread_kind = MainThreadOnly]
    pub(super) struct ComposerEditor;
    unsafe impl NSObjectProtocol for ComposerEditor {}
    impl ComposerEditor {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            match key_action(event.keyCode(), event.modifierFlags().contains(NSEventModifierFlags::Shift), self.hasMarkedText()) {
                KeyAction::Send => super::overlay::send_composer(),
                KeyAction::Close => super::overlay::close_composer(),
                KeyAction::Edit => unsafe { let _: () = msg_send![super(self), keyDown: event]; },
            }
        }
    }
);
impl ComposerEditor {
    fn new(main: MainThreadMarker) -> Retained<Self> {
        unsafe {
            msg_send![Self::alloc(main), initWithFrame: NSRect::new(NSPoint::ZERO, NSSize::new(230.0, 22.0))]
        }
    }
}

define_class!(
    #[unsafe(super = NSPanel)]
    #[thread_kind = MainThreadOnly]
    struct ComposerPanel;
    unsafe impl NSObjectProtocol for ComposerPanel {}
    impl ComposerPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn key(&self) -> bool { true }
        #[unsafe(method(canBecomeMainWindow))]
        fn main(&self) -> bool { false }
        #[unsafe(method(close))]
        fn close(&self) { super::overlay::close_composer(); }
    }
);

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    pub(super) struct Actions;
    unsafe impl NSObjectProtocol for Actions {}
    impl Actions {
        #[unsafe(method(sendComposer:))]
        fn send(&self, _sender: Option<&AnyObject>) { super::overlay::send_composer(); }
        #[unsafe(method(openComposer:))]
        fn open(&self, _sender: Option<&AnyObject>) { super::overlay::open_composer(); }
    }
);
impl Actions {
    pub fn new(main: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::alloc(main), init] }
    }
}

pub struct Composer {
    panel: Retained<ComposerPanel>,
    _glass: Retained<NSGlassEffectView>,
    content: Retained<NSView>,
    scroll: Retained<NSScrollView>,
    editor: Retained<ComposerEditor>,
    button: Retained<NSButton>,
    _actions: Retained<Actions>,
    container: Retained<NSTextContainer>,
    layout: Retained<NSLayoutManager>,
    pub open: bool,
    frame: Option<NSRect>,
    pub height: f64,
    last_text: String,
    focus_deadline: Option<Instant>,
    focus_attempt: Instant,
    focus_acquired: bool,
}

impl Composer {
    pub fn new(main: MainThreadMarker) -> Result<Self, String> {
        let panel: Retained<ComposerPanel> = unsafe {
            msg_send![ComposerPanel::alloc(main), initWithContentRect: NSRect::ZERO, styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel, backing: NSBackingStoreType::Buffered, defer: false]
        };
        unsafe {
            panel.setReleasedWhenClosed(false);
        }
        panel.setBecomesKeyOnlyIfNeeded(false);
        panel.setFloatingPanel(true);
        panel.setTitle(ns_string!("Petsona Composer"));
        panel.setOpaque(false);
        panel.setHasShadow(false);
        panel.setHidesOnDeactivate(false);
        panel.setExcludedFromWindowsMenu(true);
        panel.setCanHide(false);
        panel.setLevel(NSFloatingWindowLevel);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary,
        );
        let glass = NSGlassEffectView::initWithFrame(NSGlassEffectView::alloc(main), NSRect::ZERO);
        glass.setCornerRadius(12.0);
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        let content = NSView::initWithFrame(NSView::alloc(main), NSRect::ZERO);
        glass.setContentView(Some(&content));
        panel.setContentView(Some(&glass));
        let editor = ComposerEditor::new(main);
        editor.setRichText(false);
        editor.setEditable(true);
        editor.setSelectable(true);
        editor.setDrawsBackground(false);
        editor.setFont(Some(&NSFont::systemFontOfSize(14.0)));
        editor.setTextColor(Some(&NSColor::labelColor()));
        editor.setInsertionPointColor(Some(&NSColor::labelColor()));
        editor.setTextContainerInset(NSSize::new(0.0, 2.0));
        editor.setHorizontallyResizable(false);
        editor.setVerticallyResizable(true);
        editor.setAutomaticQuoteSubstitutionEnabled(false);
        editor.setAutomaticDashSubstitutionEnabled(false);
        editor.setAutomaticTextReplacementEnabled(false);
        let container = unsafe { editor.textContainer() }.ok_or("原生文本容器未创建")?;
        let layout = unsafe { editor.layoutManager() }.ok_or("原生文本布局未创建")?;
        container.setLineFragmentPadding(0.0);
        container.setWidthTracksTextView(true);
        container.setContainerSize(NSSize::new(230.0, 100_000.0));
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(main), NSRect::ZERO);
        scroll.setBorderType(NSBorderType::NoBorder);
        scroll.setDrawsBackground(false);
        scroll.setHasVerticalScroller(false);
        scroll.setHasHorizontalScroller(false);
        scroll.setDocumentView(Some(&editor));
        content.addSubview(&scroll);
        let actions = Actions::new(main);
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("↑"),
                Some(&actions),
                Some(sel!(sendComposer:)),
                main,
            )
        };
        button.setBezelStyle(NSBezelStyle::Circular);
        button.setBezelColor(Some(&NSColor::controlAccentColor()));
        button.setFont(Some(&NSFont::systemFontOfSize(16.0)));
        button.setEnabled(false);
        content.addSubview(&button);
        Ok(Self {
            panel,
            _glass: glass,
            content,
            scroll,
            editor,
            button,
            _actions: actions,
            container,
            layout,
            open: false,
            frame: None,
            height: 44.0,
            last_text: String::new(),
            focus_deadline: None,
            focus_attempt: Instant::now(),
            focus_acquired: false,
        })
    }
    pub fn show(&mut self, main: MainThreadMarker) {
        self.open = true;
        self.focus_acquired = false;
        self.focus_deadline = Some(Instant::now() + Duration::from_millis(1500));
        self.focus_attempt = Instant::now();
        let length = self.editor.string().length();
        self.editor.setSelectedRange(NSRange::new(length, 0));
        self.editor.scrollRangeToVisible(NSRange::new(length, 0));
        // Opening the editor explicitly requests keyboard focus. A nonactivating
        // panel alone may prepare its first responder without obtaining key focus.
        // Activation is asynchronous; the existing bounded retry handles that delay.
        let application = NSApplication::sharedApplication(main);
        if !application.isActive() {
            application.activate();
        }
        self.panel.makeKeyAndOrderFront(None);
        self.panel.makeFirstResponder(Some(&self.editor));
        if self.panel.isKeyWindow()
            && application.isActive()
            && self
                .panel
                .firstResponder()
                .is_some_and(|responder| responder.isKindOfClass(ComposerEditor::class()))
        {
            self.focus_acquired = true;
            self.focus_deadline = None;
        }
        crate::logging::log("macos: composer opened (native TextKit)");
    }
    pub fn hide(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        self.focus_acquired = false;
        self.focus_deadline = None;
        self.panel.orderOut(None);
        crate::logging::log("macos: composer closed (draft kept)");
    }
    pub fn text(&self) -> String {
        self.editor.string().to_string()
    }
    pub fn clear(&mut self) {
        self.editor.setString(ns_string!(""));
        self.last_text.clear();
        self.height = 44.0;
        self.editor.setSelectedRange(NSRange::new(0, 0));
        self.button.setEnabled(false);
    }
    pub fn update_focus(&mut self, main: MainThreadMarker) {
        if !self.open {
            return;
        }
        let focused = self.panel.isKeyWindow() && NSApplication::sharedApplication(main).isActive();
        if focused {
            self.focus_acquired = true;
        } else if self.focus_acquired {
            // Poll on the main-thread tick instead of re-entering overlay state
            // from an AppKit resign-key callback. TextKit keeps the draft intact.
            self.hide();
            crate::logging::log("macos: composer lost focus -> closed (draft kept)");
            return;
        }
        let Some(deadline) = self.focus_deadline else {
            return;
        };
        if !self.open
            || (focused
                && self
                    .panel
                    .firstResponder()
                    .is_some_and(|responder| responder.isKindOfClass(ComposerEditor::class())))
        {
            self.focus_deadline = None;
            return;
        }
        if Instant::now() > deadline {
            self.hide();
            crate::logging::log("macos: composer initial focus timeout");
            return;
        }
        if self.focus_attempt.elapsed() < Duration::from_millis(100) {
            return;
        }
        self.focus_attempt = Instant::now();
        self.panel.makeKeyAndOrderFront(None);
        self.panel.makeFirstResponder(Some(&self.editor));
    }
    pub fn update_height(&mut self) {
        let text = self.text();
        if text == self.last_text {
            return;
        }
        self.last_text = text;
        self.layout.ensureLayoutForTextContainer(&self.container);
        let used = self
            .layout
            .usedRectForTextContainer(&self.container)
            .size
            .height;
        self.height = (used + 24.0).clamp(44.0, 132.0);
        self.button.setEnabled(!self.last_text.trim().is_empty());
    }
    pub fn set_frame(&mut self, frame: NSRect) {
        if self.frame == Some(frame) {
            return;
        }
        self.frame = Some(frame);
        self.panel.setFrame_display(frame, false);
        let (width, height) = (frame.size.width, frame.size.height);
        self.content
            .setFrame(NSRect::new(NSPoint::ZERO, frame.size));
        let text_width = (width - 58.0).max(80.0);
        self.scroll.setFrame(NSRect::new(
            NSPoint::new(12.0, 10.0),
            NSSize::new(text_width, (height - 20.0).max(22.0)),
        ));
        self.container
            .setContainerSize(NSSize::new(text_width, 100_000.0));
        self.layout.ensureLayoutForTextContainer(&self.container);
        let used = self
            .layout
            .usedRectForTextContainer(&self.container)
            .size
            .height;
        self.editor
            .setFrameSize(NSSize::new(text_width, (used + 4.0).max(height - 20.0)));
        self.button.setFrame(NSRect::new(
            NSPoint::new(width - 36.0, (height - 28.0) / 2.0),
            NSSize::new(28.0, 28.0),
        ));
    }
    pub fn caret(&self) -> NSPoint {
        // This uses the selection/TextKit layout, not a system caret that
        // disappears when focus moves to a different application.
        let selection = self.editor.selectedRange();
        let end = (selection.location + selection.length).min(self.editor.string().length());
        let rect = unsafe {
            self.editor
                .firstRectForCharacterRange_actualRange(NSRange::new(end, 0), std::ptr::null_mut())
        };
        let target = if rect.size.height > 0.0 {
            NSPoint::new(rect.origin.x + 1.0, rect.origin.y + rect.size.height / 2.0)
        } else {
            let local = self.editor.convertRect_toView(
                NSRect::new(NSPoint::new(0.0, 2.0), NSSize::new(1.0, 18.0)),
                None,
            );
            let screen = self.panel.convertRectToScreen(local);
            NSPoint::new(
                screen.origin.x + 1.0,
                screen.origin.y + screen.size.height / 2.0,
            )
        };
        let clip = self
            .panel
            .convertRectToScreen(self.scroll.convertRect_toView(self.scroll.bounds(), None));
        if clip.size.width > 2.0 && clip.size.height > 2.0 {
            NSPoint::new(
                target
                    .x
                    .clamp(clip.origin.x + 1.0, clip.origin.x + clip.size.width - 1.0),
                target
                    .y
                    .clamp(clip.origin.y + 1.0, clip.origin.y + clip.size.height - 1.0),
            )
        } else {
            target
        }
    }
    #[cfg(feature = "acceptance-hooks")]
    pub fn capture(&self) {
        let Some(path) = std::env::var_os("PETSONA_COMPOSER_SHOT") else {
            return;
        };
        if !self.open {
            return;
        }
        if let Some(bitmap) = self
            .content
            .bitmapImageRepForCachingDisplayInRect(self.content.bounds())
        {
            self.content
                .cacheDisplayInRect_toBitmapImageRep(self.content.bounds(), &bitmap);
            if let Some(data) = unsafe {
                bitmap.representationUsingType_properties(
                    NSBitmapImageFileType::PNG,
                    &objc2_foundation::NSDictionary::new(),
                )
            } {
                let _ = std::fs::write(path, data.to_vec());
            }
        }
    }
    #[cfg(feature = "acceptance-hooks")]
    pub fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({"appActive":NSApplication::sharedApplication(self.editor.mtm()).isActive(), "applicationKey":NSApplication::sharedApplication(self.editor.mtm()).keyWindow().map(|window|window.title().to_string()), "visible":self.panel.isVisible(), "key":self.panel.isKeyWindow(), "editorFirstResponder":self.panel.firstResponder().is_some_and(|responder| responder.isKindOfClass(ComposerEditor::class())), "height":self.height, "length":self.editor.string().length(), "caret":{"x":self.caret().x,"y":self.caret().y}})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ime_and_shift_enter_remain_editor_actions() {
        assert_eq!(key_action(36, false, false), KeyAction::Send);
        assert_eq!(key_action(76, false, false), KeyAction::Send);
        assert_eq!(key_action(36, true, false), KeyAction::Edit);
        assert_eq!(key_action(36, false, true), KeyAction::Edit);
        assert_eq!(key_action(53, false, true), KeyAction::Edit);
        assert_eq!(key_action(53, false, false), KeyAction::Close);
    }
}
