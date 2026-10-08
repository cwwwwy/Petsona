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
    var credentialStatusLabel: String {
        keyConfigured ? "已配置（密钥不会显示）" : "未配置"
    }
}

private struct MemoryConfigProjection: Decodable {
    var enabled = true
}

private struct GreetingConfigProjection: Decodable {
    var enabled = true
    var idleMinutes = 30
}

private struct MemoryFactProjection: Decodable, Identifiable {
    let id: String
    let key: String
    let value: String
    let confidence: Double
    let createdAt: Int64
    let updatedAt: Int64
    var source: String?
    var archived = false

    enum CodingKeys: String, CodingKey {
        case id, key, value, confidence, createdAt, updatedAt, source, archived
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(String.self, forKey: .id)
        key = try values.decode(String.self, forKey: .key)
        value = try values.decode(String.self, forKey: .value)
        confidence = try values.decodeIfPresent(Double.self, forKey: .confidence) ?? 0.0
        createdAt = try values.decodeIfPresent(Int64.self, forKey: .createdAt) ?? 0
        updatedAt = try values.decodeIfPresent(Int64.self, forKey: .updatedAt) ?? 0
        source = try values.decodeIfPresent(String.self, forKey: .source)
        archived = try values.decodeIfPresent(Bool.self, forKey: .archived) ?? false
    }

    /// Where the fact came from (REQ-P06); older files have no source.
    var sourceLabel: String {
        switch source {
        case "conversation": return "对话"
        case "import": return "导入"
        case "compressed": return "压缩"
        default: return "手动"
        }
    }

    var updatedLabel: String {
        guard updatedAt > 0 else { return "时间未知" }
        return Date(timeIntervalSince1970: Double(updatedAt) / 1_000)
            .formatted(date: .abbreviated, time: .shortened)
    }
}

private struct MemoryProjection: Decodable {
    var config = MemoryConfigProjection()
    var greeting = GreetingConfigProjection()
    var facts: [MemoryFactProjection] = []
    var archivedFacts: [MemoryFactProjection] = []
    var candidates: [MemoryCandidateProjection] = []
    var learning = false
}

private struct MemoryCandidateProjection: Decodable, Identifiable {
    let id: String
    let key: String
    let value: String
    let confidence: Double
    let evidence: [String]
    let status: String
}

private extension MemoryProjection {
    var allFacts: [MemoryFactProjection] {
        facts + archivedFacts.map { fact in
            var archived = fact
            archived.archived = true
            return archived
        }
    }
    var pendingCandidates: [MemoryCandidateProjection] {
        candidates.filter { $0.status == "pending" }
    }
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

    var windowTitle: String {
        "Petsona 设置 — \(title)"
    }
}

@MainActor
final class SettingsNavigationState: ObservableObject {
    static let lastSectionDefaultsKey = "Petsona.settings.lastSection"

    @Published var selection: SettingsSection {
        didSet {
            guard oldValue != selection else { return }
            defaults.set(selection.rawValue, forKey: Self.lastSectionDefaultsKey)
            onSelectionChange?(selection)
        }
    }
    @Published var columnVisibility: NavigationSplitViewVisibility = .all
    var onSelectionChange: ((SettingsSection) -> Void)?

    private let defaults: UserDefaults

    init(defaults: UserDefaults? = nil) {
        let defaults = defaults ?? Self.resolveDefaults()
        self.defaults = defaults
        let savedSection = defaults.string(forKey: Self.lastSectionDefaultsKey)
        self._selection = Published(initialValue: savedSection.flatMap(SettingsSection.init(rawValue:)) ?? .library)
    }

    private static func resolveDefaults() -> UserDefaults {
        if let home = ProcessInfo.processInfo.environment["PETSONA_HOME"], !home.isEmpty {
            return UserDefaults(suiteName: "com.petsona.desktop.acceptance") ?? .standard
        }
        return .standard
    }

