import AppKit
import QuartzCore

@MainActor
final class BubblePanel: NSPanel {
    private let bubbleView = BubbleView(frame: .zero)
    var onReply: (() -> Void)?
    private var hasShown = false

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
        orderOut(nil)
    }

    func update(text: String, petFrame: NSRect) {
        guard !text.isEmpty else {
            orderOut(nil)
            alphaValue = 1
            hasShown = false
            return
        }
        bubbleView.text = text
        let size = frameRect(for: text)
        setContentSize(size)
        let target = NSPoint(x: petFrame.midX - size.width / 2,
                             y: petFrame.maxY + 10)
        if !hasShown {
            hasShown = true
            alphaValue = 1
            setFrameOrigin(target)
            orderFrontRegardless()
        } else {
            setFrameOrigin(target)
            alphaValue = 1
            orderFrontRegardless()
        }
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
    var onReply: (() -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }

    required init?(coder: NSCoder) { nil }

    override func draw(_ dirtyRect: NSRect) {
        let bubble = bounds.insetBy(dx: 2, dy: 2)
        NSColor(calibratedWhite: 0.12, alpha: 0.96).setFill()
        NSBezierPath(roundedRect: bubble, xRadius: 12, yRadius: 12).fill()
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 14),
            .foregroundColor: NSColor.white,
        ]
        NSString(string: text).draw(in: NSRect(x: 14, y: 30, width: bubble.width - 28, height: bubble.height - 38),
                                     withAttributes: attributes)
        let tail = NSBezierPath()
        tail.move(to: NSPoint(x: bubble.midX - 8, y: bubble.minY + 1))
        tail.line(to: NSPoint(x: bubble.midX, y: bubble.minY - 8))
        tail.line(to: NSPoint(x: bubble.midX + 8, y: bubble.minY + 1))
        tail.close()
        tail.fill()
    }

    override func mouseUp(with event: NSEvent) { onReply?() }
}

@MainActor
final class EditButtonPanel: NSPanel {
    var onOpen: (() -> Void)?
    private let button = NSButton(title: "✎", target: nil, action: nil)

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 40, height: 40),
                   styleMask: [.borderless, .nonactivatingPanel],
                   backing: .buffered,
                   defer: false)
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        level = .floating
        collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary]
        button.bezelStyle = .texturedRounded
        button.font = .systemFont(ofSize: 17, weight: .medium)
        button.target = self
        button.action = #selector(open)
        button.frame = NSRect(x: 3, y: 3, width: 34, height: 34)
        contentView = NSView(frame: frame)
        contentView?.addSubview(button)
        orderOut(nil)
    }

    func update(petFrame: NSRect, visible: Bool) {
        guard visible else { orderOut(nil); return }
        setFrameOrigin(NSPoint(x: petFrame.midX - 20, y: max(8, petFrame.minY - 56)))
        orderFrontRegardless()
    }

    @objc private func open() { onOpen?() }
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

    func show(near petFrame: NSRect, draft: String) {
        if textView.string != draft { textView.string = draft }
        let size = NSSize(width: 320, height: 46)
        setContentSize(size)
        content.frame = NSRect(origin: .zero, size: size)
        layoutContent()
        setFrameOrigin(NSPoint(x: petFrame.midX - size.width / 2,
                               y: max(12, petFrame.minY - size.height - 22)))
        makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        makeFirstResponder(textView)
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
