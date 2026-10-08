import AppKit
import XCTest
@testable import Petsona

@MainActor
final class GazeStabilizerTests: XCTestCase {
    func testRetinaPetAcceptsCursorOnItsBodyAndFlipsAppKitY() throws {
        // A 256px cell on a 2x screen occupies 128 screen points. The old
        // pixel-derived 89.6pt dead zone suppressed this 20pt cursor vector.
        let frame = NSRect(x: 100, y: 100, width: 128, height: 128)
        let target = try XCTUnwrap(PetGazeGeometry.target(
            cursor: NSPoint(x: frame.midX + 20, y: frame.midY + 10),
            petFrame: frame, active: false))
        XCTAssertEqual(target, NSPoint(x: 20, y: -10))
        XCTAssertNil(PetGazeGeometry.target(cursor: NSPoint(x: frame.midX, y: frame.midY),
                                           petFrame: frame, active: false))
    }

    func testGazeRegionUsesRenderedSizeAndReleaseHysteresis() {
        let frame = NSRect(x: 100, y: 100, width: 100, height: 100)
        let edge = NSPoint(x: frame.midX + 80, y: frame.midY)
        XCTAssertNil(PetGazeGeometry.target(cursor: edge, petFrame: frame, active: false))
        XCTAssertNotNil(PetGazeGeometry.target(cursor: edge, petFrame: frame, active: true))
        XCTAssertNil(PetGazeGeometry.target(cursor: NSPoint(x: 500, y: 500),
                                           petFrame: frame, active: true))
    }

    func testOfficialDirectionMappingAndUnitVectorRoundTrip() {
        XCTAssertEqual(GazeStabilizer.quantize(dx: 0, dy: -1), 0)
        XCTAssertEqual(GazeStabilizer.quantize(dx: 1, dy: 0), 4)
        XCTAssertEqual(GazeStabilizer.quantize(dx: 0, dy: 1), 8)
        XCTAssertEqual(GazeStabilizer.quantize(dx: -1, dy: 0), 12)

        for direction in 0..<16 {
            let vector = GazeStabilizer.unitVector(direction: direction)
            XCTAssertEqual(GazeStabilizer.quantize(dx: vector.x, dy: vector.y), direction)
        }
    }

    func testHysteresisAndMinimumMovementHoldDirection() {
        let stabilizer = GazeStabilizer()
        XCTAssertEqual(stabilizer.update(dx: 0, dy: -100), 0)
        XCTAssertEqual(stabilizer.update(dx: 1, dy: -100), 0)
        XCTAssertEqual(stabilizer.update(dx: 25, dy: -100), 0)
        XCTAssertEqual(stabilizer.update(dx: 100, dy: 0), 4)
        stabilizer.reset()
        XCTAssertEqual(stabilizer.direction, -1)
    }
}
