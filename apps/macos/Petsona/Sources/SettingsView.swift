import AppKit
import SwiftUI
import UniformTypeIdentifiers

private struct PetChoice: Decodable, Identifiable {
    let id: String
    let name: String
    let v2: Bool
    let spritesheet: String?
    let cellWidth: Int?
    let cellHeight: Int?

    enum CodingKeys: String, CodingKey {
        case id, name, v2, spritesheet, cellWidth, cellHeight
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(String.self, forKey: .id)
        name = try values.decode(String.self, forKey: .name)
        v2 = try values.decodeIfPresent(Bool.self, forKey: .v2) ?? false
        spritesheet = try values.decodeIfPresent(String.self, forKey: .spritesheet)
        cellWidth = try values.decodeIfPresent(Int.self, forKey: .cellWidth)
        cellHeight = try values.decodeIfPresent(Int.self, forKey: .cellHeight)
    }
}

private struct CodexPetChoice: Decodable, Identifiable {
    let id: String
    let name: String
    let path: String
    let spritesheet: String
    let cellWidth: Int
    let cellHeight: Int
    let columns: Int
    let v2: Bool
}

private struct PersonaTraitsProjection: Decodable {
    var tone = ""
    var verbosity = "normal"
    var emoji = true

    enum CodingKeys: String, CodingKey { case tone, verbosity, emoji }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        tone = try values.decodeIfPresent(String.self, forKey: .tone) ?? tone
        verbosity = try values.decodeIfPresent(String.self, forKey: .verbosity) ?? verbosity
        emoji = try values.decodeIfPresent(Bool.self, forKey: .emoji) ?? emoji
    }
}

private struct PersonaSamplingProjection: Decodable {
    var temperature = 0.8
    var maxTokens = 800
}

private struct PersonaMemoryProjection: Decodable {
    var enabled = true
    var windowTurns = 12
    var longTerm = true
    var summarizeAfterTurns = 20
}

private struct PersonaTTSProjection: Decodable {
    var enabled = false
    var voice: String?
    var rate = 1.0
}

private struct PersonaProactiveProjection: Decodable {
    var enabled = false
    var idleMinutes = 30
}

private struct PersonaModelProjection: Decodable {
    var provider = ""
    var model: String?
}

private struct PersonaProjection: Decodable {
    var id = "default"
    var name = "小助手"
    var systemPrompt = ""
    var greeting: String?
    var traits = PersonaTraitsProjection(tone: "", verbosity: "normal", emoji: true)

    enum CodingKeys: String, CodingKey {
        case id, name, systemPrompt, greeting, traits
    }

    init() {}

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(String.self, forKey: .id) ?? id
        name = try values.decodeIfPresent(String.self, forKey: .name) ?? name
        systemPrompt = try values.decodeIfPresent(String.self, forKey: .systemPrompt) ?? systemPrompt
        greeting = try values.decodeIfPresent(String.self, forKey: .greeting)
        traits = try values.decodeIfPresent(PersonaTraitsProjection.self, forKey: .traits) ?? traits
    }
}

private extension PersonaTraitsProjection {
    init(tone: String, verbosity: String, emoji: Bool) {
        self.tone = tone
        self.verbosity = verbosity
        self.emoji = emoji
    }
}

/// Tone presets carry the style text and the reply length (REQ-S13/S14).
private struct TonePreset: Identifiable {
    let label: String
    let tone: String
    let verbosity: String
    var id: String { label }

    static let all: [TonePreset] = [
        TonePreset(label: "温和友好", tone: "温和、友好、乐于帮忙", verbosity: "normal"),
        TonePreset(label: "简洁干练", tone: "简洁、直接、不说废话", verbosity: "short"),
        TonePreset(label: "活泼元气", tone: "活泼、元气满满、鼓励式回应", verbosity: "normal"),
        TonePreset(label: "沉稳顾问", tone: "沉稳、克制、结构化", verbosity: "detailed"),
        TonePreset(label: "毒舌但温柔", tone: "毒舌但温柔，吐槽背后是真的关心", verbosity: "short"),
    ]
}

struct DeepSeekProjection: Decodable {
    var provider = "deepseek"
    /// Recomputed by the worker when the key or provider changes (W19 feedback).
    var keyConfigured = false
    var baseUrl = "https://api.deepseek.com/v1"
    var model = "deepseek-v4-flash"
    var apiKeyEnv = "DEEPSEEK_API_KEY"
    var timeoutSeconds = 20
    var maxTokens = 80
    var temperature = 0.9
    var thinkingDisabled = true

    var credentialStatusLabel: String {
        keyConfigured ? "已配置（密钥不会显示）" : "未配置"
    }
}

private struct MemoryConfigProjection: Decodable {
    var enabled = true
    var recentEvents = 5
    var factLimit = 20
    var eventRetentionDays = 0
    var factCompress = true
}

private struct GreetingConfigProjection: Decodable {
    var enabled = true
    var idleMinutes = 30
    var cooldownMinutes = 120
    var maxChars = 40
}

private struct MemoryFactProjection: Decodable, Identifiable {
    let id: String
    let key: String
    let value: String
    let confidence: Double
    let createdAt: Int64
    let updatedAt: Int64
    var source: String?

