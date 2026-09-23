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
    static let composerHeight: CGFloat = 46
    static let composerDesiredWidth: CGFloat = 320
    static let composerMinWidth: CGFloat = 280
    static let stripCompactLength: CGFloat = 34
    static let stripExpandedLength: CGFloat = 72
    static let stripThickness: CGFloat = 34

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
        current: OverlaySide? = nil
    ) -> OverlaySide {
        let below = pet.minY - work.minY - gap - edgeMargin
        let right = work.maxX - pet.maxX - gap - edgeMargin
        let left = pet.minX - work.minX - gap - edgeMargin
        let bottomFits = below >= composerHeight

        if current == .bottom {
            return bottomFits ? .bottom : (right >= left ? .right : .left)
        }

        if current == .left || current == .right {
            if below >= composerHeight + 16 {
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
        side: OverlaySide
    ) -> NSRect {
        let width = side == .bottom ? composerDesiredWidth : sidePanelWidth(pet: pet, work: work, side: side)
        let x: CGFloat
        switch side {
        case .left: x = pet.minX - width - gap
        case .right: x = pet.maxX + gap
        case .bottom: x = pet.midX - width / 2
        }
        let y = side == .bottom
            ? pet.minY - composerHeight - gap
            : pet.midY - composerHeight / 2
        return clamp(NSRect(x: x, y: y, width: width, height: composerHeight), to: work)
    }

    static func positionStrip(
        pet: NSRect,
        work: NSRect,
        side: OverlaySide,
        expansion: CGFloat
    ) -> NSRect {
        let length = stripCompactLength
            + ((stripExpandedLength - stripCompactLength) * max(0, min(1, expansion)))
        let vertical = side == .left || side == .right
        let width = vertical ? stripThickness : length
        let height = vertical ? length : stripThickness
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

@MainActor
final class BubblePanel: NSPanel {
    private let bubbleView = BubbleView(frame: .zero)
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
        contentView = bubbleView
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
        bubbleView.progress = timing.progress
        needsRefresh = fadeProgress < 1 || (!bubbleView.isHovered && timing.totalMilliseconds > 0)
    }

    private func frameRect(for text: String) -> NSSize {
        let font = NSFont.systemFont(ofSize: 14)
        let width = min(280.0, max(150.0, NSString(string: text).size(withAttributes: [.font: font]).width + 34))
        let rect = NSString(string: text).boundingRect(with: NSSize(width: width - 28, height: 120),
                                                        options: [.usesLineFragmentOrigin, .usesFontLeading],
                                                        attributes: [.font: font])
        return NSSize(width: width, height: max(64, min(120, rect.height + 42)))
    }
}

@MainActor
private final class BubbleView: NSView {
    var text = "" { didSet { needsDisplay = true } }
    var progress: CGFloat = 1 { didSet { needsDisplay = true } }
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
        let bubble = bounds.insetBy(dx: 2, dy: 2)
        NSColor(calibratedWhite: 0.12, alpha: 0.96).setFill()
        NSBezierPath(roundedRect: bubble, xRadius: 12, yRadius: 12).fill()
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 14),
            .foregroundColor: NSColor.white,
        ]
        NSString(string: text).draw(in: NSRect(x: 14, y: 19, width: bubble.width - 28, height: bubble.height - 35),
                                     withAttributes: attributes)
        let bar = NSRect(x: 14, y: 7, width: bubble.width - 28, height: 3)
        NSColor.white.withAlphaComponent(0.18).setFill()
        NSBezierPath(roundedRect: bar, xRadius: 1.5, yRadius: 1.5).fill()
        let filled = NSRect(x: bar.minX, y: bar.minY, width: bar.width * progress, height: bar.height)
        NSColor.controlAccentColor.setFill()
        NSBezierPath(roundedRect: filled, xRadius: 1.5, yRadius: 1.5).fill()
        let tail = NSBezierPath()
        tail.move(to: NSPoint(x: bubble.midX - 8, y: bubble.minY + 1))
        tail.line(to: NSPoint(x: bubble.midX, y: bubble.minY - 8))
        tail.line(to: NSPoint(x: bubble.midX + 8, y: bubble.minY + 1))
        tail.close()
        tail.fill()
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
    private(set) var needsRefresh = false

    var isHovered: Bool { stripView.isHovered }

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
        contentView = stripView
        stripView.onClick = { [weak self] in self?.onOpen?() }
        orderOut(nil)
    }

    func update(petFrame: NSRect, workFrame: NSRect, side: OverlaySide, now: TimeInterval) {
        let target: CGFloat = stripView.isHovered ? 1 : 0
        if target != targetExpansion {
            targetExpansion = target
            animationFrom = displayExpansion
            animationStarted = now
        }
        let progress = min(1, max(0, (now - animationStarted) / 0.22))
        displayExpansion = animationFrom + ((targetExpansion - animationFrom) * CGFloat(progress))
        if progress >= 1 { displayExpansion = targetExpansion }
        let targetFrame = MacOverlayLayout.positionStrip(pet: petFrame,
                                                         work: workFrame,
                                                         side: side,
                                                         expansion: displayExpansion)
        let orientation = side == .left || side == .right
        stripView.orientation = orientation
        stripView.expansion = displayExpansion
        if targetFrame != lastFrame {
            setFrame(targetFrame, display: true)
            lastFrame = targetFrame
        }
        orderFrontRegardless()
        needsRefresh = progress < 1
    }

    func hide() {
        needsRefresh = false
        orderOut(nil)
    }
}

