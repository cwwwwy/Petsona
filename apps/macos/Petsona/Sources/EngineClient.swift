import Foundation

@MainActor
final class EngineClient: ObservableObject {
    @Published private(set) var snapshot = PetsonaSnapshot()
    @Published private(set) var errorMessage = ""
    @Published private(set) var statusMessage = ""
    @Published private(set) var conversation = ConversationProjection()
    @Published private(set) var personaSource: PersonaSourceProjection?
    @Published private(set) var personaDraft = PersonaDraftEnvelope(draft: nil,
                                                                      generating: false,
                                                                      error: "")
    @Published private(set) var personaPreview = PersonaPreviewProjection(inFlight: false,
                                                                           text: "",
                                                                           error: "")
    var personaSourceError: String { personaSource?.error ?? "" }
    var personaDraftError: String { personaDraft.error }
    private var lastConversationJSON = ""
    private var lastPersonaSourceJSON = ""
    private var lastPersonaDraftJSON = ""
    private var lastPersonaPreviewJSON = ""
    private var pendingConfigurationFields: [UInt32: [String: Any]] = [:]

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

    func shutdown() {
        guard let handle else { return }
        petsona_engine_destroy(handle)
        self.handle = nil
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
        let conversationJSON = text(PETSONA_TEXT_CONVERSATION)
        if conversationJSON != lastConversationJSON {
            lastConversationJSON = conversationJSON
            if let data = conversationJSON.data(using: .utf8),
               let projection = try? JSONDecoder().decode(ConversationProjection.self, from: data) {
                conversation = projection
            }
        }
        let personaSourceJSON = text(PETSONA_TEXT_PERSONA_SOURCE)
        if personaSourceJSON != lastPersonaSourceJSON {
            lastPersonaSourceJSON = personaSourceJSON
            if let data = personaSourceJSON.data(using: .utf8) {
                personaSource = try? JSONDecoder().decode(PersonaSourceProjection.self, from: data)
            }
        }
        let personaDraftJSON = text(PETSONA_TEXT_PERSONA_DRAFT)
        if personaDraftJSON != lastPersonaDraftJSON {
            lastPersonaDraftJSON = personaDraftJSON
            if let data = personaDraftJSON.data(using: .utf8),
               let value = try? JSONDecoder().decode(PersonaDraftEnvelope.self, from: data) {
                personaDraft = value
            }
        }
        let personaPreviewJSON = text(PETSONA_TEXT_PERSONA_PREVIEW)
        if personaPreviewJSON != lastPersonaPreviewJSON {
            lastPersonaPreviewJSON = personaPreviewJSON
            if let data = personaPreviewJSON.data(using: .utf8),
               let value = try? JSONDecoder().decode(PersonaPreviewProjection.self, from: data) {
                personaPreview = value
            }
        }
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
        reconcileConfiguration(field: PETSONA_TEXT_DEEPSEEK_CONFIG)
        reconcileConfiguration(field: PETSONA_TEXT_MEMORY, section: "config")
        reconcileConfiguration(field: PETSONA_TEXT_MEMORY, section: "greeting")
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

    func sendPosition(x: CGFloat, y: CGFloat) {
        send(kind: PETSONA_COMMAND_SET_POSITION,
             text: "\(Double(x)),\(Double(y))")
    }

    func savedWindowPosition() -> SavedWindowPosition? {
        guard let data = text(PETSONA_TEXT_WINDOW_POSITION).data(using: .utf8) else { return nil }
        guard let position = try? JSONDecoder().decode(SavedWindowPosition.self, from: data),
              position.displayId != nil else { return nil }
        return position
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

    func sendWindowPosition(_ position: SavedWindowPosition) {
        guard let data = try? JSONEncoder().encode(position),
              let value = String(data: data, encoding: .utf8) else { return }
        send(kind: PETSONA_COMMAND_SET_WINDOW_POSITION, text: value)
    }

    func startConversation(_ message: String, retryTurnID: String? = nil) {
        var request: [String: Any] = [
            "requestId": UUID().uuidString,
            "petId": conversation.petId.isEmpty ? text(PETSONA_TEXT_PET_ID) : conversation.petId,
            "text": message,
        ]
        if let retryTurnID { request["retryTurnId"] = retryTurnID }
        sendJSONObject(kind: PETSONA_COMMAND_START_CONVERSATION, object: request)
    }

    func cancelConversation() {
        guard let requestId = conversation.requestId else { return }
        send(kind: PETSONA_COMMAND_CANCEL_CONVERSATION, text: requestId)
    }

    func updateConversationConfig(saveHistory: Bool) {
        sendJSONObject(kind: PETSONA_COMMAND_UPDATE_CONVERSATION_CONFIG,
                       object: ["saveHistory": saveHistory])
    }

    func clearConversationHistory() {
        let petID = conversation.petId.isEmpty ? text(PETSONA_TEXT_PET_ID) : conversation.petId
        guard !petID.isEmpty else { return }
        send(kind: PETSONA_COMMAND_CLEAR_CONVERSATION_HISTORY, text: petID)
    }

    func loadEarlierConversationHistory() {
        guard !conversation.petId.isEmpty else { return }
        send(kind: PETSONA_COMMAND_LOAD_EARLIER_CONVERSATION_HISTORY,
             text: conversation.petId)
    }

    func reviewMemoryCandidate(_ id: String, accept: Bool) {
        sendJSONObject(kind: PETSONA_COMMAND_REVIEW_MEMORY_CANDIDATE,
                       object: ["candidateId": id, "accept": accept])
    }

    func parsePersonaSourceFile(_ url: URL) {
        sendJSONObject(kind: PETSONA_COMMAND_PARSE_PERSONA_SOURCE,
                       object: [
                        "requestId": UUID().uuidString,
                        "label": url.lastPathComponent,
                        "format": url.pathExtension.lowercased(),
                        "path": url.path,
                       ])
    }

    func parsePersonaSourceText(_ text: String, format: String) {
        sendJSONObject(kind: PETSONA_COMMAND_PARSE_PERSONA_SOURCE,
                       object: [
                        "requestId": UUID().uuidString,
                        "label": "粘贴的聊天记录",
                        "format": format,
                        "text": text,
                       ])
    }

    func generatePersonaProfile(kind: String,
                                sourceID: String,
                                label: String,
                                description: String,
                                targetSpeaker: String,
                                targetSpeakerLabel: String,
                                startIndex: Int,
                                endIndex: Int) {
        sendJSONObject(kind: PETSONA_COMMAND_GENERATE_PERSONA_PROFILE,
                       object: [
                        "requestId": UUID().uuidString,
                        "sourceId": sourceID,
                        "kind": kind,
                        "label": label,
                        "description": description,
                        "targetSpeaker": targetSpeaker,
                        "targetSpeakerLabel": targetSpeakerLabel,
                        "startIndex": startIndex,
                        "endIndex": endIndex,
                       ])
    }

    func applyPersonaDraft(draftID: String,
                           petID: String,
                           name: String,
                           style: PersonaStyleProfileDraft) {
        guard let data = try? JSONEncoder().encode(style),
              let styleObject = try? JSONSerialization.jsonObject(with: data) else {
            errorMessage = "人格内容无法编码为 JSON"
            return
        }
        sendJSONObject(kind: PETSONA_COMMAND_APPLY_PERSONA_DRAFT,
                       object: [
                        "draftId": draftID,
                        "petId": petID,
                        "name": name,
                        "style": styleObject,
                       ])
    }

    func previewPersonaDraft(draftID: String, petID: String, prompt: String) {
        sendJSONObject(kind: PETSONA_COMMAND_PREVIEW_PERSONA_DRAFT,
                       object: [
                        "requestId": UUID().uuidString,
                        "petId": petID,
                        "draftId": draftID,
                        "prompt": prompt,
                       ])
    }

    func clearPersonaDraft() {
        send(kind: PETSONA_COMMAND_CLEAR_PERSONA_DRAFT)
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

    func importPersona(_ url: URL) {
        send(kind: PETSONA_COMMAND_APPLY_IMPORTED_PERSONA,
             text: url.path)
    }

    func exportPersona(_ id: String, to url: URL) {
        send(kind: PETSONA_COMMAND_EXPORT_PERSONA, text: id + "\n" + url.path)
    }

    func updateDeepSeekConfig(_ object: [String: Any]) {
        updateConfiguration(kind: PETSONA_COMMAND_UPDATE_DEEPSEEK_CONFIG,
                            field: PETSONA_TEXT_DEEPSEEK_CONFIG, changes: object)
    }

    func updateMemoryConfig(_ object: [String: Any]) {
        updateConfiguration(kind: PETSONA_COMMAND_UPDATE_MEMORY_CONFIG,
                            field: PETSONA_TEXT_MEMORY, section: "config", changes: object)
    }

    func resetPersona() {
        send(kind: PETSONA_COMMAND_RESET_PERSONA)
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
        updateConfiguration(kind: PETSONA_COMMAND_UPDATE_GREETING_CONFIG,
                            field: PETSONA_TEXT_MEMORY, section: "greeting", changes: object)
    }

    // ABI 3 configuration commands replace the complete object. Keep hidden
    // settings and edits still queued in the worker when the UI changes a field.
    private func configuration(field: PetsonaTextField, section: String?) -> [String: Any]? {
        guard let data = text(field).data(using: .utf8),
              let projection = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return section.map { projection[$0] as? [String: Any] } ?? projection
    }

    private func updateConfiguration(kind: PetsonaCommandKind, field: PetsonaTextField,
                                     section: String? = nil, changes: [String: Any]) {
        guard var current = configuration(field: field, section: section) else {
            reportError("设置尚未加载，请稍后再试。")
            return
        }
        var pending = pendingConfigurationFields[kind.rawValue] ?? [:]
        pending.merge(changes) { _, new in new }
        current.merge(pending) { _, new in new }
        current.removeValue(forKey: "keyConfigured")
        pendingConfigurationFields[kind.rawValue] = pending
        sendJSONObject(kind: kind, object: current)
    }

    private func reconcileConfiguration(field: PetsonaTextField, section: String? = nil) {
        let kind = field == PETSONA_TEXT_DEEPSEEK_CONFIG ? PETSONA_COMMAND_UPDATE_DEEPSEEK_CONFIG
            : (section == "greeting" ? PETSONA_COMMAND_UPDATE_GREETING_CONFIG : PETSONA_COMMAND_UPDATE_MEMORY_CONFIG)
        guard let current = configuration(field: field, section: section),
              let pending = pendingConfigurationFields[kind.rawValue] else { return }
        let outstanding = pending.filter { key, value in
            guard let projected = current[key] as? NSObject, let expected = value as? NSObject else { return true }
            return !projected.isEqual(expected)
        }
        pendingConfigurationFields[kind.rawValue] = outstanding.isEmpty ? nil : outstanding
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