    func toggleSidebar() {
        withAnimation(.easeInOut(duration: 0.2)) {
            columnVisibility = columnVisibility == .all ? .detailOnly : .all
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
}

private enum SettingsApplyKind: Hashable {
    case persona
    case deepSeek
    case memory
    case greeting
}

struct SettingsView: View {
    @ObservedObject var engine: EngineClient
    @ObservedObject var navigation: SettingsNavigationState
    var onOpenConversationHistory: (() -> Void)? = nil

    @State private var showingPersonaSource = false
    @State private var personaSourceMode = PersonaSourceMode.chatImport
    @State private var scale = 1.0
    @State private var clickThrough = true
    @State private var alwaysOnTop = true
    @State private var showingCodexPets = false
    @State private var dropTargeted = false
    @State private var selectedPetID: String?
    @State private var selectedCodexPetID: String?
    @State private var selectedFactID: String?

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
    @State private var deepSeekKey = ""

    @State private var memoryEnabled = true

    @State private var greetingEnabled = true
    @State private var greetingIdleMinutes = 30

    /// Signatures captured when the form is filled from the engine, so that a
    /// reload never looks like a user edit (instant apply, REQ-S05).
    @State private var loadedPersonaSignature = ""
    @State private var loadedDeepSeekSignature = ""
    @State private var loadedMemorySignature = ""
    @State private var loadedGreetingSignature = ""
    @State private var pendingApplies: [SettingsApplyKind: Task<Void, Never>] = [:]
    @State private var factKey = ""
    @State private var factValue = ""
    @State private var editingFactID = ""
    @State private var autostart = false
    @State private var confirmClearConversationHistory = false
    private var pets: [PetChoice] { decode(PETSONA_TEXT_PETS, as: [PetChoice].self) ?? [] }
    private var codexPets: [CodexPetChoice] { decode(PETSONA_TEXT_CODEX_PETS, as: [CodexPetChoice].self) ?? [] }
    private var selectedPet: PetChoice? { pets.first { $0.id == selectedPetID } }
    private var selectedCodexPet: CodexPetChoice? { codexPets.first { $0.id == selectedCodexPetID } }
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
    private var modelFetchBusy: Bool {
        engine.statusMessage.hasPrefix("正在拉取")
    }

    private var availableModels: [String] {
        decode(PETSONA_TEXT_MODELS, as: [String].self) ?? []
    }

    private var memory: MemoryProjection {
        decode(PETSONA_TEXT_MEMORY, as: MemoryProjection.self) ?? MemoryProjection()
    }
    private var selectedFact: MemoryFactProjection? {
        memory.facts.first { $0.id == selectedFactID }
    }
    private var importConflict: ImportConflictProjection? {
        decode(PETSONA_TEXT_IMPORT_CONFLICT, as: ImportConflictProjection.self)
    }

    var body: some View {
        NavigationSplitView(columnVisibility: $navigation.columnVisibility) {
            List(SettingsSection.allCases, selection: $navigation.selection) { section in
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
        .onChange(of: engine.text(PETSONA_TEXT_PET_ID)) { oldID, newID in
            guard oldID != newID else { return }
            pendingApplies[.persona]?.cancel()
            resetFactEditor()
            reloadPersona()
        }
        .sheet(isPresented: $showingPersonaSource) {
            PersonaSourceView(engine: engine, initialMode: personaSourceMode)
        }
    }

    @ViewBuilder
    private var settingsDetail: some View {
        Form {
            if !engine.errorMessage.isEmpty {
                Section("错误") {
                    Label(engine.errorMessage, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .textSelection(.enabled)
                }
            }
            switch navigation.selection {
            case .library:
                petLibrarySection
            case .behavior:
                behaviorSection
            case .deepSeek:
                deepSeekSection
            case .persona:
                personaSection
            case .memory:
                memorySection
            case .startup:
                startupSection
            }
            if !engine.statusMessage.isEmpty {
                Section("状态") {
                    Text(engine.statusMessage)
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                }
            }
        }
        .formStyle(.grouped)
        .scrollEdgeEffectHidden(true, for: .top)
        .frame(maxWidth: SettingsLayout.contentMaxWidth)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    @ViewBuilder
    private func settingsCard<Content: View>(_ title: String,
                                              @ViewBuilder content: () -> Content) -> some View {
        Section {
            content()
        } header: {
            Text(title)
        }
    }

    @ViewBuilder
    private func settingsRow<Control: View>(
        _ title: String,
        description: String? = nil,
        @ViewBuilder control: () -> Control
    ) -> some View {
        LabeledContent {
            control()
        } label: {
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                if let description {
                    Text(description)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    @ViewBuilder
    private var petLibrarySection: some View {
        settingsCard("本地宠物") {
            Text("双击列表中的宠物即可切换；也可先选中，再使用下方按钮。")
                .font(.footnote)
                .foregroundStyle(.secondary)
            if pets.isEmpty {
                ContentUnavailableView("本地库为空", systemImage: "pawprint",
                                       description: Text("导入宠物文件夹或 .zip，或从 Codex 候选中选择。"))
            } else {
                List(pets, selection: $selectedPetID) { pet in
                    HStack(spacing: 12) {
                        if let spritesheet = pet.spritesheet,
                           let cellWidth = pet.cellWidth,
                           let cellHeight = pet.cellHeight {
                            CodexPreview(path: spritesheet,
                                         cellWidth: cellWidth,
                                         cellHeight: cellHeight)
                        } else {
                            Image(systemName: "pawprint.fill")
                                .font(.title2)
                                .foregroundStyle(.tertiary)
                                .frame(width: 48, height: 48)
                        }
                        VStack(alignment: .leading, spacing: 3) {
                            Text(pet.name)
                            Text(pet.id).font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        if pet.v2 { Text("V2").foregroundStyle(.secondary) }
                        if engine.text(PETSONA_TEXT_PET_ID) == pet.id {
                            Label("当前", systemImage: "checkmark.circle.fill")
                                .foregroundStyle(.secondary)
                        }
                    }
                    .contentShape(Rectangle())
                    .tag(pet.id)
                    .simultaneousGesture(TapGesture(count: 2).onEnded {
                        selectedPetID = pet.id
                        switchPet(to: pet.id)
                    })
                }
                .frame(height: min(CGFloat(pets.count) * 60 + 16, 220))
                .accessibilityLabel("本地宠物")
            }
            HStack {
                Button("导入…") { importPet() }
                Button("切换") {
                    if let selectedPet { switchPet(to: selectedPet.id) }
                }
                .disabled(selectedPet == nil || selectedPet?.id == engine.text(PETSONA_TEXT_PET_ID))
                Button("导出…") {
                    if let selectedPet { exportPet(selectedPet.id) }
                }
                .disabled(selectedPet == nil)
                Button("删除", role: .destructive) {
                    if let selectedPet { deletePet(selectedPet.id) }
                }
                .disabled(selectedPet == nil)
            }
            GroupBox {
                Text("将宠物文件夹或 .zip 拖到这里导入")
                    .foregroundStyle(dropTargeted ? .primary : .secondary)
                    .frame(maxWidth: .infinity, minHeight: 36)
                    .contentShape(Rectangle())
                    .onDrop(of: [UTType.fileURL], isTargeted: $dropTargeted, perform: handleDrop)
            } label: {
                Label("拖放导入", systemImage: "arrow.down.doc")
            }
        }

        if let conflict = importConflict {
            settingsCard("导入冲突") {
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
        }

        settingsCard("从 Codex 导入") {
            Text("只读取 Codex 宠物候选；双击候选才会复制到 Petsona 本地库。")
                .font(.footnote)
                .foregroundStyle(.secondary)
            HStack {
                Button(showingCodexPets ? "重新扫描" : "从 Codex 导入…") { importCodexPet() }
                if showingCodexPets {
                    Button("导入选中") {
                        if let selectedCodexPet {
                            engine.importPet(URL(fileURLWithPath: selectedCodexPet.path))
                        }
                    }
                    .disabled(selectedCodexPet == nil)
                }
            }
            if showingCodexPets {
                if codexPets.isEmpty {
                    ContentUnavailableView("没有发现 Codex 宠物", systemImage: "pawprint",
                                           description: Text("请确认 ~/.codex/pets 中已有宠物包，然后重新扫描。"))
                } else {
                    List(codexPets, selection: $selectedCodexPetID) { pet in
                        HStack {
                            CodexPreview(path: pet.spritesheet,
                                         cellWidth: pet.cellWidth,
                                         cellHeight: pet.cellHeight)
                            VStack(alignment: .leading) {
                                Text(pet.name)
                                Text(pet.id).font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            if pet.v2 { Text("V2").foregroundStyle(.secondary) }
                        }
                        .contentShape(Rectangle())
                        .tag(pet.id)
                        .simultaneousGesture(TapGesture(count: 2).onEnded {
                            selectedCodexPetID = pet.id
                            engine.importPet(URL(fileURLWithPath: pet.path))
                        })
                    }
                    .frame(height: min(CGFloat(codexPets.count) * 60 + 16, 220))
                    .accessibilityLabel("Codex 宠物候选")
                }
            }
        }
    }

    private var personaSection: some View {
        settingsCard("这只宠物的人格") {
            HStack {
                Button("从聊天记录塑造…") {
                    personaSourceMode = .chatImport
                    showingPersonaSource = true
                }
                Button("参考人物…") {
                    personaSourceMode = .publicFigure
                    showingPersonaSource = true
                }
            }
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
            settingsRow("说话方式", description: personaTonePreset.isEmpty
                        ? "描述宠物希望采用的说话方式。"
                        : "预设会同时调整回答的长短。") {
                VStack(alignment: .leading, spacing: 8) {
                    Picker("说话方式", selection: $personaTonePreset) {
                        Text("自定义…").tag("")
                        ForEach(TonePreset.all) { preset in
                            Text(preset.label).tag(preset.tone)
                        }
                    }
                    .frame(maxWidth: 240)
                    if personaTonePreset.isEmpty {
                        TextField("例如：毒舌但温柔", text: $personaTone)
                            .textFieldStyle(.roundedBorder)
                            .frame(maxWidth: 360)
                    }
                }
                .onChange(of: personaTonePreset) { _, tone in
                    guard !tone.isEmpty else { return }
                    personaTone = tone
                    personaVerbosity = TonePreset.all.first { $0.tone == tone }?.verbosity ?? "normal"
                }
            }
            settingsRow("允许 emoji", description: "关闭时会要求模型不要使用 emoji。") {
                Toggle("", isOn: $personaEmoji)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .accessibilityLabel("允许 emoji")
            }
            DisclosureGroup {
                VStack(alignment: .leading, spacing: 8) {
                    TextEditor(text: $systemPrompt)
                        .frame(minHeight: 120)
                        .frame(maxWidth: .infinity)
                        .accessibilityLabel("性格与回应习惯")
                    Text("说话方式会自动结合这里的设定。导入或生成的人格也可以在这里调整。")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            } label: {
                VStack(alignment: .leading, spacing: 3) {
                    Text("性格与回应习惯").font(.headline)
                    Text("补充性格、表达风格和你们的相处方式。")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .onChange(of: personaSignature) { _, value in
            guard value != loadedPersonaSignature else { return }
            scheduleApply(.persona, savePersona)
        }
    }

    @ViewBuilder
    private var deepSeekSection: some View {
        settingsCard("服务商与模型") {
            settingsRow("服务商", description: "选择 DeepSeek 或其他 OpenAI 兼容端点。") {
                Picker("服务商", selection: $deepSeekProvider) {
                    Text("DeepSeek").tag("deepseek")
                    Text("自定义").tag("custom")
                }
                .frame(maxWidth: 240)
                .onChange(of: deepSeekProvider) { _, provider in
                    if provider != "custom" {
                        deepSeekBaseURL = "https://api.deepseek.com/v1"
                    }
                    scheduleApply(.deepSeek, saveDeepSeekConfig)
                }
            }
            settingsRow("Base URL", description: deepSeekProvider == "custom" ? "自定义 OpenAI 兼容端点。" : "DeepSeek 的固定地址。") {
                TextField("Base URL", text: $deepSeekBaseURL)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 180, maxWidth: 420)
                    .disabled(deepSeekProvider != "custom")
            }
            settingsRow("API Key", description: "密钥只保存到 macOS Keychain，不会进入配置快照。") {
                VStack(alignment: .leading, spacing: 8) {
                    SecureField("可选；输入新密钥可覆盖", text: $deepSeekKey)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 360)
                        .accessibilityLabel("API Key")
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
                    }
                    Text(deepSeek.credentialStatusLabel)
                        .foregroundStyle(.secondary)
                }
            }
            settingsRow("模型名称", description: "可以手动填写，也可以从服务商拉取列表。") {
                TextField("模型名称", text: $deepSeekModel)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 360)
            }
            settingsRow("模型列表") {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Button(modelFetchBusy ? "正在拉取…" : "拉取模型列表") { engine.listModels() }
                            .disabled(modelFetchBusy)
                        if modelFetchBusy {
                            ProgressView()
                                .controlSize(.small)
                                .accessibilityLabel("正在拉取模型列表")
                        }
                    }
                    if !availableModels.isEmpty {
                        Picker("选择模型", selection: $deepSeekModel) {
                            ForEach(availableModels, id: \.self) { Text($0).tag($0) }
                        }
                        .frame(maxWidth: 300)
                    }
                }
            }
            if modelFetchFailed {
                Label(engine.statusMessage, systemImage: "exclamationmark.triangle.fill")
                    .font(.footnote)
                    .foregroundStyle(.red)
            }
        }

        .onChange(of: deepSeekSignature) { _, value in
            guard value != loadedDeepSeekSignature else { return }
            scheduleApply(.deepSeek, saveDeepSeekConfig)
        }

    }

    private var memorySection: some View {
        settingsCard("记忆与用户偏好") {
            settingsRow("启用记忆", description: "关闭后停止学习偏好和习惯，也不会把已有长期记忆发送给模型。") {
                Toggle("", isOn: $memoryEnabled)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .accessibilityLabel("启用记忆")
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
            if memory.allFacts.isEmpty {
                ContentUnavailableView("暂无偏好", systemImage: "list.bullet",
                                       description: Text("对话中的明确偏好或手动添加的内容会显示在这里。"))
            } else {
                List(memory.allFacts, selection: $selectedFactID) { fact in
                    VStack(alignment: .leading, spacing: 3) {
                        Text("\(fact.key)：\(fact.value)")
                        Text("\(fact.archived ? "归档 · " : "")来源：\(fact.sourceLabel) · 更新：\(fact.updatedLabel)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
                    .tag(fact.id)
                }
                .frame(height: min(CGFloat(memory.allFacts.count) * 52 + 12, 240))
                .accessibilityLabel("已记录的偏好")
                .onChange(of: selectedFactID) { _, id in
                    guard let id,
                          let fact = memory.allFacts.first(where: { $0.id == id }) else { return }
                    editingFactID = fact.id
                    factKey = fact.key
                    factValue = fact.value
                }
                Button("删除选中偏好", role: .destructive) {
                    if let selectedFact { confirmForgetFact(selectedFact) }
                }
                .disabled(selectedFact == nil)
            }
            if memory.learning {
                ProgressView("正在整理近期习惯…")
                    .controlSize(.small)
            }
            if !memory.pendingCandidates.isEmpty {
                Divider()
                Text("待确认的习惯").font(.headline)
                ForEach(memory.pendingCandidates) { candidate in
                    VStack(alignment: .leading, spacing: 6) {
                        Text("\(candidate.key)：\(candidate.value)")
                            .font(.body)
                        Text("依据以下聊天内容推断")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        ForEach(candidate.evidence, id: \.self) { evidence in
                            Text(evidence)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                        HStack {
                            Button("记住") {
                                engine.reviewMemoryCandidate(candidate.id, accept: true)
                            }
                            Button("忽略", role: .destructive) {
                                engine.reviewMemoryCandidate(candidate.id, accept: false)
                            }
                        }
                    }
                    .padding(.vertical, 6)
                }
            }
            HStack {
                Button("清空这只宠物的记忆", role: .destructive) {
                    confirmMemoryClear(scope: 0, title: "清空记忆？",
                                       message: "这会删除这只宠物保存的全部偏好与互动事件，无法撤销。")
                }
            }
            HStack {
                Button("导出记忆…") { exportMemory() }
                Button("导入记忆…") { importMemory() }
            }
            Text("记忆只保存在这台电脑上；只有配置了模型服务时，对话与记忆片段才会发送给该服务。")
                .font(.footnote)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
            Divider()
            DisclosureGroup("聊天记录") {
                settingsRow("保存聊天历史",
                            description: "按宠物分别保存在本机。关闭后保留本次会话上下文，不删除已有记录。") {
                    Toggle("",
                           isOn: Binding(
                            get: { engine.conversation.saveHistory },
                            set: { engine.updateConversationConfig(saveHistory: $0) }
                           ))
                        .labelsHidden()
                        .toggleStyle(.switch)
                        .accessibilityLabel("保存聊天历史")
                }
                HStack {
                    Button("查看聊天记录") { onOpenConversationHistory?() }
                    Button("清除这只宠物的聊天记录", role: .destructive) {
                        confirmClearConversationHistory = true
                    }
                }
                .confirmationDialog("清除这只宠物的聊天记录？",
                                    isPresented: $confirmClearConversationHistory,
                                    titleVisibility: .visible) {
                    Button("清除聊天记录", role: .destructive) {
                        engine.clearConversationHistory()
                    }
                    Button("取消", role: .cancel) {}
                } message: {
                    Text("此操作不会清除宠物已保存的偏好和习惯。")
                }
            }
        }
        .onChange(of: memorySignature) { _, value in
            guard value != loadedMemorySignature else { return }
            scheduleApply(.memory, saveMemoryConfig)
        }
    }

    @ViewBuilder
    private var behaviorSection: some View {
        settingsCard("窗口与交互") {
            settingsRow("缩放", description: "固定档位 50%–200%，与状态栏菜单保持一致。") {
                HStack {
                    Slider(value: $scale, in: 0.5...2.0, step: 0.25)
                        .frame(maxWidth: 220)
                    Text(scaleLabel(scale)).frame(width: 56, alignment: .trailing)
                }
                .onChange(of: scale) { _, value in
                    engine.send(kind: PETSONA_COMMAND_SET_SCALE, value: value)
                }
            }
            settingsRow("像素级点击穿透", description: "透明像素的点击落到桌面；宠物像素仍可交互。") {
                Toggle("", isOn: $clickThrough)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .accessibilityLabel("像素级点击穿透")
                    .onChange(of: clickThrough) { _, value in
                        engine.send(kind: PETSONA_COMMAND_SET_CLICK_THROUGH, value: value ? 1 : 0)
                    }
            }
            settingsRow("始终置顶", description: "控制宠物窗口是否保持在普通窗口上方。") {
                Toggle("", isOn: $alwaysOnTop)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .accessibilityLabel("始终置顶")
                    .onChange(of: alwaysOnTop) { _, value in
                        engine.send(kind: PETSONA_COMMAND_SET_ALWAYS_ON_TOP, value: value ? 1 : 0)
                    }
                }
            }

        settingsCard("空闲问候") {
            settingsRow("启用空闲问候", description: "长时间没有操作后显示一句短问候。") {
                Toggle("", isOn: $greetingEnabled)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .accessibilityLabel("启用空闲问候")
            }
            if greetingEnabled {
                settingsRow("多久后问候", description: "连续没有点击、拖动或输入的时长。") {
                    Stepper("\(greetingIdleMinutes) 分钟", value: $greetingIdleMinutes, in: 1...1440)
                        .frame(width: 180, alignment: .trailing)
                }
            }
        }
        .onChange(of: greetingSignature) { _, value in
            guard value != loadedGreetingSignature else { return }
            scheduleApply(.greeting, saveGreetingConfig)
        }

    }

    @ViewBuilder
    private var startupSection: some View {
        settingsCard("启动") {
            settingsRow("登录时启动 Petsona", description: "通过 macOS LaunchAgent 在登录后启动。") {
                Toggle("", isOn: $autostart)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .accessibilityLabel("登录时启动 Petsona")
                    .onChange(of: autostart) { _, value in
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
            Text("最低要求：macOS 26")
                .font(.footnote)
                .foregroundStyle(.secondary)
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

        reloadPersona()

        let deepSeek = deepSeek
        deepSeekProvider = deepSeek.provider == "custom" ? "custom" : "deepseek"
        deepSeekBaseURL = deepSeek.baseUrl
        deepSeekModel = deepSeek.model
        loadedDeepSeekSignature = deepSeekSignature

        let memory = memory
        memoryEnabled = memory.config.enabled
        loadedMemorySignature = memorySignature

        let greeting = memory.greeting
        greetingEnabled = greeting.enabled
        greetingIdleMinutes = greeting.idleMinutes
        loadedGreetingSignature = greetingSignature
    }

    private func reloadPersona() {
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
    }

    // Instant apply: every field change schedules one debounced command and
    // never fires for values that came from a reload (settings-consolidation S05).
    private func scheduleApply(_ kind: SettingsApplyKind, _ action: @escaping () -> Void) {
        pendingApplies[kind]?.cancel()
        let targetPetID = kind == .persona ? engine.text(PETSONA_TEXT_PET_ID) : nil
        pendingApplies[kind] = Task { @MainActor in
            try? await Task.sleep(nanoseconds: 450_000_000)
            guard !Task.isCancelled else { return }
            if let targetPetID, engine.text(PETSONA_TEXT_PET_ID) != targetPetID { return }
            action()
        }
    }

    private var personaSignature: String {
        [personaName, personaTone, personaVerbosity, personaEmoji ? "1" : "0",
         greeting, systemPrompt].joined(separator: "\u{1F}")
    }

    private var deepSeekSignature: String {
        [deepSeekProvider, deepSeekBaseURL, deepSeekModel].joined(separator: "\u{1F}")
    }

    private var memorySignature: String {
        memoryEnabled ? "1" : "0"
    }

    private var greetingSignature: String {
        [greetingEnabled ? "1" : "0", String(greetingIdleMinutes)].joined(separator: "\u{1F}")
    }

    private func reloadPersonaLater() {
        let targetPetID = engine.text(PETSONA_TEXT_PET_ID)
        let submittedSignature = personaSignature
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 180_000_000)
            guard engine.text(PETSONA_TEXT_PET_ID) == targetPetID,
                  personaSignature == submittedSignature else { return }
            reloadPersona()
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
        reloadPersonaLater()
    }

    private func saveDeepSeekConfig() {
        engine.updateDeepSeekConfig([
            "provider": deepSeekProvider,
            "baseUrl": deepSeekBaseURL,
            "model": deepSeekModel,
        ])
    }

    private func saveMemoryConfig() {
        engine.updateMemoryConfig([
            "enabled": memoryEnabled,
        ])
    }

    private func saveGreetingConfig() {
        engine.updateGreetingConfig([
            "enabled": greetingEnabled,
            "idleMinutes": greetingIdleMinutes,
        ])
    }

    private func resetFactEditor() {
        editingFactID = ""
        factKey = ""
        factValue = ""
        selectedFactID = nil
    }

    private func confirmForgetFact(_ fact: MemoryFactProjection) {
        let alert = NSAlert()
        alert.messageText = "删除这条偏好？"
        alert.informativeText = "将删除“\(fact.key)：\(fact.value)”，此操作无法撤销。"
        alert.addButton(withTitle: "删除")
        alert.addButton(withTitle: "取消")
        if alert.runModal() == .alertFirstButtonReturn {
            engine.forgetFact(fact.id)
            if selectedFactID == fact.id { selectedFactID = nil }
            if editingFactID == fact.id { resetFactEditor() }
        }
    }

    private func confirmMemoryClear(scope: Int, title: String, message: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.addButton(withTitle: "清空")
        alert.addButton(withTitle: "取消")
        if alert.runModal() == .alertFirstButtonReturn {
            engine.clearMemory(scope: scope)
            resetFactEditor()
        }
    }

    private var memoryFactFields: some View {
        HStack(spacing: 8) {
            TextField("偏好名称", text: $factKey)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 220)
            TextField("偏好内容", text: $factValue)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)
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

    private func switchPet(to id: String) {
        guard engine.text(PETSONA_TEXT_PET_ID) != id else { return }
        pendingApplies[.persona]?.cancel()
        if personaSignature != loadedPersonaSignature { savePersona() }
        engine.selectPet(id)
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
        reloadPersonaLater()
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

    var body: some View {
        PreviewImage(path: path, cellWidth: cellWidth, cellHeight: cellHeight)
            .frame(width: 48, height: 48)
            .clipped()
    }
}

private struct PreviewImage: NSViewRepresentable {
    let path: String
    let cellWidth: Int
    let cellHeight: Int

    func makeNSView(context: Context) -> PreviewImageView { PreviewImageView() }

    func updateNSView(_ view: PreviewImageView, context: Context) {
        view.image = PreviewImageCache.image(at: path)
        view.cellWidth = cellWidth
        view.cellHeight = cellHeight
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
