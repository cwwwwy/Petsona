import AppKit
import QuartzCore

enum OverlaySide: Equatable {
    case bottom
    case left
    case right
}

struct BubbleTiming: Equatable {
    let remainingMilliseconds: Int64
    let totalMilliseconds: Int64
    let generation: Int64

    var progress: CGFloat {
        guard totalMilliseconds > 0 else { return 1 }
        return CGFloat(max(0, min(1, Double(remainingMilliseconds) / Double(totalMilliseconds))))
    }

    static func parse(_ value: String) -> BubbleTiming? {
        let parts = value.split(separator: ",", omittingEmptySubsequences: false)
        guard parts.count == 3,
              let remaining = Int64(parts[0]),
              let total = Int64(parts[1]),
              let generation = Int64(parts[2]) else {
            return nil
        }
        return BubbleTiming(
            remainingMilliseconds: max(0, remaining),
            totalMilliseconds: max(0, total),
            generation: generation
        )
    }
}

enum MacOverlayLayout {
    static let gap: CGFloat = 12
    static let edgeMargin: CGFloat = 8
    static let composerHeight: CGFloat = 40
    static let composerDesiredWidth: CGFloat = 330
    static let composerMinWidth: CGFloat = 280
    static let stripCompactLength: CGFloat = 23.5
    static let stripExpandedLength: CGFloat = 24
    static let stripCompactThickness: CGFloat = 6
    static let stripThickness: CGFloat = 24
    static let controlsTransition: TimeInterval = 0.2

    static func clamp(_ rect: NSRect, to work: NSRect) -> NSRect {
        let width = min(rect.width, work.width)
        let height = min(rect.height, work.height)
        guard width > 0, height > 0, work.width > 0, work.height > 0 else { return rect }
        let x = max(work.minX, min(rect.minX, work.maxX - width))
        let y = max(work.minY, min(rect.minY, work.maxY - height))
        return NSRect(x: x, y: y, width: width, height: height)
    }

    static func chooseSide(
        pet: NSRect,
        work: NSRect,
        current: OverlaySide? = nil,
        height: CGFloat = composerHeight
    ) -> OverlaySide {
        let below = pet.minY - work.minY - gap - edgeMargin
        let right = work.maxX - pet.maxX - gap - edgeMargin
        let left = pet.minX - work.minX - gap - edgeMargin
        let bottomFits = below >= height

        if current == .bottom {
            return bottomFits ? .bottom : (right >= left ? .right : .left)
        }

        if current == .left || current == .right {
            if below >= height + 16 {
                return .bottom
            }
            let currentSpace = current == .right ? right : left
            let otherSpace = current == .right ? left : right
            if currentSpace >= composerMinWidth || currentSpace >= otherSpace {
                return current!
            }
            return current == .right ? .left : .right
        }

        if bottomFits { return .bottom }
        return right >= left ? .right : .left
    }

    static func sidePanelWidth(pet: NSRect, work: NSRect, side: OverlaySide) -> CGFloat {
        let space = side == .right
            ? work.maxX - pet.maxX - gap - edgeMargin
            : pet.minX - work.minX - gap - edgeMargin
        return min(composerDesiredWidth, max(1, space))
    }

    static func positionComposer(
        pet: NSRect,
        work: NSRect,
        side: OverlaySide,
        height: CGFloat = composerHeight
    ) -> NSRect {
        let width = side == .bottom ? composerDesiredWidth : sidePanelWidth(pet: pet, work: work, side: side)
        let x: CGFloat
        switch side {
        case .left: x = pet.minX - width - gap
        case .right: x = pet.maxX + gap
        case .bottom: x = pet.midX - width / 2
        }
        let y = side == .bottom
            ? pet.minY - height - gap
            : pet.midY - height / 2
        return clamp(NSRect(x: x, y: y, width: width, height: height), to: work)
    }

