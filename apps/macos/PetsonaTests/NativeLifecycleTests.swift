import AppKit
import XCTest
@testable import Petsona

@MainActor
final class NativeLifecycleTests: XCTestCase {
    func testSettingsNavigationContractHasSixStableSections() {
        XCTAssertEqual(SettingsSection.allCases.map(\.rawValue), [
            "library", "behavior", "deepSeek", "persona", "memory", "startup",
        ])
        XCTAssertEqual(SettingsSection.allCases.map(\.title), [
            "宠物库", "外观与交互", "DeepSeek", "人格", "记忆", "启动",
        ])
    }

    func testOverlayActivationContractMatchesFocusRules() {
        let bubble = BubblePanel()
        let composer = ComposerPanel()

        XCTAssertFalse(bubble.canBecomeKey)
        XCTAssertFalse(bubble.canBecomeMain)
        XCTAssertTrue(composer.canBecomeKey)
        XCTAssertTrue(composer.canBecomeMain)
    }

    func testOverlayPanelsCanBeCreatedAndClosedRepeatedly() {
        for _ in 0..<20 {
            let bubble = BubblePanel()
            bubble.update(text: "生命周期测试", petFrame: NSRect(x: 100, y: 100, width: 96, height: 96))
            bubble.orderOut(nil)

            let composer = ComposerPanel()
            composer.closePreservingDraft()
        }
    }
}
