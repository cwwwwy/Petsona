import AppKit
import XCTest
@testable import Petsona

final class DisplayGeometryTests: XCTestCase {
    func testPhysicalPositionRestoresRelativeToNegativeOriginRetinaDisplay() {
        let display = NSRect(x: -1920, y: 0, width: 1920, height: 1080)
        let size = NSSize(width: 120, height: 100)
        let position = SavedWindowPosition(x: 240,
                                           y: 360,
                                           displayId: "retina",
                                           backingScale: 2)

        let origin = PetDisplayGeometry.restoredOrigin(position: position,
                                                      size: size,
                                                      screenFrame: display)

        XCTAssertEqual(origin.x, -1800, accuracy: 0.001)
        XCTAssertEqual(origin.y, 800, accuracy: 0.001)
    }

    func testSaveAndRestoreRoundTripsAcrossBackingScales() {
        let display = NSRect(x: 1440, y: -900, width: 1728, height: 1117)
        let frame = NSRect(x: 1500, y: -800, width: 240, height: 160)
        let saved = PetDisplayGeometry.savedPosition(frame: frame,
                                                     screenFrame: display,
                                                     displayId: "scaled",
                                                     backingScale: 2)
        let restored = PetDisplayGeometry.restoredOrigin(position: saved,
                                                         size: frame.size,
                                                         screenFrame: display)

        XCTAssertEqual(restored.x, frame.minX, accuracy: 0.001)
        XCTAssertEqual(restored.y, frame.minY, accuracy: 0.001)
        XCTAssertEqual(saved.displayId, "scaled")
    }

    func testSavedPositionUsesVisibleFrameClamp() {
        let proposed = NSRect(x: -100, y: 900, width: 240, height: 160)
        let visible = NSRect(x: 0, y: 0, width: 1200, height: 900)
        let clamped = PetDisplayGeometry.clampedFrame(proposed, visibleFrame: visible)

        XCTAssertEqual(clamped.minX, 0, accuracy: 0.001)
        XCTAssertEqual(clamped.maxY, visible.maxY, accuracy: 0.001)
    }

    func testLegacyPhysicalPositionRestoresOnItsDisplayAndClampsToVisibleFrame() {
        let primary = NSRect(x: 0, y: 0, width: 1440, height: 900)
        let secondary = NSRect(x: -1920, y: -200, width: 1920, height: 1080)
        let primaryVisible = NSRect(x: 0, y: 0, width: 1440, height: 860)
        let secondaryVisible = NSRect(x: -1920, y: -200, width: 1920, height: 1040)

        let restored = PetDisplayGeometry.restoredLegacyFrame(
            x: -1600,
            y: 1200,
            size: NSSize(width: 128, height: 128),
            backingScale: 2,
            screenFrames: [primary, secondary],
            visibleFrames: [primaryVisible, secondaryVisible])

        XCTAssertNotNil(restored)
        XCTAssertLessThan(restored!.midX, 0)
        XCTAssertGreaterThanOrEqual(restored!.minY, secondaryVisible.minY)
        XCTAssertLessThanOrEqual(restored!.maxY, secondaryVisible.maxY)
    }

}
