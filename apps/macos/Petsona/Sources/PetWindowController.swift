import AppKit
import QuartzCore

@MainActor
final class PetWindowController: NSObject {
    private let engine: EngineClient
    private let window: NSPanel
    private let view: PetView
    private var hasAppliedInitialSize = false
    private var hasAppliedInitialPosition = false
    private var targetSize: NSSize?
    var onDoubleClick: (() -> Void)?
    var onRightClick: ((NSEvent, NSView) -> Void)?
    var onDragBegan: (() -> Void)?
    var onDragMoved: (() -> Void)?
    var onDragEnded: (() -> Void)?

    init(engine: EngineClient) {
        self.engine = engine
        self.view = PetView(engine: engine)
        self.window = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 220, height: 280),
                              styleMask: [.borderless, .nonactivatingPanel],
                              backing: .buffered,
                              defer: false)
        super.init()
        view.onDoubleClick = { [weak self] in self?.onDoubleClick?() }
        view.onRightClick = { [weak self] event, view in
            self?.onRightClick?(event, view)
        }
        view.onDragBegan = { [weak self] in self?.onDragBegan?() }
        view.onDragMoved = { [weak self] in self?.onDragMoved?() }
        view.onDragEnded = { [weak self] in self?.onDragEnded?() }
        window.contentView = view
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = false
        window.level = .floating
        window.collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary]
        window.hidesOnDeactivate = false
        window.isReleasedWhenClosed = false
        // Pixel-level pass-through is implemented by PetView.hitTest. The
        // whole NSWindow must remain event-capable so opaque sprite pixels can
        // still receive clicks and drags.
        window.ignoresMouseEvents = false
        window.acceptsMouseMovedEvents = true
        window.alphaValue = 0
        window.orderFrontRegardless()
    }

    func update() {
        let snapshot = engine.snapshot
        let width = CGFloat(max(snapshot.cell_width, 1)) * CGFloat(max(snapshot.scale, 0.1))
        let height = CGFloat(max(snapshot.cell_height, 1)) * CGFloat(max(snapshot.scale, 0.1))
        let requestedSize = NSSize(width: width, height: height)
        if !hasAppliedInitialSize {
            window.setContentSize(requestedSize)
            hasAppliedInitialSize = true
            targetSize = requestedSize
        } else if targetSize != requestedSize {
            targetSize = requestedSize
            let currentFrame = window.frame
            let targetFrame = NSRect(x: currentFrame.midX - requestedSize.width * 0.5,
                                     y: currentFrame.minY,
                                     width: requestedSize.width,
                                     height: requestedSize.height)
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.18
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                window.animator().setFrame(targetFrame, display: true)
            }
        }
        if !hasAppliedInitialPosition, snapshot.ready != 0,
           let (x, y) = engine.savedPosition() {
            let scale = max(window.backingScaleFactor, 0.1)
            let maxY = NSScreen.screens.map(\.frame.maxY).max() ?? 900
            window.setFrameOrigin(NSPoint(x: x / scale,
                                          y: maxY - y / scale - height))
            hasAppliedInitialPosition = true
        }
        window.alphaValue = snapshot.ready != 0 && snapshot.pet_visible != 0 && snapshot.has_pet != 0 ? 1 : 0
        window.level = snapshot.always_on_top != 0 ? .floating : .normal
        view.clickThrough = snapshot.click_through != 0
        view.setNeedsDisplay(view.bounds)
    }

    func screenFrame() -> NSRect { window.frame }
}

@MainActor
private final class PetView: NSView {
    private let engine: EngineClient
    private var atlas: NSBitmapImageRep?
    private var atlasImage: NSImage?
    private var atlasPath = ""
    private var dragStart: NSPoint?
    private var windowStart: NSPoint?
    private var isDragging = false
    private var clickGeneration = 0
    private var lastDragState = ""
    private var lastDragStateAt = Date.distantPast
    var clickThrough = true
    var onDoubleClick: (() -> Void)?
    var onRightClick: ((NSEvent, NSView) -> Void)?
    var onDragBegan: (() -> Void)?
    var onDragMoved: (() -> Void)?
    var onDragEnded: (() -> Void)?

    init(engine: EngineClient) {
        self.engine = engine
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        addTrackingArea(NSTrackingArea(rect: .zero,
                                       options: [.activeAlways, .mouseMoved, .inVisibleRect],
                                       owner: self,
                                       userInfo: nil))
    }

    required init?(coder: NSCoder) { nil }

    override func draw(_ dirtyRect: NSRect) {
        guard engine.snapshot.has_pet != 0 else { return }
        loadAtlasIfNeeded()
        guard let atlas else { return }
        let snapshot = engine.snapshot
        let columns = max(1, Int(snapshot.atlas_width / max(1, snapshot.cell_width)))
        let column = Int(snapshot.sprite_index) % columns
        let row = Int(snapshot.sprite_index) / columns
        let cellWidth = Int(snapshot.cell_width)
        let cellHeight = Int(snapshot.cell_height)
        let sourceTop = row * cellHeight
        let source = NSRect(x: column * cellWidth,
                            y: atlas.pixelsHigh - sourceTop - cellHeight,
                            width: cellWidth,
                            height: cellHeight)
        guard let image = atlasImage else { return }
        image.draw(in: bounds, from: source, operation: .sourceOver, fraction: 1)
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard clickThrough else { return super.hitTest(point) }
        guard engine.snapshot.has_pet != 0, alphaAt(point: point, spriteIndex: engine.snapshot.sprite_index) else {
            return nil
        }
        return self
    }

