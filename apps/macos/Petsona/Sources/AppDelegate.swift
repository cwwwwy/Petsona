import AppKit
import SwiftUI

@MainActor
private final class SettingsWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let engine = EngineClient()

    private var statusItem: NSStatusItem!
    private var petWindow: PetWindowController!
    private var settingsWindow: NSWindow?
    private var tickTimer: Timer?
    private var didPresentEmptyLibrary = false
    private var bubblePanel: BubblePanel!
    private var editPanel: EditButtonPanel!
    private var composerPanel: ComposerPanel!
    private var conversationDraft = ""
    private var lastPetsMenuJSON = ""
    private var lastSelectedPetID = ""
    private var lastScale = -1.0
    private var globalGazeActive = false
    private var lastGlobalCursor: NSPoint?

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.title = "🐾"
        statusItem.menu = makeMenu()

        petWindow = PetWindowController(engine: engine)
        petWindow.onDoubleClick = { [weak self] in
            self?.engine.send(kind: PETSONA_COMMAND_SET_STATE, text: "jumping")
        }
        petWindow.onRightClick = { [weak self] event, view in
            self?.showPetContextMenu(with: event, in: view)
        }
        bubblePanel = BubblePanel()
        bubblePanel.onReply = { [weak self] in self?.openComposer() }
        editPanel = EditButtonPanel()
        editPanel.onOpen = { [weak self] in self?.openComposer() }
        composerPanel = ComposerPanel()
        composerPanel.onSend = { [weak self] text in
            self?.conversationDraft = ""
            self?.engine.send(kind: PETSONA_COMMAND_SEND_CONVERSATION, text: text)
        }
        composerPanel.onClose = { [weak self] in
            self?.conversationDraft = self?.composerPanel.draft() ?? ""
            self?.engine.clearGaze()
        }
        composerPanel.onCaret = { [weak self] point in
            guard let self else { return }
            let petFrame = self.petWindow.screenFrame()
            self.engine.setGazeTarget(dx: point.x - petFrame.midX,
                                      dy: petFrame.midY - point.y)
        }
        scheduleTick()
    }

    func applicationWillTerminate(_ notification: Notification) {
        tickTimer?.invalidate()
        tickTimer = nil
    }

    private func makeMenu() -> NSMenu {
        let menu = NSMenu()
        menu.addItem(withTitle: "设置…", action: #selector(openSettings), keyEquivalent: ",")
        let petsItem = NSMenuItem(title: "选择宠物", action: nil, keyEquivalent: "")
        petsItem.submenu = NSMenu(title: "选择宠物")
        menu.addItem(petsItem)
        let scaleItem = NSMenuItem(title: "缩放", action: nil, keyEquivalent: "")
        scaleItem.submenu = makeScaleMenu()
        menu.addItem(scaleItem)
        menu.addItem(.separator())
        menu.addItem(withTitle: "显示 / 隐藏宠物", action: #selector(togglePet), keyEquivalent: "")
        menu.addItem(withTitle: "立即活动", action: #selector(triggerActivity), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "退出 Petsona", action: #selector(quit), keyEquivalent: "q")
        for item in menu.items { item.target = self }
        return menu
    }

    private func scheduleTick() {
        tickTimer?.invalidate()
        let nextMilliseconds = engine.tick()
        if engine.snapshot.ready != 0,
           engine.snapshot.has_pet == 0,
           !didPresentEmptyLibrary {
            didPresentEmptyLibrary = true
            openSettings()
        }
        let pointerPollMilliseconds: UInt32 = engine.snapshot.has_pet == 0
            ? 1_000
            : (globalGazeActive ? 16 : 33)
        let next = max(0.016,
                      min(Double(max(min(nextMilliseconds, pointerPollMilliseconds), 16)) / 1000.0,
                          60.0))
        tickTimer = Timer.scheduledTimer(withTimeInterval: next,
                                         repeats: false) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.petWindow.update()
                self.updateOverlays()
                self.updateGlobalGaze()
                self.refreshPetMenu()
                self.refreshScaleMenu()
                self.scheduleTick()
            }
        }
    }

    @objc private func openSettings() {
        if settingsWindow == nil {
            let root = SettingsView(engine: engine)
            let hosting = NSHostingView(rootView: root)
            let window = SettingsWindow(contentRect: NSRect(x: 0, y: 0, width: 960, height: 700),
                                        styleMask: [.titled, .closable, .resizable],
                                        backing: .buffered,
                                        defer: false)
            window.title = "Petsona 设置"
            window.contentView = hosting
            window.minSize = NSSize(width: 820, height: 600)
            window.collectionBehavior = [.moveToActiveSpace]
            window.center()
            window.isReleasedWhenClosed = false
            settingsWindow = window
        }
        NSApp.activate(ignoringOtherApps: true)
        settingsWindow?.makeKeyAndOrderFront(nil)
        settingsWindow?.orderFrontRegardless()
        settingsWindow?.makeKey()
        settingsWindow?.makeMain()
    }

    @objc private func togglePet() {
        engine.send(kind: PETSONA_COMMAND_SET_VISIBILITY,
                    value: engine.snapshot.pet_visible == 0 ? 1 : 0)
        petWindow.update()
    }

    @objc private func triggerActivity() {
        engine.send(kind: PETSONA_COMMAND_SET_STATE,
                    ttlMilliseconds: 8_000,
                    text: "running")
        engine.send(kind: PETSONA_COMMAND_SHOW_BUBBLE,
                    ttlMilliseconds: 5_000,
                    text: "现在活动一下吧")
        petWindow.update()
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }

    @objc private func selectPetFromMenu(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        engine.selectPet(id)
    }

    private func showPetContextMenu(with event: NSEvent, in view: NSView) {
        let menu = NSMenu(title: "Petsona")
        let settings = NSMenuItem(title: "设置…", action: #selector(openSettings), keyEquivalent: ",")
        settings.target = self
        menu.addItem(settings)

        let pets = NSMenuItem(title: "选择宠物", action: nil, keyEquivalent: "")
        pets.submenu = makePetSelectionMenu()
        menu.addItem(pets)

        let scale = NSMenuItem(title: "缩放", action: nil, keyEquivalent: "")
        scale.submenu = makeScaleMenu()
        menu.addItem(scale)
        menu.addItem(.separator())

        let visibility = NSMenuItem(title: "显示 / 隐藏宠物",
                                    action: #selector(togglePet),
                                    keyEquivalent: "")
        visibility.target = self
        menu.addItem(visibility)

        let activity = NSMenuItem(title: "立即活动",
                                  action: #selector(triggerActivity),
                                  keyEquivalent: "")
        activity.target = self
        menu.addItem(activity)
        menu.addItem(.separator())

        let quit = NSMenuItem(title: "退出 Petsona", action: #selector(quit), keyEquivalent: "q")
        quit.target = self
        menu.addItem(quit)

        NSMenu.popUpContextMenu(menu, with: event, for: view)
    }

    private func makePetSelectionMenu() -> NSMenu {
        let submenu = NSMenu(title: "选择宠物")
        let selectedID = engine.text(PETSONA_TEXT_PET_ID)
        guard let data = engine.text(PETSONA_TEXT_PETS).data(using: .utf8),
              let values = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else {
            return submenu
        }
        for value in values {
            guard let id = value["id"] as? String,
                  let name = value["name"] as? String else { continue }
            let item = NSMenuItem(title: name,
                                  action: #selector(selectPetFromMenu),
                                  keyEquivalent: "")
            item.target = self
            item.representedObject = id
            item.state = id == selectedID ? .on : .off
            submenu.addItem(item)
        }
        if submenu.items.isEmpty {
            let empty = NSMenuItem(title: "本地库为空", action: nil, keyEquivalent: "")
            empty.isEnabled = false
            submenu.addItem(empty)
        }
        return submenu
    }

    private func makeScaleMenu() -> NSMenu {
        let menu = NSMenu(title: "缩放")
        let values: [(Double, String)] = [
            (0.5, "小（50%）"),
            (0.75, "较小（75%）"),
            (1.0, "中（100%）"),
            (1.25, "较大（125%）"),
            (1.5, "大（150%）"),
            (1.75, "特大（175%）"),
            (2.0, "超大（200%）"),
        ]
        for (value, title) in values {
            let item = NSMenuItem(title: title,
                                  action: #selector(setScaleFromMenu),
                                  keyEquivalent: "")
            item.target = self
            item.representedObject = value
            menu.addItem(item)
        }
        return menu
    }

    @objc private func setScaleFromMenu(_ sender: NSMenuItem) {
        guard let value = sender.representedObject as? Double else { return }
        engine.send(kind: PETSONA_COMMAND_SET_SCALE, value: value)
        refreshScaleMenu()
    }

    private func refreshScaleMenu() {
        let value = Double(engine.snapshot.scale)
        guard abs(value - lastScale) > 0.001,
              let menu = statusItem.menu,
              let item = menu.items.first(where: { $0.title == "缩放" }),
              let submenu = item.submenu else { return }
        lastScale = value
        for menuItem in submenu.items {
            guard let represented = menuItem.representedObject as? Double else { continue }
            menuItem.state = abs(represented - value) < 0.001 ? .on : .off
        }
    }

    private func refreshPetMenu() {
        let json = engine.text(PETSONA_TEXT_PETS)
        let selectedID = engine.text(PETSONA_TEXT_PET_ID)
        guard json != lastPetsMenuJSON || selectedID != lastSelectedPetID,
              let menu = statusItem.menu,
              let item = menu.items.first(where: { $0.title == "选择宠物" }),
              let submenu = item.submenu else { return }
        lastPetsMenuJSON = json
        lastSelectedPetID = selectedID
        submenu.removeAllItems()
        guard let data = json.data(using: .utf8),
              let values = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else {
            return
        }
        for value in values {
            guard let id = value["id"] as? String,
                  let name = value["name"] as? String else { continue }
            let menuItem = NSMenuItem(title: name, action: #selector(selectPetFromMenu), keyEquivalent: "")
            menuItem.target = self
            menuItem.representedObject = id
            menuItem.state = engine.text(PETSONA_TEXT_PET_ID) == id ? .on : .off
            submenu.addItem(menuItem)
        }
        if submenu.items.isEmpty {
            let empty = NSMenuItem(title: "本地库为空", action: nil, keyEquivalent: "")
            empty.isEnabled = false
            submenu.addItem(empty)
        }
    }

    private func openComposer() {
        guard engine.snapshot.has_pet != 0 else {
            openSettings()
            return
        }
        composerPanel.show(near: petWindow.screenFrame(), draft: conversationDraft)
    }

    private func updateOverlays() {
        guard engine.snapshot.pet_visible != 0, engine.snapshot.has_pet != 0 else {
            bubblePanel.update(text: "", petFrame: petWindow.screenFrame())
            editPanel.update(petFrame: petWindow.screenFrame(), visible: false)
            return
        }
        bubblePanel.update(text: engine.text(PETSONA_TEXT_BUBBLE),
                           petFrame: petWindow.screenFrame())
        editPanel.update(petFrame: petWindow.screenFrame(), visible: !composerPanel.isVisible)
    }

    private func updateGlobalGaze() {
        guard !composerPanel.isVisible,
              engine.snapshot.pet_visible != 0,
              engine.snapshot.has_pet != 0 else {
            if globalGazeActive {
                engine.clearGaze()
                globalGazeActive = false
            }
            return
        }
        let frame = petWindow.screenFrame()
        let cursor = NSEvent.mouseLocation
        let moved = lastGlobalCursor.map { hypot(cursor.x - $0.x, cursor.y - $0.y) > 1 } ?? false
        lastGlobalCursor = cursor
        let dx = cursor.x - frame.midX
        // Core gaze coordinates use screen Y (positive below); AppKit's
        // global coordinate system grows upwards.
        let dy = frame.midY - cursor.y
        let width = CGFloat(max(engine.snapshot.cell_width, 1)) * CGFloat(max(engine.snapshot.scale, 0.1))
        let height = CGFloat(max(engine.snapshot.cell_height, 1)) * CGFloat(max(engine.snapshot.scale, 0.1))
        // Keep gaze local, but make the native trigger forgiving enough for
        // normal mouse motion. Once active, use a larger release radius so a
        // one-pixel edge fluctuation does not cancel the gaze.
        let margin = min(width, height) * (globalGazeActive ? 0.50 : 0.40)
        let radiusX = width * 0.5 + margin
        let radiusY = height * 0.5 + margin
        let near = (dx * dx) / (radiusX * radiusX) + (dy * dy) / (radiusY * radiusY) <= 1
        let deadZone = min(width, height) * 0.22
        if near && hypot(dx, dy) > deadZone {
            if moved {
                engine.setGazeTarget(dx: dx, dy: dy)
                globalGazeActive = true
            }
            // A stationary cursor inside the active range keeps the current
            // pose. Do not clear it merely because this timer tick had no
            // pointer movement.
            return
        }
        if globalGazeActive {
            engine.clearGaze()
            globalGazeActive = false
        }
    }
}