@MainActor
private final class EditStripView: NSView {
    var orientation = false { didSet { needsDisplay = true } }
    var expansion: CGFloat = 0 { didSet { needsDisplay = true } }
    var onClick: (() -> Void)?
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
        let length = MacOverlayLayout.stripCompactLength
            + ((MacOverlayLayout.stripExpandedLength - MacOverlayLayout.stripCompactLength) * expansion)
        let rect: NSRect
        if orientation {
            rect = NSRect(x: (bounds.width - MacOverlayLayout.stripThickness) / 2,
                          y: (bounds.height - length) / 2,
                          width: MacOverlayLayout.stripThickness,
                          height: length)
        } else {
            rect = NSRect(x: (bounds.width - length) / 2,
                          y: (bounds.height - MacOverlayLayout.stripThickness) / 2,
                          width: length,
                          height: MacOverlayLayout.stripThickness)
        }
        NSColor.labelColor.withAlphaComponent(0.28 + (0.36 * expansion)).setFill()
        NSBezierPath(roundedRect: rect, xRadius: min(rect.width, rect.height) / 2,
                     yRadius: min(rect.width, rect.height) / 2).fill()

        let icon = NSString(string: "✎")
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 16, weight: .medium),
            .foregroundColor: NSColor.labelColor,
        ]
        let iconSize = icon.size(withAttributes: attributes)
        icon.draw(at: NSPoint(x: bounds.midX - iconSize.width / 2,
                              y: bounds.midY - iconSize.height / 2),
                  withAttributes: attributes)
    }

    override func mouseEntered(with event: NSEvent) {
        isHovered = true
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
    }

    override func mouseUp(with event: NSEvent) { onClick?() }
}

@MainActor
final class ComposerPanel: NSPanel, NSTextViewDelegate {
    private let textView = NSTextView(frame: .zero)
    private let sendButton = NSButton(title: "↑", target: nil, action: nil)
    private let content = NSView(frame: .zero)
    var onSend: ((String) -> Void)?
    var onClose: (() -> Void)?
    var onCaret: ((NSPoint) -> Void)?

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 320, height: 46),
                   styleMask: [.borderless],
                   backing: .buffered,
                   defer: false)
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        level = .floating
        contentView = content
        content.wantsLayer = true
        content.layer?.backgroundColor = NSColor(calibratedWhite: 0.12, alpha: 0.98).cgColor
        content.layer?.cornerRadius = 17

        textView.isRichText = false
        textView.drawsBackground = false
        textView.textColor = .white
        textView.font = .systemFont(ofSize: 14)
        textView.delegate = self
        textView.focusRingType = .none
        textView.isVerticallyResizable = false
        textView.isHorizontallyResizable = false
        textView.textContainerInset = NSSize(width: 8, height: 7)
        content.addSubview(textView)

        sendButton.bezelStyle = .texturedRounded
        sendButton.target = self
        sendButton.action = #selector(send)
        content.addSubview(sendButton)
        isReleasedWhenClosed = false
        orderOut(nil)
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    private func layoutContent() {
        let buttonWidth: CGFloat = 34
        sendButton.frame = NSRect(x: content.bounds.maxX - buttonWidth - 6,
                                  y: 6,
                                  width: buttonWidth,
                                  height: 34)
        textView.frame = NSRect(x: 6,
                                y: 6,
                                width: content.bounds.width - buttonWidth - 18,
                                height: 34)
    }

    func show(near petFrame: NSRect,
              workFrame: NSRect,
              side: OverlaySide,
              draft: String) {
        if textView.string != draft { textView.string = draft }
        updatePosition(near: petFrame, workFrame: workFrame, side: side)
        makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        makeFirstResponder(textView)
    }

    func updatePosition(near petFrame: NSRect, workFrame: NSRect, side: OverlaySide) {
        let frame = MacOverlayLayout.positionComposer(pet: petFrame, work: workFrame, side: side)
        let size = frame.size
        if content.frame.size != size {
            setContentSize(size)
            content.frame = NSRect(origin: .zero, size: size)
            layoutContent()
        }
        if self.frame != frame {
            setFrame(frame, display: true)
        }
    }

    func draft() -> String { textView.string }

    func closePreservingDraft() {
        orderOut(nil)
        onClose?()
    }

    @objc private func send() {
        let text = textView.string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        onSend?(text)
        textView.string = ""
    }

    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if selector == #selector(insertNewline(_:)) {
            if NSApp.currentEvent?.modifierFlags.contains(.shift) == true {
                return false
            }
            send()
            return true
        }
        return false
    }

    override func cancelOperation(_ sender: Any?) {
        closePreservingDraft()
    }

    override func resignKey() {
        super.resignKey()
        if isVisible { closePreservingDraft() }
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        let range = textView.selectedRange()
        guard let layoutManager = textView.layoutManager,
              let container = textView.textContainer else { return }
        let glyph = layoutManager.glyphRange(forCharacterRange: range, actualCharacterRange: nil)
        let rect = layoutManager.boundingRect(forGlyphRange: glyph, in: container)
        let windowPoint = textView.convert(NSPoint(x: rect.minX, y: rect.minY), to: nil)
        let screenPoint = convertToScreen(windowPoint)
        onCaret?(screenPoint)
    }

    private func convertToScreen(_ point: NSPoint) -> NSPoint {
        convertPoint(toScreen: point)
    }
}
