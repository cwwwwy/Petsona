import Foundation

@MainActor
final class EngineClient: ObservableObject {
    @Published private(set) var snapshot = PetsonaSnapshot()
    @Published private(set) var errorMessage = ""
    @Published private(set) var statusMessage = ""

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
        let statusText = text(PETSONA_TEXT_STATUS)
        if statusMessage != statusText { statusMessage = statusText }
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

    func clearImportConflict() {
        send(kind: PETSONA_COMMAND_CLEAR_IMPORT_CONFLICT)
    }

    func selectPet(_ id: String) {
        send(kind: PETSONA_COMMAND_SELECT_PET, text: id)
    }

    func deletePet(_ id: String) {
        send(kind: PETSONA_COMMAND_DELETE_PET, text: id)
    }

    func updatePersona(fields: [String: Any]) {
        sendJSONObject(kind: PETSONA_COMMAND_UPDATE_PERSONA, object: fields)
        send(kind: PETSONA_COMMAND_SAVE_PERSONA)
    }

    func refreshPersonas() {
        send(kind: PETSONA_COMMAND_REFRESH_PERSONAS)
    }

    func createPersona(id: String, name: String, template: String?) {
        var object: [String: Any] = ["id": id, "name": name]
        if let template { object["template"] = template }
        sendJSONObject(kind: PETSONA_COMMAND_CREATE_PERSONA, object: object)
    }

    func duplicatePersona(sourceID: String, id: String, name: String) {
        sendJSONObject(kind: PETSONA_COMMAND_DUPLICATE_PERSONA,
                       object: ["source_id": sourceID, "id": id, "name": name])
    }

    func selectPersona(_ id: String) {
        send(kind: PETSONA_COMMAND_SELECT_PERSONA, text: id)
    }

    func deletePersona(_ id: String) {
        send(kind: PETSONA_COMMAND_DELETE_PERSONA, text: id)
    }

    func importPersona(_ url: URL, overwrite: Bool = false) {
        send(kind: PETSONA_COMMAND_IMPORT_PERSONA,
             value: overwrite ? 1 : 0,
             text: url.path)
    }

    func exportPersona(_ id: String, to url: URL) {
        send(kind: PETSONA_COMMAND_EXPORT_PERSONA, text: id + "\n" + url.path)
    }

    func updateDeepSeekConfig(_ object: [String: Any]) {
        sendJSONObject(kind: PETSONA_COMMAND_UPDATE_DEEPSEEK_CONFIG, object: object)
    }

    func updateMemoryConfig(_ object: [String: Any]) {
        sendJSONObject(kind: PETSONA_COMMAND_UPDATE_MEMORY_CONFIG, object: object)
    }

    func listModels() {
        send(kind: PETSONA_COMMAND_LIST_MODELS)
    }

    func updateMemoryFact(_ object: [String: Any]) {
        sendJSONObject(kind: PETSONA_COMMAND_UPDATE_MEMORY_FACT, object: object)
    }

    func updateMemoryFact(id: String, key: String, value: String, confidence: Double = 0.8) {
        updateMemoryFact(["id": id, "key": key, "value": value, "confidence": confidence])
    }

    func clearMemory(scope: Int) {
        send(kind: PETSONA_COMMAND_CLEAR_MEMORY_SCOPE, value: Double(scope))
    }

    func exportMemory(to url: URL, personaId: String) {
        send(kind: PETSONA_COMMAND_EXPORT_MEMORY, text: url.path)
    }

    func importMemory(from url: URL, personaId: String) {
        send(kind: PETSONA_COMMAND_IMPORT_MEMORY, text: url.path)
    }

    func updateGreetingConfig(_ object: [String: Any]) {
        sendJSONObject(kind: PETSONA_COMMAND_UPDATE_GREETING_CONFIG, object: object)
    }

    func rememberFact(key: String, value: String, confidence: Double = 0.8) {
        sendJSONObject(kind: PETSONA_COMMAND_REMEMBER_FACT,
                       object: ["key": key, "value": value, "confidence": confidence])
    }

    func forgetFact(_ id: String) {
        send(kind: PETSONA_COMMAND_FORGET_FACT, text: id)
    }

    func clearMemory() {
        send(kind: PETSONA_COMMAND_CLEAR_MEMORY)
    }

    private func sendJSONObject(kind: PetsonaCommandKind, object: [String: Any]) {
        guard JSONSerialization.isValidJSONObject(object),
              let data = try? JSONSerialization.data(withJSONObject: object),
              let text = String(data: data, encoding: .utf8) else {
            errorMessage = "设置内容无法编码为 JSON"
            return
        }
        send(kind: kind, text: text)
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
