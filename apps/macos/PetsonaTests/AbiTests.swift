import XCTest
@testable import Petsona

final class AbiTests: XCTestCase {
    func testCLayoutMatchesTheRustContract() {
        XCTAssertEqual(MemoryLayout<PetsonaStringView>.size, 16)
        XCTAssertEqual(MemoryLayout<PetsonaEngineOptions>.size, 24)
        XCTAssertEqual(MemoryLayout<PetsonaCommand>.size, 40)
        XCTAssertEqual(MemoryLayout<PetsonaSnapshot>.size, 64)
        XCTAssertEqual(PETSONA_ABI_VERSION, 3)
    }
}
