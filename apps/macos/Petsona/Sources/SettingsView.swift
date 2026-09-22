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

private struct PersonaChoice: Decodable, Identifiable {
    let id: String
    let name: String
    let description: String?
    let builtin: Bool
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
        language = try values.decodeIfPresent(String.self, forKey: .language) ?? language
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

private struct DeepSeekProjection: Decodable {
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
}

private struct MemoryConfigProjection: Decodable {
    var enabled = true
    var recentEvents = 5
    var factLimit = 20
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
    case deepSeek
    case persona
    case memory
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
        case .deepSeek: return "sparkles"
        case .persona: return "person.crop.circle"
        case .memory: return "clock.arrow.circlepath"
        case .startup: return "power"
        }
    }
}

struct SettingsView: View {
    @ObservedObject var engine: EngineClient

    private let personaTemplates = [
        ("", "空白人格"),
        ("genki", "元气助手"),
        ("snark", "毒舌吐槽"),
        ("advisor", "沉稳顾问"),
    ]

    @State private var scale = 1.0
    @State private var clickThrough = true
    @State private var autoWalk = true
    @State private var gravity = false
    @State private var alwaysOnTop = true
    @State private var showingCodexPets = false
    @State private var dropTargeted = false

    @State private var personaID = "default"
    @State private var personaName = ""
    @State private var personaTone = ""
    @State private var personaTonePreset = ""
    @State private var personaVerbosity = "normal"
    @State private var personaEmoji = false
    @State private var greeting = ""
    @State private var systemPrompt = ""
    @State private var showingNewPersona = false

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
    private var personas: [PersonaChoice] { decode(PETSONA_TEXT_PERSONAS, as: [PersonaChoice].self) ?? [] }
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
            .navigationTitle("Petsona 设置")
            .navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 260)
        } detail: {
            settingsDetail
        }
        .frame(minWidth: 920, minHeight: 680)
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
        Form {
            content()
            if !engine.errorMessage.isEmpty {
                Section("错误") {
                    Text(engine.errorMessage).foregroundStyle(.red)
                }
            }
            if !engine.statusMessage.isEmpty {
                Section {
                    Text(engine.statusMessage)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle(title)
        .padding(.vertical, 8)
    }

    private var petLibrarySection: some View {
        Section("宠物库") {
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
                HStack {
                    if let spritesheet = pet.spritesheet,
                       let cellWidth = pet.cellWidth,
                       let cellHeight = pet.cellHeight {
                        CodexPreview(path: spritesheet,
                                     cellWidth: cellWidth,
                                     cellHeight: cellHeight,
                                     columns: 8)
                    }
                    Text(pet.name)
                    if pet.v2 { Text("V2").foregroundStyle(.secondary) }
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
        }
    }

    private var personaSection: some View {
        Section("人格管理") {
            HStack {
                Picker("当前人格", selection: $personaID) {
                    ForEach(personas) { persona in
                        Text(persona.name).tag(persona.id)
                    }
                }
                .onChange(of: personaID) { id in
                    guard id != engine.text(PETSONA_TEXT_PERSONA_ID) else { return }
                    engine.selectPersona(id)
                    reloadLater()
                }
                Button("新建") { showingNewPersona = true }
                Button("复制") { duplicateCurrentPersona() }
                Button("导出") { exportPersona(personaID) }
                Button("导入") { importPersona() }
                Button("删除") { deletePersona(personaID) }
            }
            TextField("人格 ID", text: $personaID).disabled(true)
            TextField("名称", text: $personaName)
            TextField("语气", text: $personaTone)
            Picker("语气预设", selection: $personaTonePreset) {
                Text("自定义…").tag("")
                ForEach(TonePreset.all) { preset in
                    Text(preset.label).tag(preset.tone)
                }
            }
            .onChange(of: personaTonePreset) { tone in
                guard !tone.isEmpty else { return }
                personaTone = tone
                personaVerbosity = TonePreset.all.first { $0.tone == tone }?.verbosity ?? "normal"
            }
            Toggle("允许 emoji", isOn: $personaEmoji)
            DisclosureGroup("高级（系统提示词）") {
                TextEditor(text: $systemPrompt).frame(minHeight: 120)
                Text("语气 / emoji / 语言会由上面的设置自动追加，不需要在这里重复。")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .onChange(of: personaSignature) { value in
            guard value != loadedPersonaSignature else { return }
            scheduleApply(savePersona)
        }
        .sheet(isPresented: $showingNewPersona) {
            NewPersonaView(templates: personaTemplates) { id, name, template in
                engine.createPersona(id: id, name: name, template: template)
                showingNewPersona = false
                reloadLater()
            }
            .frame(width: 360, height: 220)
            .padding()
        }
    }

    private var deepSeekSection: some View {
        Section("模型服务") {
            Picker("服务商", selection: $deepSeekProvider) {
                Text("DeepSeek").tag("deepseek")
                Text("自定义").tag("custom")
            }
            .onChange(of: deepSeekProvider) { provider in
                if provider != "custom" {
                    deepSeekBaseURL = "https://api.deepseek.com/v1"
                }
                scheduleApply(saveDeepSeekConfig)
            }
            TextField("Base URL", text: $deepSeekBaseURL)
                .disabled(deepSeekProvider != "custom")
            SecureField("API Key（可选）", text: $deepSeekKey)
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
                Text(deepSeek.keyConfigured ? "已配置（密钥不会显示）" : "未配置")
                    .foregroundStyle(.secondary)
            }
            TextField("模型", text: $deepSeekModel)
            HStack {
                Button("拉取模型列表") { engine.listModels() }
                if !availableModels.isEmpty {
                    Picker("选择模型", selection: $deepSeekModel) {
                        ForEach(availableModels, id: \.self) { Text($0).tag($0) }
                    }
                }
            }
            if modelFetchFailed {
                Text(engine.statusMessage).font(.footnote).foregroundStyle(.red)
            }
        }

        Section("高级") {
            TextField("API Key 环境变量", text: $deepSeekAPIKeyEnv)
            Stepper("超时：\(deepSeekTimeout) 秒", value: $deepSeekTimeout, in: 5...120)
            Stepper("最大 token：\(deepSeekMaxTokens)", value: $deepSeekMaxTokens, in: 16...4000, step: 16)
            HStack {
                Text("温度")
                Slider(value: $deepSeekTemperature, in: 0...2, step: 0.1)
                Text(String(format: "%.1f", deepSeekTemperature)).frame(width: 40)
            }
            if deepSeekProvider == "custom" {
                Text("自定义端点不会收到 DeepSeek 专有的 thinking 字段。")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            } else {
                Toggle("关闭思考模式（短回复更快）", isOn: $deepSeekThinkingDisabled)
            }
        }
        .onChange(of: deepSeekSignature) { value in
            guard value != loadedDeepSeekSignature else { return }
            scheduleApply(saveDeepSeekConfig)
        }

    }

    private var memorySection: some View {
        Section("记忆与用户偏好") {
            Toggle("启用记忆", isOn: $memoryEnabled)
            Stepper("保留最近事件：\(memoryRecentEvents)", value: $memoryRecentEvents, in: 1...100)
            Stepper("最多偏好：\(memoryFactLimit)", value: $memoryFactLimit, in: 1...50)
            Divider()
            Text("对话中的“我喜欢… / 我不喜欢… / 请叫我…”等明确表达会自动记录，并用于后续回复。")
                .font(.footnote)
                .foregroundStyle(.secondary)
            HStack {
                TextField("偏好名称", text: $factKey)
                TextField("偏好内容", text: $factValue)
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
            ForEach(memory.facts) { fact in
                HStack {
                    Text("\(fact.key)：\(fact.value)")
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
                Button("全部清空") { engine.clearMemory(scope: 0) }
            }
            HStack {
                Button("导出记忆…") { exportMemory() }
                Button("导入记忆…") { importMemory() }
            }
        }
        .onChange(of: memorySignature) { value in
            guard value != loadedMemorySignature else { return }
            scheduleApply(saveMemoryConfig)
        }
    }

    private var behaviorSection: some View {
        Section("行为") {
            HStack {
                Slider(value: $scale, in: 0.5...2.0, step: 0.25)
                Text(scaleLabel(scale)).frame(width: 96, alignment: .trailing)
            }
            .onChange(of: scale) { value in
                engine.send(kind: PETSONA_COMMAND_SET_SCALE, value: value)
            }
            Toggle("像素级点击穿透", isOn: $clickThrough)
                .onChange(of: clickThrough) { value in
                    engine.send(kind: PETSONA_COMMAND_SET_CLICK_THROUGH, value: value ? 1 : 0)
                }
            Toggle("启用活动提醒", isOn: $autoWalk)
                .onChange(of: autoWalk) { value in
                    engine.send(kind: PETSONA_COMMAND_SET_AUTO_WALK, value: value ? 1 : 0)
                }
            Toggle("重力", isOn: $gravity)
                .onChange(of: gravity) { value in
                    engine.send(kind: PETSONA_COMMAND_SET_GRAVITY, value: value ? 1 : 0)
                }
            Toggle("始终置顶", isOn: $alwaysOnTop)
                .onChange(of: alwaysOnTop) { value in
                    engine.send(kind: PETSONA_COMMAND_SET_ALWAYS_ON_TOP, value: value ? 1 : 0)
                }
        }

        Section("空闲问候") {
            Toggle("让宠物主动打招呼", isOn: $greetingEnabled)
            TextField("固定问候文案（无 Key 时使用；留空按时间自动选）", text: $greeting)
            Stepper("空闲 \(greetingIdleMinutes) 分钟后触发", value: $greetingIdleMinutes, in: 1...1440)
            Stepper("两次问候至少间隔 \(greetingCooldownMinutes) 分钟", value: $greetingCooldownMinutes, in: 0...1440)
            Stepper("问候最长 \(greetingMaxChars) 字", value: $greetingMaxChars, in: 1...200)
            Text("没有配置 DeepSeek 时使用人格里的固定问候；问候会显示为气泡并记入记忆。")
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
        .onChange(of: greetingSignature) { value in
            guard value != loadedGreetingSignature else { return }
            scheduleApply(saveGreetingConfig)
        }

        Section("测试") {
            Button("测试问候") {
                engine.send(kind: PETSONA_COMMAND_SHOW_BUBBLE,
                            ttlMilliseconds: 5_000,
                            text: "你好，我在这里")
            }
        }
    }

    private var startupSection: some View {
        Section("启动") {
            Toggle("登录时启动 Petsona", isOn: $autostart)
                .onChange(of: autostart) { value in
                    do {
                        try LaunchAgentService.setEnabled(value)
                    } catch {
                        autostart = LaunchAgentService.isEnabled
                        engine.reportError("自启设置失败：\(error.localizedDescription)")
                    }
                }
        }

        Section("数据") {
            Button("打开数据目录") { openDataDirectory() }
            Text(dataDirectoryPath).font(.footnote).foregroundStyle(.secondary)
        }

        Section("关于") {
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
        autoWalk = engine.snapshot.auto_walk != 0
        gravity = engine.snapshot.gravity_enabled != 0
        alwaysOnTop = engine.snapshot.always_on_top != 0
        autostart = LaunchAgentService.isEnabled

        let persona = currentPersona
        personaID = persona.id
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
        [memoryEnabled ? "1" : "0", String(memoryRecentEvents), String(memoryFactLimit)]
            .joined(separator: "\u{1F}")
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

    private func duplicateCurrentPersona() {
        let baseID = personaID.isEmpty ? "default" : personaID
        engine.duplicatePersona(sourceID: baseID, id: baseID + "-copy", name: personaName + " 副本")
        reloadLater()
    }

    private func deletePersona(_ id: String) {
        let alert = NSAlert()
        alert.messageText = "删除人格？"
        alert.informativeText = "删除后当前人格会回到默认人格。"
        alert.addButton(withTitle: "删除")
        alert.addButton(withTitle: "取消")
        if alert.runModal() == .alertFirstButtonReturn {
            engine.deletePersona(id)
            reloadLater()
        }
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

private struct NewPersonaView: View {
    let templates: [(String, String)]
    let onCreate: (String, String, String?) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var id = ""
    @State private var name = ""
    @State private var template = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("新建人格").font(.headline)
            TextField("ID（英文、数字或短横线）", text: $id)
            TextField("名称", text: $name)
            Picker("模板", selection: $template) {
                ForEach(Array(templates.enumerated()), id: \.offset) { item in
                    Text(item.element.1).tag(item.element.0)
                }
            }
            HStack {
                Spacer()
                Button("取消") { dismiss() }
                Button("创建") {
                    onCreate(id.trimmingCharacters(in: .whitespacesAndNewlines),
                             name.trimmingCharacters(in: .whitespacesAndNewlines),
                             template.isEmpty ? nil : template)
                }
                .keyboardShortcut(.defaultAction)
            }
        }
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
