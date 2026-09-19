import XCTest
@testable import Petsona

final class SystemServiceTests: XCTestCase {
    func testLaunchAgentCanBeCreatedAndRemovedInAnIsolatedDirectory() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("petsona-launch-agent-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        setenv("PETSONA_AUTOSTART_PLIST_DIR", directory.path, 1)
        defer {
            unsetenv("PETSONA_AUTOSTART_PLIST_DIR")
            try? FileManager.default.removeItem(at: directory)
        }

        try LaunchAgentService.setEnabled(true)
        let plist = directory.appendingPathComponent("com.petsona.desktop.plist")
        XCTAssertTrue(FileManager.default.fileExists(atPath: plist.path))
        let data = try Data(contentsOf: plist)
        let object = try PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any]
        XCTAssertEqual(object?["Label"] as? String, "com.petsona.desktop")
        XCTAssertEqual(object?["RunAtLoad"] as? Bool, true)

        try LaunchAgentService.setEnabled(false)
        XCTAssertFalse(FileManager.default.fileExists(atPath: plist.path))
    }
}
