import SwiftUI

struct ConversationHistoryView: View {
    @ObservedObject var engine: EngineClient
    @State private var confirmClear = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Toggle("保存聊天记录",
                       isOn: Binding(
                        get: { engine.conversation.saveHistory },
                        set: { engine.updateConversationConfig(saveHistory: $0) }
                       ))
                Text("\(engine.conversation.totalCount) 条")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Button("清除记录") { confirmClear = true }
                    .disabled(engine.conversation.turns.isEmpty)
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
            Divider()
            if engine.conversation.hasEarlier {
                Button("加载更早记录") { engine.loadEarlierConversationHistory() }
                    .buttonStyle(.plain)
                    .padding(.horizontal, 16)
            }
            if !engine.conversation.error.isEmpty {
                Label(engine.conversation.error, systemImage: "exclamationmark.triangle")
                    .font(.callout)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            }
            if engine.conversation.turns.isEmpty {
                ContentUnavailableView(
                    "还没有聊天记录",
                    systemImage: "bubble.left.and.bubble.right",
                    description: Text("和宠物聊过的内容会显示在这里。")
                )
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 14) {
                            ForEach(Array(engine.conversation.turns.enumerated()), id: \.element.id) { item in
                                turnRow(index: item.offset, turn: item.element)
                                    .id(item.element.id)
                            }
                        }
                        .padding(16)
                    }
                    .onChange(of: engine.conversation.turns.last?.id) { _, id in
                        guard let id else { return }
                        withAnimation(.easeOut(duration: 0.15)) {
                            proxy.scrollTo(id, anchor: .bottom)
                        }
                    }
                }
            }
        }
        .frame(minWidth: 520, minHeight: 420)
        .confirmationDialog("清除这只宠物的聊天记录？",
                            isPresented: $confirmClear,
                            titleVisibility: .visible) {
            Button("清除聊天记录", role: .destructive) {
                engine.clearConversationHistory()
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("此操作不会清除宠物已保存的偏好和习惯。")
        }
    }

    @ViewBuilder
    private func turnRow(index: Int, turn: ConversationTurnProjection) -> some View {
        HStack {
            if turn.user { Spacer(minLength: 60) }
            VStack(alignment: turn.user ? .trailing : .leading, spacing: 5) {
                Text(turn.user ? "你" : "宠物")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(turn.text)
                    .textSelection(.enabled)
                    .frame(maxWidth: 420, alignment: turn.user ? .trailing : .leading)
                    .padding(10)
                    .background(turn.user ? Color.accentColor.opacity(0.14) : Color.secondary.opacity(0.12))
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                if turn.status == "failed" || turn.status == "cancelled" || turn.status == "interrupted" {
                    HStack(spacing: 8) {
                        Text(turn.status == "failed" ? "未完成" : "已停止")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        if canRetry(index: index, status: turn.status),
                           let previous = engine.conversation.turns[..<index].last(where: { $0.user }) {
                            Button("重试") {
                                engine.startConversation(previous.text, retryTurnID: previous.id)
                            }
                            .font(.caption)
                        }
                    }
                }
            }
            if !turn.user { Spacer(minLength: 60) }
        }
    }

    private func canRetry(index: Int, status: String) -> Bool {
        guard status == "failed" || status == "cancelled" || status == "interrupted" else {
            return false
        }
        let turns = engine.conversation.turns
        return index + 1 >= turns.count || turns[index + 1].user
    }
}
