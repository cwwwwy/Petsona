import AppKit
import XCTest
@testable import Petsona

@MainActor
final class GazeStabilizerTests: XCTestCase {
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
