import XCTest
@testable import Petsona

@MainActor
final class EngineClientTests: XCTestCase {
    func testEmptyHomeIsIsolatedAndBecomesReadyWithoutAPet() {
        let home = makeIsolatedHome()
        let client = EngineClient(home: home)
        waitUntilReady(client)

        XCTAssertEqual(client.snapshot.abi_version, PETSONA_ABI_VERSION)
        XCTAssertNotEqual(client.snapshot.ready, 0)
        XCTAssertEqual(client.snapshot.has_pet, 0)
        XCTAssertEqual(client.snapshot.faulted, 0)
        XCTAssertTrue(FileManager.default.fileExists(atPath: home.appendingPathComponent("config.json").path))
    }

    func testCommandsRoundTripThroughTheWorkerProjection() {
        let client = EngineClient(home: makeIsolatedHome())
        waitUntilReady(client)

        client.send(kind: PETSONA_COMMAND_SET_SCALE, value: 1.5)
        client.send(kind: PETSONA_COMMAND_SHOW_BUBBLE,
                    ttlMilliseconds: 5_000,
                    text: "隔离测试")
        for _ in 0..<20 {
            _ = client.tick()
            if client.text(PETSONA_TEXT_BUBBLE) == "隔离测试" { break }
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        }
        XCTAssertEqual(client.snapshot.scale, 1.5, accuracy: 0.001)
        XCTAssertEqual(client.text(PETSONA_TEXT_BUBBLE), "隔离测试")

        client.send(kind: PETSONA_COMMAND_CLEAR_BUBBLE)
        for _ in 0..<20 {
            _ = client.tick()
            if client.text(PETSONA_TEXT_BUBBLE).isEmpty { break }
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        }
        XCTAssertEqual(client.text(PETSONA_TEXT_BUBBLE), "")
    }

    func testSettingsMemoryAndPersonaCommandsRoundTrip() {
        let client = EngineClient(home: makeIsolatedHome())
        waitUntilReady(client)

        client.updateDeepSeekConfig([
            "baseUrl": "https://example.invalid/v1",
            "model": "test-model",
            "apiKeyEnv": "TEST_DEEPSEEK_KEY",
            "timeoutSeconds": 9,
            "maxTokens": 64,
            "temperature": 0.4,
            "thinkingDisabled": true,
        ])
        client.updateMemoryConfig([
            "enabled": true,
            "recentEvents": 7,
            "retentionDays": 30,
            "factLimit": 12,
        ])
        client.createPersona(id: "native-test", name: "原生测试", template: nil)

        for _ in 0..<40 {
            _ = client.tick()
            if client.text(PETSONA_TEXT_PERSONA_ID) == "native-test" { break }
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        }
        client.rememberFact(key: "喜欢", value: "安静音乐")
        for _ in 0..<40 {
            _ = client.tick()
            if client.text(PETSONA_TEXT_MEMORY).contains("安静音乐") { break }
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        }

        XCTAssertEqual(client.text(PETSONA_TEXT_PERSONA_ID), "native-test")
        XCTAssertTrue(client.text(PETSONA_TEXT_PERSONAS).contains("原生测试"))
        XCTAssertTrue(client.text(PETSONA_TEXT_DEEPSEEK_CONFIG).contains("test-model"))
        XCTAssertTrue(client.text(PETSONA_TEXT_MEMORY).contains("安静音乐"))
    }

    private func waitUntilReady(_ client: EngineClient) {
        for _ in 0..<100 {
            _ = client.tick()
            if client.snapshot.ready != 0 { return }
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        }
        XCTFail("isolated runtime did not become ready")
    }

    private func makeIsolatedHome() -> URL {
        let home = FileManager.default.temporaryDirectory
            .appendingPathComponent("petsona-native-test-\(UUID().uuidString)", isDirectory: true)
        try! FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        let config = "{\"stateServer\":{\"enabled\":false}}"
        try! config.data(using: .utf8)!.write(to: home.appendingPathComponent("config.json"), options: .atomic)
        return home
    }
}