    static func positionStrip(
        pet: NSRect,
        work: NSRect,
        side: OverlaySide,
        expansion: CGFloat
    ) -> NSRect {
        let length = stripCompactLength
            + ((stripExpandedLength - stripCompactLength) * max(0, min(1, expansion)))
        let thickness = stripCompactThickness
            + ((stripThickness - stripCompactThickness) * max(0, min(1, expansion)))
        let vertical = side == .left || side == .right
        let width = vertical ? thickness : length
        let height = vertical ? length : thickness
        let x: CGFloat
        switch side {
        case .left: x = pet.minX - width - edgeMargin
        case .right: x = pet.maxX + edgeMargin
        case .bottom: x = pet.midX - width / 2
        }
        let y = side == .bottom
            ? pet.minY - height - edgeMargin
            : pet.midY - height / 2
        return clamp(NSRect(x: x, y: y, width: width, height: height), to: work)
    }

    static func positionBubble(pet: NSRect, work: NSRect, size: NSSize) -> NSRect {
        let x = pet.midX - size.width / 2
        var y = pet.maxY + 10
        if y + size.height > work.maxY - edgeMargin {
            y = pet.minY - size.height - 10
        }
        return clamp(NSRect(x: x, y: y, width: size.width, height: size.height), to: work)
    }
}

/// macOS 26 native glass, matching the reference overlay's regular material.
@MainActor
private final class OverlayGlass: NSGlassEffectView {
    init(content: NSView, radius: CGFloat) {
        super.init(frame: .zero)
        style = .regular
        cornerRadius = radius
        contentView = content
        wantsLayer = true
    }

    required init?(coder: NSCoder) { nil }
}

@MainActor
final class BubblePanel: NSPanel {
    private let bubbleView = BubbleView(frame: .zero)
    private lazy var glass = OverlayGlass(content: bubbleView, radius: 15)
    var onReply: (() -> Void)?
    var onHoverChanged: ((Bool) -> Void)?
    private var hasShown = false
    private var generation: Int64 = -1
    private var fadeStartedAt = 0.0
    private(set) var needsRefresh = false

    var isHovered: Bool { bubbleView.isHovered }

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 280, height: 82),
                   styleMask: [.borderless, .nonactivatingPanel],
                   backing: .buffered,
                   defer: false)
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        level = .floating
        collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary]
        ignoresMouseEvents = false
        contentView = glass
        bubbleView.onReply = { [weak self] in self?.onReply?() }
        bubbleView.onHoverChanged = { [weak self] hovered in self?.onHoverChanged?(hovered) }
        orderOut(nil)
    }

    func update(text: String,
                timing: BubbleTiming?,
                petFrame: NSRect,
                workFrame: NSRect,
                now: TimeInterval = CACurrentMediaTime()) {
        guard !text.isEmpty else {
            orderOut(nil)
            alphaValue = 1
            hasShown = false
            generation = -1
            fadeStartedAt = 0
            needsRefresh = false
            return
        }

        let timing = timing ?? BubbleTiming(remainingMilliseconds: 0,
                                            totalMilliseconds: 0,
                                            generation: -1)
        bubbleView.text = text
        let size = frameRect(for: text)
        setContentSize(size)
        let target = MacOverlayLayout.positionBubble(pet: petFrame, work: workFrame, size: size)
        let isNewBubble = !hasShown || generation != timing.generation
        if isNewBubble {
            hasShown = true
            generation = timing.generation
            fadeStartedAt = now
            alphaValue = 0
            setFrameOrigin(target.origin)
            orderFrontRegardless()
        } else {
            setFrameOrigin(target.origin)
            orderFrontRegardless()
        }
        let fadeProgress = min(1, max(0, (now - fadeStartedAt) / 0.15))
        let easedFadeProgress = fadeProgress * fadeProgress * (3 - 2 * fadeProgress)
        alphaValue = easedFadeProgress
        needsRefresh = fadeProgress < 1 || (!bubbleView.isHovered && timing.totalMilliseconds > 0)
    }

    private func frameRect(for text: String) -> NSSize {
        let font = NSFont.systemFont(ofSize: 13)
        let width = min(220.0, max(80.0, NSString(string: text).size(withAttributes: [.font: font]).width + 22))
        let rect = NSString(string: text).boundingRect(with: NSSize(width: width - 22, height: 80),
                                                        options: [.usesLineFragmentOrigin, .usesFontLeading],
                                                        attributes: [.font: font])
        return NSSize(width: width, height: max(32, min(96, ceil(rect.height) + 16)))
    }
}

