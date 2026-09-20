import AppKit
import SwiftUI
import UniformTypeIdentifiers

private struct PetChoice: Decodable, Identifiable {
    let id: String
    let name: String
    let v2: Bool
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
    var language = "zh-CN"
    var emoji = false

    enum CodingKeys: String, CodingKey { case tone, verbosity, language, emoji }

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
    var description: String?
    var avatarPet: String?
    var systemPrompt = ""
    var greeting: String?
    var traits = PersonaTraitsProjection(tone: "", verbosity: "normal", language: "zh-CN", emoji: false)
    var sampling = PersonaSamplingProjection()
    var model: PersonaModelProjection?
    var memory = PersonaMemoryProjection()
    var tts = PersonaTTSProjection()
    var proactive = PersonaProactiveProjection()

    enum CodingKeys: String, CodingKey {
        case id, name, description, avatarPet, systemPrompt, greeting
        case traits, sampling, model, memory, tts, proactive
    }

    init() {}

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(String.self, forKey: .id) ?? id
        name = try values.decodeIfPresent(String.self, forKey: .name) ?? name
        description = try values.decodeIfPresent(String.self, forKey: .description)
        avatarPet = try values.decodeIfPresent(String.self, forKey: .avatarPet)
        systemPrompt = try values.decodeIfPresent(String.self, forKey: .systemPrompt) ?? systemPrompt
        greeting = try values.decodeIfPresent(String.self, forKey: .greeting)
        traits = try values.decodeIfPresent(PersonaTraitsProjection.self, forKey: .traits) ?? traits
        sampling = try values.decodeIfPresent(PersonaSamplingProjection.self, forKey: .sampling) ?? sampling
        model = try values.decodeIfPresent(PersonaModelProjection.self, forKey: .model)
        memory = try values.decodeIfPresent(PersonaMemoryProjection.self, forKey: .memory) ?? memory
        tts = try values.decodeIfPresent(PersonaTTSProjection.self, forKey: .tts) ?? tts
        proactive = try values.decodeIfPresent(PersonaProactiveProjection.self, forKey: .proactive) ?? proactive
    }
}

private extension PersonaTraitsProjection {
    init(tone: String, verbosity: String, language: String, emoji: Bool) {
        self.tone = tone
        self.verbosity = verbosity
        self.language = language
        self.emoji = emoji
    }
}

private struct DeepSeekProjection: Decodable {
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
    var retentionDays = 90
    var factLimit = 20
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
    var facts: [MemoryFactProjection] = []
    var events: [MemoryEventProjection] = []
}

private struct ImportConflictProjection: Decodable {
    let id: String
    let name: String
    let path: String
}

struct SettingsView: View {
    @ObservedObject var engine: EngineClient

