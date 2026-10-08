import AppKit

struct SavedWindowPosition: Codable, Equatable {
    var x: Double
    var y: Double
    var displayId: String?
    var backingScale: Double
}

enum PetDisplayGeometry {
    static func identifier(for screen: NSScreen) -> String? {
        (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?
            .stringValue
    }

    static func screen(for saved: SavedWindowPosition?,
                      current: NSScreen?,
                      screens: [NSScreen]) -> NSScreen? {
        if let id = saved?.displayId,
           let match = screens.first(where: { identifier(for: $0) == id }) {
            return match
        }
        if let current, screens.contains(where: { $0 === current }) {
            return current
        }
        return NSScreen.main ?? screens.first
    }

    static func restoredFrame(position: SavedWindowPosition,
                              size: NSSize,
                              screen: NSScreen) -> NSRect {
        let origin = restoredOrigin(position: position, size: size, screenFrame: screen.frame)
        let proposed = NSRect(origin: origin, size: size)
        return clampedFrame(proposed, visibleFrame: screen.visibleFrame)
    }

    static func restoredLegacyFrame(x: CGFloat,
                                    y: CGFloat,
                                    size: NSSize,
                                    backingScale: CGFloat,
                                    screenFrames: [NSRect],
                                    visibleFrames: [NSRect]) -> NSRect? {
        guard !screenFrames.isEmpty, screenFrames.count == visibleFrames.count else { return nil }
        let globalTop = screenFrames.map(\.maxY).max() ?? 0
        let scale = max(backingScale, 0.1)
        let proposed = NSRect(x: x / scale,
                              y: globalTop - y / scale - size.height,
                              width: size.width,
                              height: size.height)
        let center = NSPoint(x: proposed.midX, y: proposed.midY)
        let index = screenFrames.firstIndex(where: { $0.contains(center) }) ?? 0
        return clampedFrame(proposed, visibleFrame: visibleFrames[index])
    }

    static func restoredOrigin(position: SavedWindowPosition,
                               size: NSSize,
                               screenFrame: NSRect) -> NSPoint {
        let scale = max(CGFloat(position.backingScale), 1)
        return NSPoint(x: screenFrame.minX + CGFloat(position.x) / scale,
                       y: screenFrame.maxY - CGFloat(position.y) / scale - size.height)
    }

    static func savedPosition(frame: NSRect,
                              screenFrame: NSRect,
                              displayId: String,
                              backingScale: CGFloat) -> SavedWindowPosition {
        let scale = max(backingScale, 1)
        return SavedWindowPosition(
            x: Double((frame.minX - screenFrame.minX) * scale),
            y: Double((screenFrame.maxY - frame.maxY) * scale),
            displayId: displayId,
            backingScale: Double(scale)
        )
    }

    static func clampedFrame(_ frame: NSRect, visibleFrame: NSRect) -> NSRect {
        MacOverlayLayout.clamp(frame, to: visibleFrame)
    }

    static func savedPosition(frame: NSRect, screen: NSScreen) -> SavedWindowPosition {
        savedPosition(frame: frame,
                      screenFrame: screen.frame,
                      displayId: identifier(for: screen) ?? "display:\(screen.localizedName)",
                      backingScale: screen.backingScaleFactor)
    }

    static func screen(for frame: NSRect,
                       current: NSScreen?,
                       screens: [NSScreen]) -> NSScreen? {
        let center = NSPoint(x: frame.midX, y: frame.midY)
        if let containing = screens.first(where: { $0.frame.contains(center) }) {
            return containing
        }
        return current ?? NSScreen.main ?? screens.first
    }
}