    /// Where the fact came from (REQ-P06); older files have no source.
    var sourceLabel: String {
        switch source {
        case "conversation": return "对话"
        case "import": return "导入"
        case "compressed": return "压缩"
        default: return "手动"
        }
    }
}

private struct MemoryEventProjection: Decodable, Identifiable {
    let id: String
    let kind: String
    let text: String?
    let createdAt: Int64
}

private struct MemoryProjection: Decodable {
    var config = MemoryConfigProjection()
    var greeting = GreetingConfigProjection()
    var facts: [MemoryFactProjection] = []
    var events: [MemoryEventProjection] = []
}

private struct ImportConflictProjection: Decodable {
    let id: String
    let name: String
    let path: String
}

enum SettingsSection: String, CaseIterable, Identifiable {
    case library
    case behavior
    case persona
    case memory
    case deepSeek
    case startup

    var id: String { rawValue }

    var title: String {
        switch self {
        case .library: return "宠物库"
        case .behavior: return "外观与交互"
        case .deepSeek: return "模型服务"
        case .persona: return "人格"
        case .memory: return "记忆"
        case .startup: return "系统"
        }
    }

    var systemImage: String {
        switch self {
        case .library: return "pawprint.fill"
        case .behavior: return "slider.horizontal.3"
        case .persona: return "person.crop.circle"
        case .memory: return "clock.arrow.circlepath"
        case .deepSeek: return "globe"
        case .startup: return "gearshape"
        }
    }
}

enum SettingsLayout {
    static let windowMinWidth: CGFloat = 720
    static let windowMinHeight: CGFloat = 600
    static let contentMaxWidth: CGFloat = 1000
    static let sidebarMinWidth: CGFloat = 160
    static let sidebarIdealWidth: CGFloat = 200
    static let sidebarMaxWidth: CGFloat = 240
    static let cardPadding: CGFloat = 16
    static let rowGap: CGFloat = 16
    static let rowLabelMinWidth: CGFloat = 220
}

struct SettingsView: View {
    @ObservedObject var engine: EngineClient

    @State private var scale = 1.0
    @State private var clickThrough = true
    @State private var alwaysOnTop = true
    @State private var showingCodexPets = false
    @State private var dropTargeted = false

    @State private var personaName = ""
    @State private var personaTone = ""
    @State private var personaTonePreset = ""
    @State private var personaVerbosity = "normal"
    @State private var personaEmoji = false
    @State private var greeting = ""
    @State private var systemPrompt = ""

    @State private var deepSeekProvider = "deepseek"
    @State private var deepSeekBaseURL = "https://api.deepseek.com/v1"
    @State private var deepSeekModel = "deepseek-v4-flash"
    @State private var deepSeekAPIKeyEnv = "DEEPSEEK_API_KEY"
    @State private var deepSeekTimeout = 20
    @State private var deepSeekMaxTokens = 80
    @State private var deepSeekTemperature = 0.9
    @State private var deepSeekThinkingDisabled = true
    @State private var deepSeekKey = ""

    @State private var memoryEnabled = true
    @State private var memoryRecentEvents = 5
    @State private var memoryFactLimit = 20
    @State private var memoryRetentionDays = 0
    @State private var memoryCompress = true

    @State private var greetingEnabled = true
    @State private var greetingIdleMinutes = 30
    @State private var greetingCooldownMinutes = 120
    @State private var greetingMaxChars = 40

    /// Signatures captured when the form is filled from the engine, so that a
    /// reload never looks like a user edit (instant apply, REQ-S05).
    @State private var loadedPersonaSignature = ""
    @State private var loadedDeepSeekSignature = ""
    @State private var loadedMemorySignature = ""
    @State private var loadedGreetingSignature = ""
    @State private var pendingApply: Task<Void, Never>?
    @State private var factKey = ""
    @State private var factValue = ""
    @State private var editingFactID = ""
    @State private var autostart = false
    @State private var selectedSection: SettingsSection = .library

    private var pets: [PetChoice] { decode(PETSONA_TEXT_PETS, as: [PetChoice].self) ?? [] }
    private var codexPets: [CodexPetChoice] { decode(PETSONA_TEXT_CODEX_PETS, as: [CodexPetChoice].self) ?? [] }
    private var currentPersona: PersonaProjection {
        decode(PETSONA_TEXT_PERSONA, as: PersonaProjection.self) ?? PersonaProjection()
    }
    private var deepSeek: DeepSeekProjection {
        decode(PETSONA_TEXT_DEEPSEEK_CONFIG, as: DeepSeekProjection.self) ?? DeepSeekProjection()
    }
    /// A failed model fetch is shown next to the button, not just in the footer.
    private var modelFetchFailed: Bool {
        engine.statusMessage.hasPrefix("拉取模型列表失败") ||
        engine.statusMessage.hasPrefix("服务商没有返回")
    }

    private var availableModels: [String] {
        decode(PETSONA_TEXT_MODELS, as: [String].self) ?? []
    }

    private var memory: MemoryProjection {
        decode(PETSONA_TEXT_MEMORY, as: MemoryProjection.self) ?? MemoryProjection()
    }
    private var importConflict: ImportConflictProjection? {
        decode(PETSONA_TEXT_IMPORT_CONFLICT, as: ImportConflictProjection.self)
    }

