import Foundation

@MainActor
final class EngineClient: ObservableObject {
    @Published private(set) var snapshot = PetsonaSnapshot()
    @Published private(set) var errorMessage = ""

    func reportError(_ message: String) { errorMessage = message }

    // The engine is thread-confined to the main actor. Swift 6 treats an
    // opaque C pointer as non-Sendable, while `deinit` is nonisolated; the
    // unsafe annotation documents that the pointer is only touched here and
    // on the main actor.
    nonisolated(unsafe) private var handle: OpaquePointer?

    init(home: URL? = nil) {
        let pathBytes = home.map { Array($0.path.utf8) } ?? []
        let status: PetsonaStatus = pathBytes.withUnsafeBufferPointer { bytes in
            var options = PetsonaEngineOptions(
                abi_version: PETSONA_ABI_VERSION,
                home: PetsonaStringView(ptr: bytes.baseAddress, len: bytes.count)
            )
            return withUnsafePointer(to: &options) { pointer in
                petsona_engine_create(pointer, &handle)
            }
        }
        if status != PETSONA_OK {
            errorMessage = Self.lastError()
        }
    }

    deinit {
        if let handle {
            petsona_engine_destroy(handle)
        }
    }

    @discardableResult
    func tick() -> UInt32 {
        guard let handle else { return 0 }
        let status = petsona_engine_tick(handle)
        if status != PETSONA_OK && status != PETSONA_RUNTIME_FAILED {
            errorMessage = Self.lastError()
        }
        var next = PetsonaSnapshot()
        let snapshotStatus = petsona_engine_snapshot(handle, &next)
        snapshot = next
        if snapshotStatus != PETSONA_OK && snapshotStatus != PETSONA_RUNTIME_FAILED {
            errorMessage = Self.lastError()
            return 0
        }
        if next.faulted != 0 {
            errorMessage = text(PETSONA_TEXT_ERROR)
            return 1_000
        }
        if next.ready == 0 {
            return 50
        }
        return max(1, next.next_frame_ms)
    }

    func text(_ field: PetsonaTextField) -> String {
        guard let handle else { return "" }
        let required = petsona_engine_copy_text(handle, field.rawValue, nil, 0)
        guard required > 0 else { return "" }
        var bytes = [UInt8](repeating: 0, count: required)
        let copied = bytes.withUnsafeMutableBufferPointer { buffer in
            petsona_engine_copy_text(handle, field.rawValue, buffer.baseAddress, buffer.count)
        }
        return String(decoding: bytes.prefix(copied), as: UTF8.self)
    }

    func savedPosition() -> (CGFloat, CGFloat)? {
        let parts = text(PETSONA_TEXT_POSITION).split(separator: ",")
        guard parts.count == 2,
              let x = Double(parts[0]),
              let y = Double(parts[1]) else { return nil }
        return (CGFloat(x), CGFloat(y))
    }

    func send(kind: PetsonaCommandKind,
              value: Double = 0,
              ttlMilliseconds: UInt64 = 0,
              text: String = "") {
        guard let handle else { return }
        let data = text.data(using: .utf8) ?? Data()
        data.withUnsafeBytes { bytes in
            var command = PetsonaCommand(kind: kind.rawValue,
                                         reserved: 0,
                                         value: value,
                                         ttl_ms: ttlMilliseconds,
                                         text: PetsonaStringView(ptr: bytes.bindMemory(to: UInt8.self).baseAddress,
                                                                  len: bytes.count))
            let status = petsona_engine_command(handle, &command)
            if status != PETSONA_OK {
                errorMessage = Self.lastError()
            }
        }
    }

    func sendPosition(x: Double, y: Double) {
        send(kind: PETSONA_COMMAND_SET_POSITION,
             text: "\(x),\(y)")
    }

    func setGazeTarget(dx: CGFloat, dy: CGFloat) {
        send(kind: PETSONA_COMMAND_SET_GAZE_TARGET,
             text: "\(dx),\(dy)")
    }

    func clearGaze() {
        send(kind: PETSONA_COMMAND_CLEAR_GAZE)
    }

    func importPet(_ url: URL, overwrite: Bool = false) {
        send(kind: PETSONA_COMMAND_IMPORT_PET,
             value: overwrite ? 1 : 0,
             text: url.path)
    }

    func selectPet(_ id: String) {
        send(kind: PETSONA_COMMAND_SELECT_PET, text: id)
    }

    func deletePet(_ id: String) {
        send(kind: PETSONA_COMMAND_DELETE_PET, text: id)
    }

    func updatePersona(name: String,
                       tone: String,
                       language: String,
                       greeting: String,
                       systemPrompt: String) {
        let patch: [String: String] = [
            "name": name,
            "tone": tone,
            "language": language,
            "greeting": greeting,
            "system_prompt": systemPrompt,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: patch),
              let text = String(data: data, encoding: .utf8) else { return }
        send(kind: PETSONA_COMMAND_UPDATE_PERSONA, text: text)
        send(kind: PETSONA_COMMAND_SAVE_PERSONA)
    }

    private static func lastError() -> String {
        let required = petsona_last_error_copy(nil, 0)
        guard required > 0 else { return "Petsona engine error" }
        var bytes = [UInt8](repeating: 0, count: required)
        let copied = bytes.withUnsafeMutableBufferPointer { buffer in
            petsona_last_error_copy(buffer.baseAddress, buffer.count)
        }
        return String(decoding: bytes.prefix(copied), as: UTF8.self)
    }
}
