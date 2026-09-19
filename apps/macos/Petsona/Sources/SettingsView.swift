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

struct SettingsView: View {
    @ObservedObject var engine: EngineClient
    @State private var scale = 1.0
    @State private var clickThrough = true
    @State private var autoWalk = true
    @State private var gravity = false
    @State private var alwaysOnTop = true
    @State private var personaName = ""
    @State private var personaTone = ""
    @State private var personaLanguage = ""
    @State private var greeting = ""
    @State private var systemPrompt = ""
    @State private var autostart = false
    @State private var deepSeekKey = ""
    @State private var showingCodexPets = false

    private var pets: [PetChoice] {
        guard let data = engine.text(PETSONA_TEXT_PETS).data(using: .utf8) else { return [] }
        return (try? JSONDecoder().decode([PetChoice].self, from: data)) ?? []
    }

    private var codexPets: [CodexPetChoice] {
        guard let data = engine.text(PETSONA_TEXT_CODEX_PETS).data(using: .utf8) else { return [] }
        return (try? JSONDecoder().decode([CodexPetChoice].self, from: data)) ?? []
    }

    var body: some View {
        Form {
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
                    Button("重新扫描") {
                        engine.send(kind: PETSONA_COMMAND_REFRESH_PETS)
                    }
                }
                if showingCodexPets {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Codex 宠物（选择后才会复制到 Petsona 本地库）")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                        if codexPets.isEmpty {
                            Text("没有发现可导入的宠物目录。")
                                .foregroundStyle(.secondary)
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
                                Button("导入") {
                                    engine.importPet(URL(fileURLWithPath: pet.path))
                                }
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

            Section("人格") {
                TextField("名字", text: $personaName)
                TextField("语气", text: $personaTone)
                TextField("语言", text: $personaLanguage)
                TextField("固定问候", text: $greeting)
                TextEditor(text: $systemPrompt)
                    .frame(minHeight: 90)
                Button("保存人格") {
                    engine.updatePersona(name: personaName,
                                         tone: personaTone,
                                         language: personaLanguage,
                                         greeting: greeting,
                                         systemPrompt: systemPrompt)
                }
            }

            Section("行为") {
                Slider(value: $scale, in: 0.5...2.0, step: 0.25) {
                    Text("大小")
                } onEditingChanged: { editing in
                    if !editing { engine.send(kind: PETSONA_COMMAND_SET_SCALE, value: scale) }
                }
                Toggle("像素级点击穿透", isOn: $clickThrough)
                    .onChange(of: clickThrough) { value in
                        engine.send(kind: PETSONA_COMMAND_SET_CLICK_THROUGH,
                                    value: value ? 1 : 0)
                    }
                Toggle("启用活动提醒", isOn: $autoWalk)
                    .onChange(of: autoWalk) { value in
                        engine.send(kind: PETSONA_COMMAND_SET_AUTO_WALK,
                                    value: value ? 1 : 0)
                    }
                Toggle("重力", isOn: $gravity)
                    .onChange(of: gravity) { value in
                        engine.send(kind: PETSONA_COMMAND_SET_GRAVITY,
                                    value: value ? 1 : 0)
                    }
                Toggle("始终置顶", isOn: $alwaysOnTop)
                    .onChange(of: alwaysOnTop) { value in
                        engine.send(kind: PETSONA_COMMAND_SET_ALWAYS_ON_TOP,
                                    value: value ? 1 : 0)
                    }
                Button("测试问候") {
                    engine.send(kind: PETSONA_COMMAND_SHOW_BUBBLE,
                                ttlMilliseconds: 5_000,
                                text: "你好，我在这里")
                }
            }

            Section("启动和凭据") {
                Toggle("登录时启动 Petsona", isOn: $autostart)
                    .onChange(of: autostart) { value in
                        do {
                            try LaunchAgentService.setEnabled(value)
                        } catch {
                            autostart = LaunchAgentService.isEnabled
                            engine.reportError("自启设置失败：\(error.localizedDescription)")
                        }
                    }
                SecureField("DeepSeek API Key（可选）", text: $deepSeekKey)
                Button("保存 DeepSeek 密钥") {
                    do {
                        try KeychainService.saveDeepSeekKey(deepSeekKey)
                        deepSeekKey = ""
                    } catch {
                        engine.reportError(error.localizedDescription)
                    }
                }
            }

            if !engine.errorMessage.isEmpty {
                Section("状态") {
                    Text(engine.errorMessage).foregroundStyle(.red)
                }
            }
            if !engine.text(PETSONA_TEXT_STATUS).isEmpty {
                Text(engine.text(PETSONA_TEXT_STATUS))
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .padding()
        .frame(minWidth: 520, minHeight: 640)
        .onAppear { reloadForm() }
    }

    private func reloadForm() {
        scale = Double(engine.snapshot.scale)
        clickThrough = engine.snapshot.click_through != 0
        autoWalk = engine.snapshot.auto_walk != 0
        gravity = engine.snapshot.gravity_enabled != 0
        alwaysOnTop = engine.snapshot.always_on_top != 0
        personaName = engine.text(PETSONA_TEXT_PERSONA_NAME)
        autostart = LaunchAgentService.isEnabled
    }

    private func importPet() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.folder, .zip]
        if panel.runModal() == .OK, let url = panel.url {
            engine.importPet(url)
        }
    }

    private func importCodexPet() {
        showingCodexPets = true
        engine.send(kind: PETSONA_COMMAND_SCAN_CODEX_PETS)
    }

    private func exportPet(_ id: String) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "\(id).zip"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        engine.send(kind: PETSONA_COMMAND_EXPORT_PET,
                    text: "\(id)\n\(url.path)")
    }

    private func deletePet(_ id: String) {
        let alert = NSAlert()
        alert.messageText = "删除宠物？"
        alert.informativeText = "将从 Petsona 本地库删除 \(id)，不会删除原始 Codex 文件。"
        alert.addButton(withTitle: "删除")
        alert.addButton(withTitle: "取消")
        if alert.runModal() == .alertFirstButtonReturn {
            engine.deletePet(id)
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