@MainActor
private final class BubbleView: NSView {
    var text = "" { didSet { needsDisplay = true } }
    var onReply: (() -> Void)?
    private(set) var isHovered = false

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }

    required init?(coder: NSCoder) { nil }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: .zero,
                                       options: [.activeAlways, .mouseEnteredAndExited, .inVisibleRect],
                                       owner: self,
                                       userInfo: nil))
    }

    override func draw(_ dirtyRect: NSRect) {
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 13),
            .foregroundColor: NSColor.labelColor,
            .paragraphStyle: paragraph,
        ]
        NSString(string: text).draw(with: bounds.insetBy(dx: 11, dy: 8),
                                     options: [.usesLineFragmentOrigin, .usesFontLeading],
                                     attributes: attributes)
    }

    override func mouseEntered(with event: NSEvent) {
        isHovered = true
        onHoverChanged?(true)
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
        onHoverChanged?(false)
    }

    override func mouseUp(with event: NSEvent) { onReply?() }

    var onHoverChanged: ((Bool) -> Void)?
}

@MainActor
final class EditButtonPanel: NSPanel {
    var onOpen: (() -> Void)?
    private let stripView = EditStripView(frame: .zero)
    private var targetExpansion: CGFloat = 0
    private var displayExpansion: CGFloat = 0
    private var animationFrom: CGFloat = 0
    private var animationStarted = 0.0
    private var lastFrame = NSRect.zero
    private lazy var glass = OverlayGlass(content: stripView, radius: 6.5)
    private var expanded = false
    private var lastNearAt = 0.0
    private(set) var needsRefresh = false

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 34, height: 34),
                   styleMask: [.borderless, .nonactivatingPanel],
                   backing: .buffered,
                   defer: false)
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        level = .floating
        collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary]
        contentView = glass
        stripView.onClick = { [weak self] in self?.onOpen?() }
        orderOut(nil)
    }

    func update(petFrame: NSRect, workFrame: NSRect, side: OverlaySide, now: TimeInterval) {
        let cursor = NSEvent.mouseLocation
        let dx = max(0, max(petFrame.minX - cursor.x, cursor.x - petFrame.maxX))
        let dy = max(0, max(petFrame.minY - cursor.y, cursor.y - petFrame.maxY))
        let near = hypot(dx, dy) <= (expanded ? 56 : 40) || frame.contains(cursor)
        if near { lastNearAt = now }
        expanded = near || (expanded && now - lastNearAt < 0.3)
        let target: CGFloat = expanded ? 1 : 0
        if target != targetExpansion {
            targetExpansion = target
            animationFrom = displayExpansion
            animationStarted = now
        }
        let progress = min(1, max(0, (now - animationStarted) / MacOverlayLayout.controlsTransition))
        displayExpansion = animationFrom + ((targetExpansion - animationFrom) * CGFloat(progress))
        if progress >= 1 { displayExpansion = targetExpansion }
        let targetFrame = MacOverlayLayout.positionStrip(pet: petFrame,
                                                         work: workFrame,
                                                         side: side,
                                                         expansion: displayExpansion)
        stripView.expansion = displayExpansion
        glass.cornerRadius = 6.5 + 5.5 * displayExpansion
        if targetFrame != lastFrame {
            setFrame(targetFrame, display: true)
            lastFrame = targetFrame
        }
        orderFrontRegardless()
        needsRefresh = progress < 1
    }

    func hide() {
        needsRefresh = false
        expanded = false
        displayExpansion = 0
        targetExpansion = 0
        stripView.expansion = 0
        orderOut(nil)
    }
}

