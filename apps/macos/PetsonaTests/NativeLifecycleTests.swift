import AppKit
import SwiftUI
import XCTest
@testable import Petsona

@MainActor
final class NativeLifecycleTests: XCTestCase {
    func testSettingsNavigationContractHasSixStableSections() {
        XCTAssertEqual(SettingsSection.allCases.map(\.rawValue), [
            "library", "behavior", "persona", "memory", "deepSeek", "startup",
        ])
        XCTAssertEqual(SettingsSection.allCases.map(\.title), [
            "宠物库", "外观与交互", "人格", "记忆", "模型服务", "系统",
        ])
    }

    func testSettingsLayoutUsesResponsiveContentBounds() {
        XCTAssertEqual(SettingsLayout.contentMaxWidth, 1000)
        XCTAssertEqual(SettingsLayout.sidebarMinWidth, 160)
        XCTAssertEqual(SettingsLayout.windowMinWidth, 720)
    }

    func testSettingsWindowUsesUnifiedTitlebarAndTracksCurrentPane() throws {
        let suiteName = "com.petsona.settings-window-tests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let navigation = SettingsNavigationState(defaults: defaults)
        let window = SettingsWindow(contentRect: NSRect(x: 0, y: 0, width: 960, height: 700),
                                   navigationState: navigation)
        defer { window.close() }

        XCTAssertTrue(window.styleMask.contains(.fullSizeContentView))
        XCTAssertTrue(window.titlebarAppearsTransparent)
        XCTAssertEqual(window.toolbarStyle, .unified)
        XCTAssertEqual(window.titlebarSeparatorStyle, .none)
        XCTAssertFalse(window.toolbar?.allowsUserCustomization ?? true)

        navigation.selection = .memory
        XCTAssertEqual(window.title, SettingsSection.memory.windowTitle)
    }

    func testSettingsNavigationRestoresPaneAndTogglesSidebar() throws {
        let suiteName = "com.petsona.settings-navigation-tests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let firstSession = SettingsNavigationState(defaults: defaults)
        firstSession.selection = .deepSeek
        XCTAssertEqual(defaults.string(forKey: SettingsNavigationState.lastSectionDefaultsKey), "deepSeek")

        let restoredSession = SettingsNavigationState(defaults: defaults)
        XCTAssertEqual(restoredSession.selection, .deepSeek)
        XCTAssertEqual(restoredSession.columnVisibility, .all)
        restoredSession.toggleSidebar()
        XCTAssertEqual(restoredSession.columnVisibility, .detailOnly)
        restoredSession.toggleSidebar()
        XCTAssertEqual(restoredSession.columnVisibility, .all)
    }

    func testAcceptanceLaunchOpensSettingsWithOrWithoutImportedPets() {
        XCTAssertTrue(SettingsLaunchPolicy.shouldPresentSettings(
            acceptanceLaunchRequested: true,
            hasPet: true
        ))
        XCTAssertTrue(SettingsLaunchPolicy.shouldPresentSettings(
            acceptanceLaunchRequested: true,
            hasPet: false
        ))
        XCTAssertTrue(SettingsLaunchPolicy.shouldPresentSettings(
            acceptanceLaunchRequested: false,
            hasPet: false
        ))
        XCTAssertFalse(SettingsLaunchPolicy.shouldPresentSettings(
            acceptanceLaunchRequested: false,
            hasPet: true
        ))
    }

    func testBubbleTimingParsesAndClampsProgress() {
        XCTAssertEqual(BubbleTiming.parse("250,1000,7"),
                       BubbleTiming(remainingMilliseconds: 250,
                                    totalMilliseconds: 1000,
                                    generation: 7))
        XCTAssertEqual(BubbleTiming.parse("-5,1000,8")?.progress, 0)
        XCTAssertEqual(BubbleTiming.parse("1500,1000,9")?.progress, 1)
        XCTAssertNil(BubbleTiming.parse("not-a-timing"))
    }

