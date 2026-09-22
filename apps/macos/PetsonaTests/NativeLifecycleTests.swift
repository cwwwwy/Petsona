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
            "宠物库", "外观与交互", "模型服务", "人格", "记忆", "系统",
        ])
    }

    func testCredentialStatusLabelCoversConfiguredAndMissingKeys() throws {
        let configured = try JSONDecoder().decode(
            DeepSeekProjection.self,
            from: Data(#"{"provider":"deepseek","keyConfigured":true,"baseUrl":"https://example.invalid","model":"test","apiKeyEnv":"TEST_KEY","timeoutSeconds":20,"maxTokens":80,"temperature":0.9,"thinkingDisabled":true}"#.utf8)
        )
        let missing = try JSONDecoder().decode(
            DeepSeekProjection.self,
            from: Data(#"{"provider":"deepseek","keyConfigured":false,"baseUrl":"https://example.invalid","model":"test","apiKeyEnv":"TEST_KEY","timeoutSeconds":20,"maxTokens":80,"temperature":0.9,"thinkingDisabled":true}"#.utf8)
        )

        XCTAssertEqual(configured.credentialStatusLabel, "已配置（密钥不会显示）")
        XCTAssertEqual(missing.credentialStatusLabel, "未配置")
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