@MainActor
private final class EditStripView: NSView {
    var expansion: CGFloat = 0 { didSet { needsDisplay = true } }
    var onClick: (() -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }

    required init?(coder: NSCoder) { nil }

    override func draw(_ dirtyRect: NSRect) {
        guard expansion > 0.1,
              let icon = NSImage(systemSymbolName: "square.and.pencil", accessibilityDescription: "聊天") else { return }
        icon.draw(in: NSRect(x: bounds.midX - 6, y: bounds.midY - 6, width: 12, height: 12),
                  from: .zero, operation: .sourceOver, fraction: expansion)
    }

    override func mouseUp(with event: NSEvent) { onClick?() }
}

@MainActor
private final class ComposerSendButton: NSButton {
    var generating = false { didSet { needsDisplay = true } }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.labelColor.withAlphaComponent(isEnabled ? 1 : 0.25).setFill()
        NSBezierPath(ovalIn: bounds.insetBy(dx: 1, dy: 1)).fill()
        let symbol = generating ? "stop.fill" : "arrow.up"
        guard let image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 13, weight: .semibold)) else { return }
        image.isTemplate = false
        let tinted = NSImage(size: image.size, flipped: false) { rect in
            image.draw(in: rect)
            NSColor.controlBackgroundColor.setFill()
            rect.fill(using: .sourceAtop)
            return true
        }
        tinted.draw(in: NSRect(x: bounds.midX - 7, y: bounds.midY - 7, width: 14, height: 14))
    }
}