    func testOverlayLayoutUsesSidesWhenBelowSpaceIsInsufficient() {
        let work = NSRect(x: 0, y: 0, width: 1440, height: 900)
        let centered = NSRect(x: 600, y: 700, width: 96, height: 96)
        XCTAssertEqual(MacOverlayLayout.chooseSide(pet: centered, work: work), .bottom)

        let nearBottom = NSRect(x: 600, y: 20, width: 96, height: 96)
        XCTAssertEqual(MacOverlayLayout.chooseSide(pet: nearBottom, work: work), .right)

        let composer = MacOverlayLayout.positionComposer(pet: nearBottom,
                                                         work: work,
                                                         side: .right)
        XCTAssertTrue(composer.minX >= nearBottom.maxX)
        XCTAssertTrue(work.contains(composer))

        let narrowWork = NSRect(x: 0, y: 0, width: 720, height: 600)
        let centeredWidePet = NSRect(x: 260, y: 20, width: 200, height: 200)
        XCTAssertEqual(MacOverlayLayout.chooseSide(pet: centeredWidePet, work: narrowWork), .right)
        let narrowComposer = MacOverlayLayout.positionComposer(pet: centeredWidePet,
                                                                work: narrowWork,
                                                                side: .right)
        XCTAssertEqual(narrowComposer.width, 240)
        XCTAssertGreaterThanOrEqual(narrowComposer.minX,
                                    centeredWidePet.maxX + MacOverlayLayout.gap)
        XCTAssertTrue(narrowWork.contains(narrowComposer))
    }

    func testEditStripLayoutExpandsWithoutLeavingWorkArea() {
        let work = NSRect(x: 0, y: 0, width: 1440, height: 900)
        let pet = NSRect(x: 600, y: 400, width: 96, height: 96)
        let compact = MacOverlayLayout.positionStrip(pet: pet,
                                                     work: work,
                                                     side: .bottom,
                                                     expansion: 0)
        let expanded = MacOverlayLayout.positionStrip(pet: pet,
                                                      work: work,
                                                      side: .bottom,
                                                      expansion: 1)
        XCTAssertLessThan(compact.width, expanded.width)
        XCTAssertTrue(work.contains(expanded))
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
        for generation in 0..<20 {
            let bubble = BubblePanel()
            let pet = NSRect(x: 100, y: 100, width: 96, height: 96)
            let work = NSRect(x: 0, y: 0, width: 1440, height: 900)
            bubble.update(text: "生命周期测试",
                          timing: BubbleTiming(remainingMilliseconds: 1_000,
                                               totalMilliseconds: 5_000,
                                               generation: Int64(generation)),
                          petFrame: pet,
                          workFrame: work)
            bubble.orderOut(nil)

            let composer = ComposerPanel()
            composer.closePreservingDraft()
        }
    }

    func testBubbleFadeProgressSurvivesFrequentPanelUpdates() {
        let bubble = BubblePanel()
        let pet = NSRect(x: 100, y: 100, width: 96, height: 96)
        let work = NSRect(x: 0, y: 0, width: 1440, height: 900)
        let timing = BubbleTiming(remainingMilliseconds: 0,
                                  totalMilliseconds: 0,
                                  generation: 51)
        let start = 100.0

        bubble.update(text: "淡入测试", timing: timing, petFrame: pet, workFrame: work, now: start)
        XCTAssertEqual(bubble.alphaValue, 0, accuracy: 0.001)
        XCTAssertTrue(bubble.needsRefresh)

        bubble.update(text: "淡入测试", timing: timing, petFrame: pet, workFrame: work, now: start + 0.075)
        XCTAssertEqual(bubble.alphaValue, 0.5, accuracy: 0.02)
        XCTAssertTrue(bubble.needsRefresh)

        bubble.update(text: "淡入测试", timing: timing, petFrame: pet, workFrame: work, now: start + 0.15)
        XCTAssertEqual(bubble.alphaValue, 1, accuracy: 0.001)
        XCTAssertFalse(bubble.needsRefresh)
        bubble.orderOut(nil)
    }
}
