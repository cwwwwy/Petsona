import Foundation

struct ConversationTurnProjection: Decodable, Identifiable, Equatable {
    var id: String
    var requestId: String?
    var user: Bool
    var text: String
    var status: String
    var createdAt: Int64
}

struct ConversationProjection: Decodable, Equatable {
    var petId = ""
    var saveHistory = true
    var requestId: String?
    var inFlight = false
    var error = ""
    var totalCount = 0
    var hasEarlier = false
    var turns: [ConversationTurnProjection] = []

    enum CodingKeys: String, CodingKey {
        case petId, saveHistory, requestId, inFlight, error, totalCount, hasEarlier, turns
    }

    init() {}

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        petId = try values.decodeIfPresent(String.self, forKey: .petId) ?? ""
        saveHistory = try values.decodeIfPresent(Bool.self, forKey: .saveHistory) ?? true
        requestId = try values.decodeIfPresent(String.self, forKey: .requestId)
        inFlight = try values.decodeIfPresent(Bool.self, forKey: .inFlight) ?? false
        error = try values.decodeIfPresent(String.self, forKey: .error) ?? ""
        totalCount = try values.decodeIfPresent(Int.self, forKey: .totalCount) ?? 0
        hasEarlier = try values.decodeIfPresent(Bool.self, forKey: .hasEarlier) ?? false
        turns = try values.decodeIfPresent([ConversationTurnProjection].self, forKey: .turns) ?? []
    }
}

enum PersonaSourceMode: String, CaseIterable, Identifiable {
    case chatImport
    case publicFigure
    var id: String { rawValue }
    var label: String {
        switch self {
        case .chatImport: return "聊天记录"
        case .publicFigure: return "参考人物"
        }
    }
}

struct PersonaSourceSpeakerProjection: Decodable, Identifiable {
    let name: String
    let count: Int
    var id: String { name }
}

struct PersonaSourceMessageProjection: Decodable, Identifiable {
    let id: String
    let speaker: String
    let text: String
    let timestamp: String?
}

struct PersonaSourceProjection: Decodable, Identifiable {
    let id: String?
    let label: String?
    let format: String?
    let messageCount: Int
    let speakers: [PersonaSourceSpeakerProjection]
    let preview: [PersonaSourceMessageProjection]
    let error: String
    let parsing: Bool
}

struct PersonaStyleProfileDraft: Codable, Equatable {
    var personality = ""
    var expressionStyle = ""
    var responseHabits = ""
    var relationship = ""
    var examples: [String] = []
}

struct PersonaSourceMetadataDraft: Decodable {
    let kind: String
    let label: String
    let description: String
    let sampleCount: Int
    let generatedAt: Int64
    let targetSpeaker: String?
}

struct PersonaDraftProjection: Decodable, Identifiable {
    let id: String
    let name: String
    let style: PersonaStyleProfileDraft
    let source: PersonaSourceMetadataDraft
}

struct PersonaDraftEnvelope: Decodable {
    let draft: PersonaDraftProjection?
    let generating: Bool
    let error: String
}

struct PersonaPreviewProjection: Decodable {
    let inFlight: Bool
    let text: String
    let error: String
}
