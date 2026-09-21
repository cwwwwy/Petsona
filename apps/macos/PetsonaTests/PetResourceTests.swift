import AppKit
import XCTest

final class PetResourceTests: XCTestCase {
    func testV2WebpFixtureIsReadableByAppKit() throws {
        let testFile = URL(fileURLWithPath: #filePath)
        let repository = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let spritesheet = repository
            .appendingPathComponent("crates/petsona-core/testdata/v2-test-pet-webp/spritesheet.webp")
        let data = try Data(contentsOf: spritesheet)
        let image = NSBitmapImageRep(data: data)
        XCTAssertNotNil(image)
        XCTAssertGreaterThan(image?.pixelsWide ?? 0, 0)
        XCTAssertGreaterThan(image?.pixelsHigh ?? 0, 0)
    }
}
