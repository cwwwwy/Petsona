import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct PersonaSourceView: View {
    @ObservedObject var engine: EngineClient
    @Environment(\.dismiss) private var dismiss
    let initialMode: PersonaSourceMode

    @State private var mode: PersonaSourceMode
    @State private var inputFormat = "txt"
    @State private var pastedSource = ""
    @State private var selectedSpeaker = ""
    @State private var targetSpeakerLabel = ""
    @State private var sampleStart = 0
    @State private var sampleEnd = 3
    @State private var waitingForApply = false
    @State private var referenceName = ""
    @State private var referenceDescription = ""
    @State private var draftID = ""
    @State private var draftName = ""
    @State private var personality = ""
    @State private var expressionStyle = ""
    @State private var responseHabits = ""
    @State private var relationship = ""
    @State private var examples = ""
    @State private var previewPrompt = ""

    init(engine: EngineClient, initialMode: PersonaSourceMode) {
        self.engine = engine
        self.initialMode = initialMode
        _mode = State(initialValue: initialMode)
    }

    private var source: PersonaSourceProjection? { engine.personaSource }
    private var draft: PersonaDraftProjection? { engine.personaDraft.draft }
    private var petID: String { engine.conversation.petId.isEmpty ? engine.text(PETSONA_TEXT_PET_ID) : engine.conversation.petId }
    private var generateButtonTitle: String {
        engine.personaDraft.generating ? "正在生成…" : "生成可编辑草稿"
    }
    private var canGenerate: Bool {
        guard !engine.personaDraft.generating else { return false }
        switch mode {
        case .chatImport:
            return source?.id != nil
        case .publicFigure:
            return !referenceName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("人格来源") {
                    Picker("来源", selection: $mode) {
                        ForEach(PersonaSourceMode.allCases) { option in
                            Text(option.label).tag(option)
                        }
                    }
                    if mode == .chatImport {
                        Picker("文本格式", selection: $inputFormat) {
                            Text("TXT").tag("txt")
                            Text("JSON").tag("json")
                        }
                        .pickerStyle(.segmented)
                        TextEditor(text: $pastedSource)
                            .frame(minHeight: 100)
                            .accessibilityLabel("粘贴聊天记录")
                        HStack {
                            Button("选择 TXT/JSON 文件…", action: chooseSourceFile)
                            Button("解析粘贴内容") {
                                engine.parsePersonaSourceText(pastedSource, format: inputFormat)
                            }
                            .disabled(pastedSource.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                        if source?.parsing == true {
                            ProgressView("正在读取并解析资料…")
                                .controlSize(.small)
                        }
                        if let source, source.messageCount > 0 {
                            let startRange: ClosedRange<Int> = 0...max(source.messageCount - 1, 0)
                            let endRange: ClosedRange<Int> = 1...max(source.messageCount, 1)
                            Picker("目标说话人", selection: $selectedSpeaker) {
                                ForEach(source.speakers) { speaker in
                                    Text("\(speaker.name)（\(speaker.count) 条）").tag(speaker.name)
                                }
                            }
                            TextField("说话人校正名称", text: $targetSpeakerLabel,
                                      prompt: Text(selectedSpeaker))
                            Text("可将文件中的昵称改成你熟悉的称呼；只影响生成资料，不改原始聊天记录。")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Stepper(value: $sampleStart, in: startRange) {
                                Text("采样起始消息：\(sampleStart + 1)")
                            }
                            Stepper(value: $sampleEnd, in: endRange) {
                                Text("采样结束位置：\(sampleEnd)")
                            }
                                .onChange(of: sampleStart) { _, start in
                                    if sampleEnd <= start {
                                        sampleEnd = min(start + 1, max(source.messageCount, 1))
                                    }
                                }
                            Text("预览（资料不写入人格，只在点击生成时发送选中范围）")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            ForEach(source.preview) { message in
                                Text("\(message.speaker)：\(message.text)")
                                    .font(.caption)
                                    .textSelection(.enabled)
                            }
                        }
                    } else {
                        TextField("人物姓名", text: $referenceName)
                        TextEditor(text: $referenceDescription)
                            .frame(minHeight: 130)
                            .accessibilityLabel("人物介绍和参考文字")
                        Text("使用已配置模型的知识和你提供的文字生成风格参考；本机不会联网搜索资料。")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                    if !engine.personaSourceError.isEmpty {
                        Label(engine.personaSourceError,
                              systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(.red)
                            .textSelection(.enabled)
                    }
                }

                generationSection

                if let draft {
                    Section("编辑人格草稿") {
                        TextField("人格名称", text: $draftName)
                        TextField("性格", text: $personality)
                        TextField("表达风格", text: $expressionStyle)
                        TextField("回应习惯", text: $responseHabits)
                        TextField("与用户的关系", text: $relationship)
                        TextEditor(text: $examples)
                            .frame(minHeight: 90)
                            .accessibilityLabel("示例回应，每行一个")
                        Text("灵感来源：\(draft.source.label)；选择的目标消息 \(draft.source.sampleCount) 条。")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }

                    Section("试聊（不会写入正式聊天或记忆）") {
                        TextField("试聊问题", text: $previewPrompt)
                        Button(engine.personaPreview.inFlight ? "正在试聊…" : "试聊这份人格") {
                            engine.previewPersonaDraft(draftID: draft.id,
                                                       petID: petID,
                                                       prompt: previewPrompt)
                        }
                        .disabled(engine.personaPreview.inFlight
                                  || previewPrompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        if !engine.personaPreview.text.isEmpty {
                            Text(engine.personaPreview.text)
                                .textSelection(.enabled)
                        }
                        if !engine.personaPreview.error.isEmpty {
                            Text(engine.personaPreview.error)
                                .foregroundStyle(.red)
                                .textSelection(.enabled)
                        }
                    }

                    Section {
                        Button("应用到这只宠物") {
                            let style = PersonaStyleProfileDraft(
                                personality: personality,
                                expressionStyle: expressionStyle,
                                responseHabits: responseHabits,
                                relationship: relationship,
                                examples: examples
                                    .split(whereSeparator: \.isNewline)
                                    .map(String.init)
                                    .filter { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
                            )
                            waitingForApply = true
                            engine.applyPersonaDraft(draftID: draft.id,
                                                     petID: petID,
                                                     name: draftName,
                                                     style: style)
                        }
                        .disabled(draftID != draft.id || petID.isEmpty)
                    }
                }
            }
            .formStyle(.grouped)
            .navigationTitle("塑造人格")
            .frame(minWidth: 540, minHeight: 600)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("关闭") { dismiss() }
                }
            }
        }
        .onAppear { loadDraftIfPresent() }
        .onChange(of: source?.id) { _, _ in
            selectedSpeaker = source?.speakers.first?.name ?? ""
            targetSpeakerLabel = source?.speakers.first?.name ?? ""
            sampleStart = 0
            sampleEnd = min(max(source?.messageCount ?? 3, 3), 100)
        }
        .onChange(of: selectedSpeaker) { _, speaker in
            targetSpeakerLabel = speaker
        }
        .onChange(of: draft?.id) { _, _ in loadDraftIfPresent() }
        .onChange(of: engine.personaDraft.draft?.id) { oldID, newID in
            if waitingForApply, oldID != nil, newID == nil { dismiss() }
        }
        .onDisappear { engine.clearPersonaDraft() }
    }

    private var generationSection: some View {
        Section {
            Button(generateButtonTitle, action: generateDraft)
                .disabled(!canGenerate)
            Text("只发送选中的聊天范围或你填写的人物介绍。原始资料不会保存在人格档案中。")
                .font(.footnote)
                .foregroundStyle(.secondary)
            if engine.personaDraft.generating {
                ProgressView().controlSize(.small)
            }
            if !engine.personaDraftError.isEmpty {
                Label(engine.personaDraftError,
                      systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            }
        } header: {
            Text("生成")
        }
    }

    private func chooseSourceFile() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.plainText, .json]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        inputFormat = url.pathExtension.lowercased() == "json" ? "json" : "txt"
        engine.parsePersonaSourceFile(url)
    }

    private func generateDraft() {
        if mode == .chatImport, let source {
            engine.generatePersonaProfile(kind: "chat_import",
                                          sourceID: source.id ?? "",
                                          label: source.label ?? "聊天记录",
                                          description: "",
                                          targetSpeaker: selectedSpeaker,
                                          targetSpeakerLabel: targetSpeakerLabel,
                                          startIndex: sampleStart,
                                          endIndex: sampleEnd)
        } else {
            engine.generatePersonaProfile(kind: "public_figure",
                                          sourceID: "",
                                          label: referenceName,
                                          description: referenceDescription,
                                          targetSpeaker: "",
                                          targetSpeakerLabel: "",
                                          startIndex: 0,
                                          endIndex: 0)
        }
    }

    private func loadDraftIfPresent() {
        guard let draft else { return }
        guard draftID != draft.id else { return }
        draftID = draft.id
        draftName = draft.name
        personality = draft.style.personality
        expressionStyle = draft.style.expressionStyle
        responseHabits = draft.style.responseHabits
        relationship = draft.style.relationship
        examples = draft.style.examples.joined(separator: "\n")
    }
}