    var body: some View {
        NavigationSplitView {
            List(SettingsSection.allCases, selection: $selectedSection) { section in
                Label(section.title, systemImage: section.systemImage)
                    .tag(section)
            }
            .listStyle(.sidebar)
            .navigationSplitViewColumnWidth(min: SettingsLayout.sidebarMinWidth,
                                             ideal: SettingsLayout.sidebarIdealWidth,
                                             max: SettingsLayout.sidebarMaxWidth)
        } detail: {
            settingsDetail
        }
        .navigationSplitViewStyle(.balanced)
        .frame(minWidth: SettingsLayout.windowMinWidth,
               minHeight: SettingsLayout.windowMinHeight)
        .onAppear { reloadForm() }
    }

    @ViewBuilder
    private var settingsDetail: some View {
        switch selectedSection {
        case .library:
            settingsForm(title: "宠物库") { petLibrarySection }
        case .behavior:
            settingsForm(title: "外观与交互") { behaviorSection }
        case .deepSeek:
            settingsForm(title: "模型服务") { deepSeekSection }
        case .persona:
            settingsForm(title: "人格") { personaSection }
        case .memory:
            settingsForm(title: "记忆") { memorySection }
        case .startup:
            settingsForm(title: "系统") { startupSection }
        }
    }

    @ViewBuilder
    private func settingsForm<Content: View>(title: String,
                                               @ViewBuilder content: () -> Content) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(title)
                    .font(.title2.weight(.semibold))
                    .frame(maxWidth: .infinity, alignment: .leading)
                content()
                    .frame(maxWidth: .infinity, alignment: .leading)
                if !engine.errorMessage.isEmpty {
                    settingsCard("错误") {
                        Text(engine.errorMessage).foregroundStyle(.red)
                    }
                }
                if !engine.statusMessage.isEmpty {
                    settingsCard("状态") {
                        Text(engine.statusMessage)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .frame(maxWidth: SettingsLayout.contentMaxWidth, alignment: .leading)
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    @ViewBuilder
    private func settingsCard<Content: View>(_ title: String,
                                              @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.headline)
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(SettingsLayout.cardPadding)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color(nsColor: .controlBackgroundColor))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(Color.secondary.opacity(0.18))
        )
    }

    @ViewBuilder
    private func settingsRow<Control: View>(
        _ title: String,
        description: String? = nil,
        @ViewBuilder control: () -> Control
    ) -> some View {
        let label = VStack(alignment: .leading, spacing: 3) {
            Text(title).fontWeight(.semibold)
            if let description {
                Text(description)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }

        ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: SettingsLayout.rowGap) {
                label
                    .frame(minWidth: SettingsLayout.rowLabelMinWidth,
                           maxWidth: .infinity,
                           alignment: .leading)
                Spacer(minLength: 0)
                control()
                    .fixedSize(horizontal: true, vertical: false)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            VStack(alignment: .leading, spacing: 8) {
                label.frame(maxWidth: .infinity, alignment: .leading)
                HStack {
                    Spacer(minLength: 0)
                    control()
                }
                .frame(maxWidth: .infinity)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var petLibrarySection: some View {
        settingsCard("宠物库") {
            if engine.snapshot.has_pet == 0 {
                Text("本地库为空。请选择一个 Codex 宠物包或文件夹导入。")
                    .foregroundStyle(.secondary)
            } else {
                Text("当前：\(engine.text(PETSONA_TEXT_PET_NAME))")
            }
            HStack {
                Button("导入…") { importPet() }
                Text("支持文件夹或 .zip")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
            RoundedRectangle(cornerRadius: 8)
                .stroke(dropTargeted ? Color.accentColor : Color.secondary,
                        style: StrokeStyle(lineWidth: 1, dash: [5]))
                .frame(height: 42)
                .overlay(Text("将宠物文件夹或 .zip 拖到这里导入").foregroundStyle(.secondary))
                .onDrop(of: [UTType.fileURL], isTargeted: $dropTargeted, perform: handleDrop)

            if let conflict = importConflict {
                VStack(alignment: .leading, spacing: 6) {
                    Text("发现同 ID 宠物：\(conflict.name)（\(conflict.id)）")
                    Text("覆盖会替换 Petsona 本地库中的版本，Codex 原文件不会被修改。")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                    HStack {
                        Button("覆盖导入") {
                            engine.importPet(URL(fileURLWithPath: conflict.path), overwrite: true)
                        }
                        Button("取消") { engine.clearImportConflict() }
                    }
                }
                .padding(.vertical, 4)
            }

            if showingCodexPets {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Codex 宠物（选择后才会复制到 Petsona 本地库）")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                    if codexPets.isEmpty {
                        Text("没有发现可导入的宠物目录。").foregroundStyle(.secondary)
                    }
                    HStack {
                        Button("从 Codex 导入…") { importCodexPet() }
                        Button("重新扫描") { engine.send(kind: PETSONA_COMMAND_REFRESH_PETS) }
                    }
                    ForEach(codexPets) { pet in
                        HStack {
                            CodexPreview(path: pet.spritesheet,
                                         cellWidth: pet.cellWidth,
                                         cellHeight: pet.cellHeight,
                                         columns: pet.columns)
                            VStack(alignment: .leading) {
                                Text(pet.name)
                                Text(pet.id).font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            if pet.v2 { Text("V2").foregroundStyle(.secondary) }
                            Button("导入") { engine.importPet(URL(fileURLWithPath: pet.path)) }
                        }
                    }
                }
            }
            ForEach(pets) { pet in
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 12) {
                        if let spritesheet = pet.spritesheet,
                           let cellWidth = pet.cellWidth,
                           let cellHeight = pet.cellHeight {
                            CodexPreview(path: spritesheet,
                                         cellWidth: cellWidth,
                                         cellHeight: cellHeight,
                                         columns: 8)
                        } else {
                            Color.clear.frame(width: 48, height: 48)
                        }
                        VStack(alignment: .leading, spacing: 3) {
                            Text(pet.name)
                            Text(pet.id).font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        if pet.v2 { Text("V2").foregroundStyle(.secondary) }
                    }
                    HStack(spacing: 8) {
                        Spacer()
                        if engine.text(PETSONA_TEXT_PET_ID) == pet.id {
                            Text("当前").foregroundStyle(.secondary)
                        } else {
                            Button("切换") { engine.selectPet(pet.id) }
                        }
                        Button("导出") { exportPet(pet.id) }
                        Button("删除") { deletePet(pet.id) }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private var personaSection: some View {
        settingsCard("这只宠物的人格") {
            HStack {
                Button("导入…") { importPersona() }
                Button("导出…") { exportPersona(engine.text(PETSONA_TEXT_PERSONA_ID)) }
                Button("重置为内置") {
                    let alert = NSAlert()
                    alert.messageText = "重置人格"
                    alert.informativeText = "会恢复内置的语气、emoji 与提示词；这只宠物的记忆不受影响。"
                    alert.addButton(withTitle: "重置")
                    alert.addButton(withTitle: "取消")
                    if alert.runModal() == .alertFirstButtonReturn {
                        engine.resetPersona()
                    }
                }
            }
            settingsRow("语气", description: "直接描述宠物希望采用的说话方式。") {
                TextField("例如：毒舌但温柔", text: $personaTone)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 220, maxWidth: 360)
            }
            settingsRow("语气预设", description: "预设会同时调整回答的长短。") {
                Picker("语气预设", selection: $personaTonePreset) {
                    Text("自定义…").tag("")
                    ForEach(TonePreset.all) { preset in
                        Text(preset.label).tag(preset.tone)
                    }
                }
                .frame(width: 220)
                .onChange(of: personaTonePreset) { tone in
                    guard !tone.isEmpty else { return }
                    personaTone = tone
                    personaVerbosity = TonePreset.all.first { $0.tone == tone }?.verbosity ?? "normal"
                }
            }
            settingsRow("允许 emoji", description: "关闭时会要求模型不要使用 emoji。") {
                Toggle("", isOn: $personaEmoji)
                    .labelsHidden()
                    .toggleStyle(.switch)
            }
            DisclosureGroup {
                VStack(alignment: .leading, spacing: 8) {
                    TextEditor(text: $systemPrompt)
                        .frame(minHeight: 120)
                        .frame(maxWidth: .infinity)
                        .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.secondary.opacity(0.25)))
                    Text("语气 / emoji 会由上面的设置自动追加，不需要在这里重复。")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            } label: {
                VStack(alignment: .leading, spacing: 3) {
                    Text("高级").font(.headline)
                    Text("系统提示词：模型的核心指令。")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .onChange(of: personaSignature) { value in
            guard value != loadedPersonaSignature else { return }
            scheduleApply(savePersona)
        }
    }

    @ViewBuilder
    private var deepSeekSection: some View {
        settingsCard("模型服务") {
            settingsRow("服务商", description: "选择 DeepSeek 或其他 OpenAI 兼容端点。") {
                Picker("服务商", selection: $deepSeekProvider) {
                    Text("DeepSeek").tag("deepseek")
                    Text("自定义").tag("custom")
                }
                .frame(width: 220)
                .onChange(of: deepSeekProvider) { provider in
                    if provider != "custom" {
                        deepSeekBaseURL = "https://api.deepseek.com/v1"
                    }
                    scheduleApply(saveDeepSeekConfig)
                }
            }
            settingsRow("Base URL", description: deepSeekProvider == "custom" ? "自定义 OpenAI 兼容端点。" : "DeepSeek 的固定地址。") {
                TextField("Base URL", text: $deepSeekBaseURL)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 280, maxWidth: 420)
                    .disabled(deepSeekProvider != "custom")
            }
            settingsRow("API Key", description: "密钥只保存到 macOS Keychain，不会进入配置快照。") {
                VStack(alignment: .trailing, spacing: 8) {
                    SecureField("可选；输入新密钥可覆盖", text: $deepSeekKey)
                        .textFieldStyle(.roundedBorder)
                        .frame(minWidth: 240, maxWidth: 320)
                    HStack {
                        Button("保存密钥") {
                            do {
                                try KeychainService.saveDeepSeekKey(deepSeekKey, provider: deepSeekProvider)
                                deepSeekKey = ""
                                engine.reportError("")
                            } catch {
                                engine.reportError(error.localizedDescription)
                            }
                        }
                        Button("清除密钥") {
                            do { try KeychainService.deleteDeepSeekKey(provider: deepSeekProvider) }
                            catch { engine.reportError(error.localizedDescription) }
                        }
                        .disabled(!deepSeek.keyConfigured)
                        Text(deepSeek.credentialStatusLabel)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            settingsRow("模型", description: "可以手动填写，也可以从服务商拉取列表。") {
                VStack(alignment: .trailing, spacing: 8) {
                    TextField("模型名称", text: $deepSeekModel)
                        .textFieldStyle(.roundedBorder)
                        .frame(minWidth: 240, maxWidth: 320)
                    HStack {
                        Button("拉取模型列表") { engine.listModels() }
                        if !availableModels.isEmpty {
                            Picker("选择模型", selection: $deepSeekModel) {
                                ForEach(availableModels, id: \.self) { Text($0).tag($0) }
                            }
                            .frame(width: 220)
                        }
                    }
                }
            }
            if modelFetchFailed {
                Text(engine.statusMessage).font(.footnote).foregroundStyle(.red)
            }
        }

        settingsCard("高级") {
            settingsRow("API Key 环境变量", description: "优先使用该环境变量；为空时默认 DEEPSEEK_API_KEY。") {
                TextField("环境变量名", text: $deepSeekAPIKeyEnv)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 240)
            }
            settingsRow("超时", description: "单次请求的最长等待时间。") {
                Stepper("\(deepSeekTimeout) 秒", value: $deepSeekTimeout, in: 5...120)
                    .frame(width: 160, alignment: .trailing)
            }
            settingsRow("最大 token", description: "对话回复的长度上限。") {
                Stepper("\(deepSeekMaxTokens)", value: $deepSeekMaxTokens, in: 16...4000, step: 16)
                    .frame(width: 160, alignment: .trailing)
            }
            settingsRow("温度", description: "越高越随机；0.7 左右比较自然。") {
                HStack {
                    Slider(value: $deepSeekTemperature, in: 0...2, step: 0.1)
                        .frame(width: 220)
                    Text(String(format: "%.1f", deepSeekTemperature))
                        .frame(width: 40, alignment: .trailing)
                }
            }
            if deepSeekProvider == "custom" {
                Text("自定义端点不会收到 DeepSeek 专有的 thinking 字段。")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            } else {
                settingsRow("思考模式", description: "关闭思考链以缩短短回复的延迟。") {
                    Toggle("", isOn: $deepSeekThinkingDisabled)
                        .labelsHidden()
                        .toggleStyle(.switch)
                }
            }
        }
        .onChange(of: deepSeekSignature) { value in
            guard value != loadedDeepSeekSignature else { return }
            scheduleApply(saveDeepSeekConfig)
        }

    }

    private var memorySection: some View {
        settingsCard("记忆与用户偏好") {
            settingsRow("启用记忆", description: "关闭后不再记录事件，也不会把已有记忆发送给模型。") {
                Toggle("", isOn: $memoryEnabled).labelsHidden().toggleStyle(.switch)
            }
            settingsRow("保留最近事件", description: "参与上下文的最近互动数量。") {
                Stepper("\(memoryRecentEvents)", value: $memoryRecentEvents, in: 1...100)
                    .frame(width: 150, alignment: .trailing)
            }
            settingsRow("最多偏好", description: "超过上限时，旧偏好可合并为一条画像。") {
                Stepper("\(memoryFactLimit)", value: $memoryFactLimit, in: 1...50)
                    .frame(width: 150, alignment: .trailing)
            }
            settingsRow("事件保留", description: "0 表示永久保留。") {
                Stepper("\(memoryRetentionDays) 天", value: $memoryRetentionDays, in: 0...3650)
                    .frame(width: 180, alignment: .trailing)
            }
            settingsRow("自动压缩偏好", description: "超出上限时合并为一条画像，而不是直接丢弃。") {
                Toggle("", isOn: $memoryCompress).labelsHidden().toggleStyle(.switch)
            }
            Divider()
            Text("对话中的“我喜欢… / 我不喜欢… / 请叫我…”等明确表达会自动记录，并用于后续回复。")
                .font(.footnote)
                .foregroundStyle(.secondary)
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .bottom, spacing: 8) {
                    memoryFactFields
                    memoryFactActions
                }
                VStack(alignment: .leading, spacing: 8) {
                    memoryFactFields
                    HStack {
                        Spacer(minLength: 0)
                        memoryFactActions
                    }
                }
            }
            ForEach(memory.facts) { fact in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        VStack(alignment: .leading) {
                            Text("\(fact.key)：\(fact.value)")
                            Text("来源：\(fact.sourceLabel)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("编辑") {
                            editingFactID = fact.id
                            factKey = fact.key
                            factValue = fact.value
                        }
                        Button("删除") {
                            engine.forgetFact(fact.id)
                            if editingFactID == fact.id { resetFactEditor() }
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            if !memory.events.isEmpty {
                DisclosureGroup("最近互动（\(memory.events.count)）") {
                    ForEach(memory.events.suffix(12)) { event in
                        Text(event.text ?? event.kind)
                            .font(.footnote)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            HStack {
                Button("只清偏好") { engine.clearMemory(scope: 1) }
                Button("只清事件") { engine.clearMemory(scope: 2) }
                Button("清空这只宠物的记忆") { engine.clearMemory(scope: 0) }
            }
            HStack {
                Button("导出记忆…") { exportMemory() }
                Button("导入记忆…") { importMemory() }
            }
            Text("记忆只保存在这台电脑上；只有配置了模型服务时，对话与记忆片段才会发送给该服务。")
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
        .onChange(of: memorySignature) { value in
            guard value != loadedMemorySignature else { return }
            scheduleApply(saveMemoryConfig)
        }
    }

    @ViewBuilder
    private var behaviorSection: some View {
        settingsCard("窗口与交互") {
            settingsRow("缩放", description: "固定档位 50%–200%，与状态栏菜单保持一致。") {
                HStack {
                    Slider(value: $scale, in: 0.5...2.0, step: 0.25)
                        .frame(width: 220)
                    Text(scaleLabel(scale)).frame(width: 56, alignment: .trailing)
                }
                .onChange(of: scale) { value in
                    engine.send(kind: PETSONA_COMMAND_SET_SCALE, value: value)
                }
            }
            settingsRow("像素级点击穿透", description: "透明像素的点击落到桌面；宠物像素仍可交互。") {
                Toggle("", isOn: $clickThrough)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .onChange(of: clickThrough) { value in
                        engine.send(kind: PETSONA_COMMAND_SET_CLICK_THROUGH, value: value ? 1 : 0)
                    }
            }
            settingsRow("始终置顶", description: "控制宠物窗口是否保持在普通窗口上方。") {
                Toggle("", isOn: $alwaysOnTop)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .onChange(of: alwaysOnTop) { value in
                        engine.send(kind: PETSONA_COMMAND_SET_ALWAYS_ON_TOP, value: value ? 1 : 0)
                    }
                }
            }

        settingsCard("空闲问候") {
            settingsRow("启用空闲问候", description: "长时间没有操作后显示一句短问候。") {
                Toggle("", isOn: $greetingEnabled).labelsHidden().toggleStyle(.switch)
            }
            settingsRow("固定问候文案", description: "无 Key 时使用；留空则按时间自动选择。") {
                TextField("问候文案", text: $greeting)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 240, maxWidth: 360)
            }
            settingsRow("空闲时长", description: "连续多久没有点击、拖动或输入后触发。") {
                Stepper("\(greetingIdleMinutes) 分钟", value: $greetingIdleMinutes, in: 1...1440)
                    .frame(width: 180, alignment: .trailing)
            }
            settingsRow("问候冷却", description: "两次问候之间的最短间隔；0 表示不限制。") {
                Stepper("\(greetingCooldownMinutes) 分钟", value: $greetingCooldownMinutes, in: 0...1440)
                    .frame(width: 180, alignment: .trailing)
            }
            settingsRow("问候最大字数", description: "模型回复超过该长度会被截断。") {
                Stepper("\(greetingMaxChars) 字", value: $greetingMaxChars, in: 1...200)
                    .frame(width: 160, alignment: .trailing)
            }
            Text("没有配置 DeepSeek 时使用人格里的固定问候；问候会显示为气泡并记入记忆。")
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
        .onChange(of: greetingSignature) { value in
            guard value != loadedGreetingSignature else { return }
            scheduleApply(saveGreetingConfig)
        }

        settingsCard("测试") {
            Button("测试问候") {
                engine.send(kind: PETSONA_COMMAND_SHOW_BUBBLE,
                            ttlMilliseconds: 5_000,
                            text: "你好，我在这里")
            }
        }
    }

    @ViewBuilder
    private var startupSection: some View {
        settingsCard("启动") {
            settingsRow("登录时启动 Petsona", description: "通过 macOS LaunchAgent 在登录后启动。") {
                Toggle("", isOn: $autostart)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .onChange(of: autostart) { value in
                        do {
                            try LaunchAgentService.setEnabled(value)
                        } catch {
                            autostart = LaunchAgentService.isEnabled
                            engine.reportError("自启设置失败：\(error.localizedDescription)")
                        }
                    }
            }
        }

        settingsCard("数据") {
            settingsRow("数据目录", description: dataDirectoryPath) {
                Button("打开") { openDataDirectory() }
            }
        }

        settingsCard("关于") {
            Text("Petsona \(appVersion)").font(.headline)
            Text("本地优先的桌宠：宠物、人格与记忆保存在你自己的机器上，只有配置了 DeepSeek 才会发起网络请求。")
                .font(.footnote)
                .foregroundStyle(.secondary)
            Link("项目主页", destination: URL(string: "https://github.com/cwwwwy/Petsona")!)
        }
    }

    private var appVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.1.0"
    }

    private var dataDirectoryPath: String {
        if let configured = ProcessInfo.processInfo.environment["PETSONA_HOME"], !configured.isEmpty {
            return configured
        }
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        return base?.appendingPathComponent("Petsona").path ?? "~/Library/Application Support/Petsona"
    }

    private func openDataDirectory() {
        NSWorkspace.shared.open(URL(fileURLWithPath: dataDirectoryPath))
    }

    private func decode<T: Decodable>(_ field: PetsonaTextField, as type: T.Type) -> T? {
        guard let data = engine.text(field).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(type, from: data)
    }

    private func reloadForm() {
        scale = Double(engine.snapshot.scale)
        clickThrough = engine.snapshot.click_through != 0
        alwaysOnTop = engine.snapshot.always_on_top != 0
        autostart = LaunchAgentService.isEnabled

        let persona = currentPersona
        personaName = persona.name
        personaTone = persona.traits.tone
        personaVerbosity = persona.traits.verbosity
        personaTonePreset = TonePreset.all.contains { $0.tone == persona.traits.tone }
            ? persona.traits.tone
            : ""
        personaEmoji = persona.traits.emoji
        greeting = persona.greeting ?? ""
        systemPrompt = persona.systemPrompt
        loadedPersonaSignature = personaSignature

        let deepSeek = deepSeek
        deepSeekProvider = deepSeek.provider == "custom" ? "custom" : "deepseek"
        deepSeekBaseURL = deepSeek.baseUrl
        deepSeekModel = deepSeek.model
        deepSeekAPIKeyEnv = deepSeek.apiKeyEnv
        deepSeekTimeout = deepSeek.timeoutSeconds
        deepSeekMaxTokens = deepSeek.maxTokens
        deepSeekTemperature = deepSeek.temperature
        deepSeekThinkingDisabled = deepSeek.thinkingDisabled
        loadedDeepSeekSignature = deepSeekSignature

        let memory = memory
        memoryEnabled = memory.config.enabled
        memoryRecentEvents = memory.config.recentEvents
        memoryFactLimit = memory.config.factLimit
        memoryRetentionDays = memory.config.eventRetentionDays
        memoryCompress = memory.config.factCompress
        loadedMemorySignature = memorySignature

        let greeting = memory.greeting
        greetingEnabled = greeting.enabled
        greetingIdleMinutes = greeting.idleMinutes
        greetingCooldownMinutes = greeting.cooldownMinutes
        greetingMaxChars = greeting.maxChars
        loadedGreetingSignature = greetingSignature
    }

    // Instant apply: every field change schedules one debounced command and
    // never fires for values that came from a reload (settings-consolidation S05).
    private func scheduleApply(_ action: @escaping () -> Void) {
        pendingApply?.cancel()
        pendingApply = Task { @MainActor in
            try? await Task.sleep(nanoseconds: 450_000_000)
            guard !Task.isCancelled else { return }
            action()
        }
    }

    private var personaSignature: String {
        [personaName, personaTone, personaVerbosity, personaEmoji ? "1" : "0",
         greeting, systemPrompt].joined(separator: "\u{1F}")
    }

    private var deepSeekSignature: String {
        [deepSeekProvider, deepSeekBaseURL, deepSeekModel, deepSeekAPIKeyEnv, String(deepSeekTimeout),
         String(deepSeekMaxTokens), String(deepSeekTemperature),
         deepSeekThinkingDisabled ? "1" : "0"].joined(separator: "\u{1F}")
    }

    private var memorySignature: String {
        [memoryEnabled ? "1" : "0", String(memoryRecentEvents), String(memoryFactLimit),
         String(memoryRetentionDays), memoryCompress ? "1" : "0"].joined(separator: "\u{1F}")
    }

    private var greetingSignature: String {
        [greetingEnabled ? "1" : "0", String(greetingIdleMinutes),
         String(greetingCooldownMinutes), String(greetingMaxChars)].joined(separator: "\u{1F}")
    }

    private func reloadLater() {
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 180_000_000)
            reloadForm()
        }
    }

    private func savePersona() {
        engine.updatePersona(fields: [
            "name": personaName,
            "tone": personaTone,
            "verbosity": personaVerbosity,
            "emoji": personaEmoji,
            "greeting": greeting,
            "system_prompt": systemPrompt,
        ])
        reloadLater()
    }

    private func saveDeepSeekConfig() {
        engine.updateDeepSeekConfig([
            "provider": deepSeekProvider,
            "baseUrl": deepSeekBaseURL,
            "model": deepSeekModel,
            "apiKeyEnv": deepSeekAPIKeyEnv,
            "timeoutSeconds": deepSeekTimeout,
            "maxTokens": deepSeekMaxTokens,
            "temperature": deepSeekTemperature,
            "thinkingDisabled": deepSeekThinkingDisabled,
        ])
    }

    private func saveMemoryConfig() {
        engine.updateMemoryConfig([
            "enabled": memoryEnabled,
            "recentEvents": memoryRecentEvents,
            "factLimit": memoryFactLimit,
            "eventRetentionDays": memoryRetentionDays,
            "factCompress": memoryCompress,
        ])
    }

    private func saveGreetingConfig() {
        engine.updateGreetingConfig([
            "enabled": greetingEnabled,
            "idleMinutes": greetingIdleMinutes,
            "cooldownMinutes": greetingCooldownMinutes,
            "maxChars": greetingMaxChars,
        ])
    }

    private func resetFactEditor() {
        editingFactID = ""
        factKey = ""
        factValue = ""
    }

    private var memoryFactFields: some View {
        HStack(spacing: 8) {
            TextField("偏好名称", text: $factKey)
                .textFieldStyle(.roundedBorder)
                .frame(minWidth: 120)
            TextField("偏好内容", text: $factValue)
                .textFieldStyle(.roundedBorder)
                .frame(minWidth: 140)
        }
    }

    private var memoryFactActions: some View {
        HStack(spacing: 8) {
            Button(editingFactID.isEmpty ? "添加" : "更新") {
                guard !factKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                      !factValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
                if editingFactID.isEmpty {
                    engine.rememberFact(key: factKey, value: factValue)
                } else {
                    engine.updateMemoryFact(id: editingFactID, key: factKey, value: factValue)
                }
                resetFactEditor()
            }
            if !editingFactID.isEmpty {
                Button("取消编辑") { resetFactEditor() }
            }
        }
    }

    private func exportMemory() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "petsona-memory.json"
        panel.allowedContentTypes = [.json]
        if panel.runModal() == .OK, let url = panel.url {
            engine.exportMemory(to: url, personaId: engine.text(PETSONA_TEXT_PERSONA_ID))
        }
    }

    private func importMemory() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.json]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let alert = NSAlert()
        alert.messageText = "导入记忆"
        alert.informativeText = "导入会用文件内容覆盖当前人格的偏好与事件，确定继续吗？"
        alert.addButton(withTitle: "导入")
        alert.addButton(withTitle: "取消")
        if alert.runModal() == .alertFirstButtonReturn {
            engine.importMemory(from: url, personaId: engine.text(PETSONA_TEXT_PERSONA_ID))
            resetFactEditor()
        }
    }

    private func importPet() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.folder, .zip]
        if panel.runModal() == .OK, let url = panel.url { engine.importPet(url) }
    }

    private func handleDrop(_ providers: [NSItemProvider]) -> Bool {
        guard let provider = providers.first else { return false }
        provider.loadDataRepresentation(forTypeIdentifier: UTType.fileURL.identifier) { data, _ in
            guard let data, let url = URL(dataRepresentation: data, relativeTo: nil) else { return }
            DispatchQueue.main.async { engine.importPet(url) }
        }
        return true
    }

    private func importCodexPet() {
        showingCodexPets = true
        engine.send(kind: PETSONA_COMMAND_SCAN_CODEX_PETS)
    }

    private func exportPet(_ id: String) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "\(id).zip"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        engine.send(kind: PETSONA_COMMAND_EXPORT_PET, text: id + "\n" + url.path)
    }

    private func deletePet(_ id: String) {
        let alert = NSAlert()
        alert.messageText = "删除宠物？"
        alert.informativeText = "将从 Petsona 本地库删除 \(id)，不会删除原始 Codex 文件。"
        alert.addButton(withTitle: "删除")
        alert.addButton(withTitle: "取消")
        if alert.runModal() == .alertFirstButtonReturn { engine.deletePet(id) }
    }

    private func exportPersona(_ id: String) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "\(id).json"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        engine.exportPersona(id, to: url)
    }

    private func importPersona() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowedContentTypes = [.json]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        engine.importPersona(url)
        reloadLater()
    }

    /// Rounded percentages so slider values outside the old preset list still
    /// print something sensible (REQ-S12 keeps the same 7 snapped steps).
    private func scaleLabel(_ value: Double) -> String {
        return String(format: "%.0f%%", (value * 100).rounded())
    }

}

private struct CodexPreview: View {
    let path: String
    let cellWidth: Int
    let cellHeight: Int
    let columns: Int

    var body: some View {
        PreviewImage(path: path,
                     cellWidth: cellWidth,
                     cellHeight: cellHeight,
                     columns: columns)
            .frame(width: 48, height: 48)
            .clipped()
    }
}

private struct PreviewImage: NSViewRepresentable {
    let path: String
    let cellWidth: Int
    let cellHeight: Int
    let columns: Int

    func makeNSView(context: Context) -> PreviewImageView { PreviewImageView() }

    func updateNSView(_ view: PreviewImageView, context: Context) {
        view.image = PreviewImageCache.image(at: path)
        view.cellWidth = cellWidth
        view.cellHeight = cellHeight
        view.columns = columns
        view.needsDisplay = true
    }
}

@MainActor
private enum PreviewImageCache {
    private static let images = NSCache<NSString, NSImage>()

    static func image(at path: String) -> NSImage? {
        let key = path as NSString
        if let cached = images.object(forKey: key) {
            return cached
        }
        guard let image = NSImage(contentsOfFile: path) else { return nil }
        images.setObject(image, forKey: key)
        return image
    }
}

private final class PreviewImageView: NSView {
    var image: NSImage?
    var cellWidth = 64
    var cellHeight = 64
    var columns = 8

    override func draw(_ dirtyRect: NSRect) {
        guard let image else {
            NSColor.clear.setFill()
            dirtyRect.fill()
            return
        }
        let source = NSRect(x: 0,
                            y: max(0, image.size.height - CGFloat(cellHeight)),
                            width: CGFloat(cellWidth),
                            height: CGFloat(cellHeight))
        image.draw(in: bounds, from: source, operation: .sourceOver, fraction: 1)
    }
}
