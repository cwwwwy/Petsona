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

    func testDisplayPositionExtensionUsesAppendedOrdinals() {
        XCTAssertEqual(PETSONA_TEXT_POSITION.rawValue, 10)
        XCTAssertEqual(PETSONA_TEXT_WINDOW_POSITION.rawValue, 19)
        XCTAssertEqual(PETSONA_TEXT_CONVERSATION.rawValue, 20)
        XCTAssertEqual(PETSONA_COMMAND_SET_POSITION.rawValue, 7)
        XCTAssertEqual(PETSONA_COMMAND_SET_WINDOW_POSITION.rawValue, 44)
        XCTAssertEqual(PETSONA_COMMAND_UPDATE_CONVERSATION_CONFIG.rawValue, 45)
        XCTAssertEqual(PETSONA_COMMAND_START_CONVERSATION.rawValue, 46)
        XCTAssertEqual(PETSONA_COMMAND_CANCEL_CONVERSATION.rawValue, 47)
        XCTAssertEqual(PETSONA_COMMAND_CLEAR_CONVERSATION_HISTORY.rawValue, 48)
        XCTAssertEqual(PETSONA_COMMAND_LOAD_EARLIER_CONVERSATION_HISTORY.rawValue, 49)
        XCTAssertEqual(PETSONA_COMMAND_REVIEW_MEMORY_CANDIDATE.rawValue, 50)
        XCTAssertEqual(PETSONA_COMMAND_PARSE_PERSONA_SOURCE.rawValue, 51)
        XCTAssertEqual(PETSONA_COMMAND_GENERATE_PERSONA_PROFILE.rawValue, 52)
        XCTAssertEqual(PETSONA_COMMAND_APPLY_PERSONA_DRAFT.rawValue, 53)
        XCTAssertEqual(PETSONA_COMMAND_CLEAR_PERSONA_DRAFT.rawValue, 54)
        XCTAssertEqual(PETSONA_COMMAND_PREVIEW_PERSONA_DRAFT.rawValue, 55)
        XCTAssertEqual(PETSONA_COMMAND_APPLY_IMPORTED_PERSONA.rawValue, 56)
    }
}