@MainActor
final class ComposerPanel: NSPanel, NSTextViewDelegate {
    private let textView = NSTextView(frame: .zero)
    private let scrollView = NSScrollView(frame: .zero)
    private let sendButton = ComposerSendButton(title: "", target: nil, action: nil)
    private let placeholder = NSTextField(labelWithString: "和我聊聊…")
    private let content = NSView(frame: .zero)
    private lazy var glass = OverlayGlass(content: content, radius: 20)
    private var generating = false
    private var petFrame = NSRect.zero
    private var workFrame = NSRect.zero
    private var side = OverlaySide.bottom
    var onSend: ((String) -> Void)?
    var onCancel: (() -> Void)?
    var onClose: (() -> Void)?
    var onCaret: ((NSPoint) -> Void)?

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 330, height: 40),
                   styleMask: [.borderless],
                   backing: .buffered,
                   defer: false)
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        level = .floating
        collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary]
        contentView = glass

        textView.isRichText = false
        textView.allowsUndo = true
        textView.drawsBackground = false
        textView.textColor = .labelColor
        textView.insertionPointColor = .labelColor
        textView.font = .systemFont(ofSize: 14)
        textView.delegate = self
        textView.focusRingType = .none
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.minSize = .zero
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.textContainerInset = .zero
        textView.textContainer?.lineFragmentPadding = 0
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.containerSize = NSSize(width: 280, height: CGFloat.greatestFiniteMagnitude)
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.documentView = textView
        content.addSubview(scrollView)

        placeholder.font = .systemFont(ofSize: 14)
        placeholder.textColor = .secondaryLabelColor
        placeholder.isSelectable = false
        content.addSubview(placeholder)
        sendButton.isBordered = false
        sendButton.target = self
        sendButton.action = #selector(send)
        sendButton.setAccessibilityLabel("发送")
        content.addSubview(sendButton)
        isReleasedWhenClosed = false
        orderOut(nil)
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    private func layoutContent() {
        sendButton.frame = NSRect(x: content.bounds.maxX - 34, y: 6, width: 28, height: 28)
        let editorRect = NSRect(x: 14, y: 11, width: max(1, content.bounds.width - 56),
                                height: max(18, content.bounds.height - 22))
        scrollView.frame = editorRect
        textView.setFrameSize(NSSize(width: scrollView.contentSize.width,
                                     height: max(editorRect.height, textView.frame.height)))
        placeholder.frame = NSRect(x: 14, y: content.bounds.maxY - 29,
                                   width: editorRect.width, height: 18)
        placeholder.isHidden = !textView.string.isEmpty
        sendButton.isEnabled = generating || !textView.string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func contentHeight() -> CGFloat {
        guard let layoutManager = textView.layoutManager,
              let container = textView.textContainer else { return MacOverlayLayout.composerHeight }
        layoutManager.ensureLayout(for: container)
        let used = layoutManager.usedRect(for: container).height
        return max(MacOverlayLayout.composerHeight, min(80, ceil(used)) + 22)
    }

    func show(near petFrame: NSRect,
              workFrame: NSRect,
              side: OverlaySide,
              draft: String) {
        if textView.string != draft {
            textView.string = draft
            textView.undoManager?.removeAllActions()
            textView.setSelectedRange(NSRange(location: textView.string.utf16.count, length: 0))
        }
        updatePosition(near: petFrame, workFrame: workFrame, side: side)
        makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        makeFirstResponder(textView)
        if !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            let animation = CABasicAnimation(keyPath: "transform.scale.x")
            animation.fromValue = MacOverlayLayout.stripExpandedLength / frame.width
            animation.toValue = 1
            animation.duration = MacOverlayLayout.controlsTransition
            animation.timingFunction = CAMediaTimingFunction(name: .easeOut)
            glass.layer?.add(animation, forKey: "expand")
        }
    }

    func updatePosition(near petFrame: NSRect, workFrame: NSRect, side: OverlaySide) {
        self.petFrame = petFrame
        self.workFrame = workFrame
        self.side = side
        let width = side == .bottom ? MacOverlayLayout.composerDesiredWidth
            : MacOverlayLayout.sidePanelWidth(pet: petFrame, work: workFrame, side: side)
        if content.bounds.width != width {
            setContentSize(NSSize(width: width, height: MacOverlayLayout.composerHeight))
            layoutContent()
        }
        let target = MacOverlayLayout.positionComposer(pet: petFrame, work: workFrame,
                                                       side: side, height: contentHeight())
        if frame != target { setFrame(target, display: true) }
        layoutContent()
    }

    func draft() -> String { textView.string }

    var desiredHeight: CGFloat { contentHeight() }

    func setConversationState(_ inFlight: Bool, error: String) {
        generating = inFlight
        sendButton.generating = inFlight
        sendButton.action = inFlight ? #selector(cancelGeneration) : #selector(send)
        sendButton.setAccessibilityLabel(inFlight ? "停止回复" : "发送")
        sendButton.toolTip = error.isEmpty ? nil : error
        layoutContent()
    }

    func closePreservingDraft() {
        orderOut(nil)
        onClose?()
    }

    @objc private func send() {
        guard !generating, !textView.hasMarkedText() else { return }
        let text = textView.string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        onSend?(text)
        textView.string = ""
        textView.undoManager?.removeAllActions()
        updatePosition(near: petFrame, workFrame: workFrame, side: side)
    }

    @objc private func cancelGeneration() { onCancel?() }

    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        guard !textView.hasMarkedText() else { return false }
        if selector == #selector(insertNewline(_:)) {
            if NSApp.currentEvent?.modifierFlags.contains(.shift) == true { return false }
            send()
            return true
        }
        if selector == #selector(cancelOperation(_:)) {
            closePreservingDraft()
            return true
        }
        return false
    }

    override func cancelOperation(_ sender: Any?) { closePreservingDraft() }

    override func resignKey() {
        super.resignKey()
        if isVisible { closePreservingDraft() }
    }

    func textDidChange(_ notification: Notification) {
        updatePosition(near: petFrame, workFrame: workFrame, side: side)
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        let caret = textView.firstRect(forCharacterRange: textView.selectedRange(), actualRange: nil)
        guard caret != .zero else { return }
        onCaret?(NSPoint(x: caret.midX, y: caret.midY))
    }
}