    private let scalePresets: [Double] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0]
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
    @State private var personaDescription = ""
    @State private var personaAvatarPet = ""
    @State private var personaTone = ""
    @State private var personaVerbosity = "normal"
    @State private var personaLanguage = "zh-CN"
    @State private var personaEmoji = false
    @State private var greeting = ""
    @State private var systemPrompt = ""
    @State private var personaTemperature = 0.8
    @State private var personaMaxTokens = 800
    @State private var modelProvider = ""
    @State private var personaModel = ""
    @State private var personaMemoryEnabled = true
    @State private var personaMemoryWindow = 12
    @State private var personaLongTermMemory = true
    @State private var personaSummarizeAfter = 20
    @State private var ttsEnabled = false
    @State private var ttsVoice = ""
    @State private var ttsRate = 1.0
    @State private var proactiveEnabled = false
    @State private var proactiveIdleMinutes = 30
    @State private var showingNewPersona = false

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
    @State private var memoryRetentionDays = 90
    @State private var memoryFactLimit = 20
    @State private var factKey = ""
    @State private var factValue = ""
    @State private var autostart = false

    private var pets: [PetChoice] { decode(PETSONA_TEXT_PETS, as: [PetChoice].self) ?? [] }
    private var codexPets: [CodexPetChoice] { decode(PETSONA_TEXT_CODEX_PETS, as: [CodexPetChoice].self) ?? [] }
    private var personas: [PersonaChoice] { decode(PETSONA_TEXT_PERSONAS, as: [PersonaChoice].self) ?? [] }
    private var currentPersona: PersonaProjection {
        decode(PETSONA_TEXT_PERSONA, as: PersonaProjection.self) ?? PersonaProjection()
    }
    private var deepSeek: DeepSeekProjection {
        decode(PETSONA_TEXT_DEEPSEEK_CONFIG, as: DeepSeekProjection.self) ?? DeepSeekProjection()
    }
    private var memory: MemoryProjection {
        decode(PETSONA_TEXT_MEMORY, as: MemoryProjection.self) ?? MemoryProjection()
    }
    private var importConflict: ImportConflictProjection? {
        decode(PETSONA_TEXT_IMPORT_CONFLICT, as: ImportConflictProjection.self)
    }

    var body: some View {
        Form {
            petLibrarySection
            personaSection
            deepSeekSection
            memorySection
            behaviorSection
            startupSection

            if !engine.errorMessage.isEmpty {
                Section("错误") {
                    Text(engine.errorMessage).foregroundStyle(.red)
                }
            }
            if !engine.statusMessage.isEmpty {
                Text(engine.statusMessage)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .padding()
        .frame(minWidth: 620, minHeight: 760)
        .onAppear { reloadForm() }
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
                Button("导入宠物…") { importPet() }
                Button("从 Codex 导入…") { importCodexPet() }
                Button("重新扫描") { engine.send(kind: PETSONA_COMMAND_REFRESH_PETS) }
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
            TextField("简介", text: $personaDescription)
            TextField("绑定宠物 ID（可选）", text: $personaAvatarPet)
            TextField("语气", text: $personaTone)
            Picker("回答长度", selection: $personaVerbosity) {
                Text("简短").tag("short")
                Text("正常").tag("normal")
                Text("详细").tag("detailed")
            }
            TextField("语言", text: $personaLanguage)
            Toggle("允许 emoji", isOn: $personaEmoji)
            TextField("固定问候（可选）", text: $greeting)
            TextEditor(text: $systemPrompt).frame(minHeight: 90)
            HStack {
                Text("温度")
                Slider(value: $personaTemperature, in: 0...2, step: 0.1)
                Text(String(format: "%.1f", personaTemperature)).frame(width: 40)
            }
            Stepper("最大回复 token：\(personaMaxTokens)", value: $personaMaxTokens, in: 16...4000, step: 16)
            TextField("模型提供方（可选）", text: $modelProvider)
            TextField("绑定模型（可选）", text: $personaModel)
            Toggle("启用人格记忆", isOn: $personaMemoryEnabled)
            Stepper("对话记忆窗口：\(personaMemoryWindow) 轮", value: $personaMemoryWindow, in: 1...100)
            Toggle("启用长期记忆", isOn: $personaLongTermMemory)
            Stepper("总结阈值：\(personaSummarizeAfter) 轮", value: $personaSummarizeAfter, in: 1...200)
            Toggle("启用语音配置", isOn: $ttsEnabled)
            TextField("语音名称（可选）", text: $ttsVoice)
            HStack {
                Text("语速")
                Slider(value: $ttsRate, in: 0.25...4.0, step: 0.05)
                Text(String(format: "%.2f", ttsRate)).frame(width: 46)
            }
            Toggle("启用主动问候配置", isOn: $proactiveEnabled)
            Stepper("主动问候空闲：\(proactiveIdleMinutes) 分钟", value: $proactiveIdleMinutes, in: 1...1440)
            Button("保存人格") { savePersona() }
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
        Section("DeepSeek") {
            TextField("Base URL", text: $deepSeekBaseURL)
            TextField("模型", text: $deepSeekModel)
            TextField("API Key 环境变量", text: $deepSeekAPIKeyEnv)
            Stepper("超时：\(deepSeekTimeout) 秒", value: $deepSeekTimeout, in: 5...120)
            Stepper("最大 token：\(deepSeekMaxTokens)", value: $deepSeekMaxTokens, in: 16...4000, step: 16)
            HStack {
                Text("温度")
                Slider(value: $deepSeekTemperature, in: 0...2, step: 0.1)
                Text(String(format: "%.1f", deepSeekTemperature)).frame(width: 40)
            }
            Toggle("关闭思考模式（短回复更快）", isOn: $deepSeekThinkingDisabled)
            Button("保存 DeepSeek 配置") { saveDeepSeekConfig() }
            Divider()
            SecureField("DeepSeek API Key（可选）", text: $deepSeekKey)
            HStack {
                Button("保存密钥") {
                    do {
                        try KeychainService.saveDeepSeekKey(deepSeekKey)
                        deepSeekKey = ""
                        engine.reportError("")
                    } catch {
                        engine.reportError(error.localizedDescription)
                    }
                }
                Button("清除密钥") {
                    do { try KeychainService.deleteDeepSeekKey() }
                    catch { engine.reportError(error.localizedDescription) }
                }
                Text(KeychainService.hasDeepSeekKey ? "已配置" : "未配置")
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var memorySection: some View {
        Section("记忆与用户偏好") {
            Toggle("启用记忆", isOn: $memoryEnabled)
            Stepper("保留最近事件：\(memoryRecentEvents)", value: $memoryRecentEvents, in: 1...100)
            Stepper("保留天数：\(memoryRetentionDays)", value: $memoryRetentionDays, in: 1...3650)
            Stepper("最多偏好：\(memoryFactLimit)", value: $memoryFactLimit, in: 1...50)
            Button("保存记忆设置") { saveMemoryConfig() }
            Divider()
            Text("对话中的“我喜欢… / 我不喜欢… / 请叫我…”等明确表达会自动记录，并用于后续回复。")
                .font(.footnote)
                .foregroundStyle(.secondary)
            HStack {
                TextField("偏好名称", text: $factKey)
                TextField("偏好内容", text: $factValue)
                Button("添加") {
                    guard !factKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                          !factValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
                    engine.rememberFact(key: factKey, value: factValue)
                    factKey = ""
                    factValue = ""
                }
            }
            ForEach(memory.facts) { fact in
                HStack {
                    Text("\(fact.key)：\(fact.value)")
                    Spacer()
                    Button("删除") { engine.forgetFact(fact.id) }
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
            Button("清空当前人格记忆") { engine.clearMemory() }
        }
    }

    private var behaviorSection: some View {
        Section("行为") {
            Picker("大小", selection: $scale) {
                ForEach(scalePresets, id: \.self) { value in
                    Text(scaleLabel(value)).tag(value)
                }
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
        personaDescription = persona.description ?? ""
        personaAvatarPet = persona.avatarPet ?? ""
        personaTone = persona.traits.tone
        personaVerbosity = persona.traits.verbosity
        personaLanguage = persona.traits.language
        personaEmoji = persona.traits.emoji
        greeting = persona.greeting ?? ""
        systemPrompt = persona.systemPrompt
        personaTemperature = persona.sampling.temperature
        personaMaxTokens = persona.sampling.maxTokens
        modelProvider = persona.model?.provider ?? ""
        personaModel = persona.model?.model ?? ""
        personaMemoryEnabled = persona.memory.enabled
        personaMemoryWindow = persona.memory.windowTurns
        personaLongTermMemory = persona.memory.longTerm
        personaSummarizeAfter = persona.memory.summarizeAfterTurns
        ttsEnabled = persona.tts.enabled
        ttsVoice = persona.tts.voice ?? ""
        ttsRate = persona.tts.rate
        proactiveEnabled = persona.proactive.enabled
        proactiveIdleMinutes = persona.proactive.idleMinutes

        let deepSeek = deepSeek
        deepSeekBaseURL = deepSeek.baseUrl
        deepSeekModel = deepSeek.model
        deepSeekAPIKeyEnv = deepSeek.apiKeyEnv
        deepSeekTimeout = deepSeek.timeoutSeconds
        deepSeekMaxTokens = deepSeek.maxTokens
        deepSeekTemperature = deepSeek.temperature
        deepSeekThinkingDisabled = deepSeek.thinkingDisabled

        let memory = memory
        memoryEnabled = memory.config.enabled
        memoryRecentEvents = memory.config.recentEvents
        memoryRetentionDays = memory.config.retentionDays
        memoryFactLimit = memory.config.factLimit
    }

    private func reloadLater() {
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 180_000_000)
            reloadForm()
        }
    }

    private func savePersona() {
        engine.updatePersona(fields: [
            "description": personaDescription,
            "avatar_pet": personaAvatarPet,
            "name": personaName,
            "tone": personaTone,
            "verbosity": personaVerbosity,
            "language": personaLanguage,
            "emoji": personaEmoji,
            "greeting": greeting,
            "system_prompt": systemPrompt,
            "temperature": personaTemperature,
            "max_tokens": personaMaxTokens,
            "model_provider": modelProvider,
            "model": personaModel,
            "memory_enabled": personaMemoryEnabled,
            "memory_window_turns": personaMemoryWindow,
            "memory_long_term": personaLongTermMemory,
            "memory_summarize_after_turns": personaSummarizeAfter,
            "tts_enabled": ttsEnabled,
            "tts_voice": ttsVoice,
            "tts_rate": ttsRate,
            "proactive_enabled": proactiveEnabled,
            "proactive_idle_minutes": proactiveIdleMinutes,
        ])
        reloadLater()
    }

    private func saveDeepSeekConfig() {
        engine.updateDeepSeekConfig([
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
            "retentionDays": memoryRetentionDays,
            "factLimit": memoryFactLimit,
        ])
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

    private func scaleLabel(_ value: Double) -> String {
        switch value {
        case 0.5: return "小（50%）"
        case 0.75: return "较小（75%）"
        case 1.0: return "中（100%）"
        case 1.25: return "较大（125%）"
        case 1.5: return "大（150%）"
        case 1.75: return "特大（175%）"
        default: return "超大（200%）"
        }
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
        view.image = NSImage(contentsOfFile: path)
        view.cellWidth = cellWidth
        view.cellHeight = cellHeight
        view.columns = columns
        view.needsDisplay = true
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