    override func mouseDown(with event: NSEvent) {
        clickGeneration += 1
        let generation = clickGeneration
        dragStart = NSEvent.mouseLocation
        windowStart = window?.frame.origin
        isDragging = false

        if event.clickCount >= 2 {
            onDoubleClick?()
            return
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.32) { [weak self] in
            guard let self, self.clickGeneration == generation, !self.isDragging else { return }
            self.engine.send(kind: PETSONA_COMMAND_SET_STATE, text: "waving")
            self.engine.send(kind: PETSONA_COMMAND_SHOW_BUBBLE,
                             ttlMilliseconds: 5_000,
                             text: "你好，我在这里")
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        onRightClick?(event, self)
    }

    override func mouseDragged(with event: NSEvent) {
        guard let dragStart, let windowStart, let window else { return }
        let current = NSEvent.mouseLocation
        let delta = NSPoint(x: current.x - dragStart.x, y: current.y - dragStart.y)
        if hypot(delta.x, delta.y) > 4 {
            if !isDragging {
                isDragging = true
                clickGeneration += 1
                onDragBegan?()
            }
        }
        guard isDragging else { return }
        window.setFrameOrigin(NSPoint(x: windowStart.x + delta.x, y: windowStart.y + delta.y))
        onDragMoved?()
        let state = delta.x >= 0 ? "running-right" : "running-left"
        if state != lastDragState || Date().timeIntervalSince(lastDragStateAt) >= 0.08 {
            engine.send(kind: PETSONA_COMMAND_SET_STATE, ttlMilliseconds: 300, text: state)
            lastDragState = state
            lastDragStateAt = Date()
        }
    }

    override func mouseUp(with event: NSEvent) {
        let dragged = isDragging
        if dragged {
            engine.send(kind: PETSONA_COMMAND_SET_STATE, ttlMilliseconds: 1, text: "idle")
            onDragEnded?()
        }
        rememberPosition()
        defer {
            dragStart = nil
            windowStart = nil
            isDragging = false
            lastDragState = ""
        }
        if dragged || event.clickCount >= 2 { return }
    }

    private func rememberPosition() {
        guard let window, let screen = window.screen else { return }
        let scale = max(window.backingScaleFactor, 0.1)
        let maxY = NSScreen.screens.map(\.frame.maxY).max() ?? screen.frame.maxY
        engine.sendPosition(x: window.frame.minX * scale,
                            y: (maxY - window.frame.maxY) * scale)
    }

    private func loadAtlasIfNeeded() {
        let path = engine.text(PETSONA_TEXT_ATLAS_PATH)
        guard path != atlasPath else { return }
        atlasPath = path
        if let data = try? Data(contentsOf: URL(fileURLWithPath: path)) {
            atlas = NSBitmapImageRep(data: data)
            atlasImage = NSImage(size: NSSize(width: atlas?.pixelsWide ?? 1,
                                              height: atlas?.pixelsHigh ?? 1))
            if let atlas, let atlasImage { atlasImage.addRepresentation(atlas) }
        } else {
            atlas = nil
            atlasImage = nil
        }
    }

    private func alphaAt(point: NSPoint, spriteIndex: UInt32) -> Bool {
        loadAtlasIfNeeded()
        guard let atlas else { return false }
        let snapshot = engine.snapshot
        let cellWidth = Int(snapshot.cell_width)
        let cellHeight = Int(snapshot.cell_height)
        guard cellWidth > 0, cellHeight > 0,
              point.x >= 0, point.y >= 0,
              point.x < bounds.width, point.y < bounds.height else { return false }
        let localX = min(cellWidth - 1, max(0, Int(point.x / bounds.width * CGFloat(cellWidth))))
        let localY = min(cellHeight - 1, max(0, Int((bounds.height - point.y) / bounds.height * CGFloat(cellHeight))))
        let columns = max(1, Int(snapshot.atlas_width / snapshot.cell_width))
        let row = Int(spriteIndex) / columns
        let column = Int(spriteIndex) % columns
        if alpha(atlas: atlas, x: column * cellWidth + localX, y: atlas.pixelsHigh - (row + 1) * cellHeight + localY) {
            return true
        }
        // A gaze pose can make a pixel transparent. Keep the idle row's union
        // clickable so a turn cannot make the resting body untouchable.
        for idleColumn in 0..<columns {
            if alpha(atlas: atlas,
                    x: idleColumn * cellWidth + localX,
                    y: atlas.pixelsHigh - cellHeight + localY) {
                return true
            }
        }
        return false
    }

    private func alpha(atlas: NSBitmapImageRep, x: Int, y: Int) -> Bool {
        guard let color = atlas.colorAt(x: x, y: y) else { return false }
        return color.alphaComponent > 0.05
    }
}
