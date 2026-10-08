import Darwin
import AppKit
import SwiftUI

@MainActor
final class SettingsWindow: NSWindow {
    private let navigationState: SettingsNavigationState
    private let settingsToolbarDelegate = SettingsToolbarDelegate()

    init(contentRect: NSRect, navigationState: SettingsNavigationState) {
        self.navigationState = navigationState
        super.init(contentRect: contentRect,
                   styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
                   backing: .buffered,
                   defer: false)

        title = navigationState.selection.windowTitle
        titleVisibility = .visible
        titlebarAppearsTransparent = true
        titlebarSeparatorStyle = .none
        toolbarStyle = .unified
        minSize = NSSize(width: SettingsLayout.windowMinWidth,
                         height: SettingsLayout.windowMinHeight)
        collectionBehavior = [.moveToActiveSpace]
        isReleasedWhenClosed = false

        let toolbar = NSToolbar(identifier: "com.petsona.settings")
        toolbar.delegate = settingsToolbarDelegate
        toolbar.allowsUserCustomization = false
        toolbar.autosavesConfiguration = false
        toolbar.displayMode = .iconOnly
        self.toolbar = toolbar

        navigationState.onSelectionChange = { [weak self] section in
            self?.title = section.windowTitle
        }
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    @objc func toggleSidebar(_ sender: Any?) {
        navigationState.toggleSidebar()
    }
}

@MainActor
private final class SettingsToolbarDelegate: NSObject, NSToolbarDelegate {
    private let itemIdentifiers: [NSToolbarItem.Identifier] = [
        .toggleSidebar,
        .sidebarTrackingSeparator,
    ]

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        itemIdentifiers
    }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        itemIdentifiers
    }
}

enum SettingsLaunchPolicy {
    static let acceptanceEnvironmentKey = "PETSONA_OPEN_SETTINGS_ON_LAUNCH"

    static func shouldPresentSettings(acceptanceLaunchRequested: Bool, hasPet: Bool) -> Bool {
        acceptanceLaunchRequested || !hasPet
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    #if DEBUG
    private(set) var testHostHome: URL?
    #endif
    let engine: EngineClient

    private var statusItem: NSStatusItem!
    private var petWindow: PetWindowController!
    private var settingsWindow: NSWindow?
    private var conversationHistoryWindow: NSWindow?
    private var openSettingsOnLaunch = false
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
    private let gazeStabilizer = GazeStabilizer()
    private var draggingPet = false
    private var lastDragRefreshTime = 0.0
    private var composerSide: OverlaySide?
    private var lastOverlaySide: OverlaySide?
    private var activeBubbleGeneration: Int64 = -1
    private var pausedBubbleGeneration: Int64 = -1

    override init() {
        #if DEBUG
        if let home = Self.makeIsolatedXCTestHome() {
            testHostHome = home
            engine = EngineClient(home: home)
        } else {
            engine = EngineClient()
        }
        #else
        engine = EngineClient()
        #endif
        super.init()
    }

    #if DEBUG
    private static var xctestHomeRoot: URL {
        let root = (0..<5).reduce(URL(fileURLWithPath: #filePath)) { root, _ in
            root.deletingLastPathComponent()
        }
        return root.appendingPathComponent(".scratch/macos-native-tests", isDirectory: true)
    }

    private static func makeIsolatedXCTestHome() -> URL? {
        let fileManager = FileManager.default
        let testRoot = xctestHomeRoot
        let activeMarker = testRoot.appendingPathComponent("host-active")
        let environment = ProcessInfo.processInfo.environment
        let requestedByEnvironment = environment["PETSONA_XCTEST_HOST"] == "1"
            || environment["XCTestConfigurationFilePath"] != nil
        guard requestedByEnvironment || fileManager.fileExists(atPath: activeMarker.path) else {
            return nil
        }

        let preparedHome = testRoot.appendingPathComponent("host-home", isDirectory: true)
        let home = fileManager.fileExists(atPath: activeMarker.path)
            ? preparedHome
            : fileManager.temporaryDirectory
                .appendingPathComponent("petsona-xctest-host-\(UUID().uuidString)", isDirectory: true)
        let keyEnvironment = "PETSONA_XCTEST_HOST_API_KEY"
        do {
            try fileManager.createDirectory(at: home, withIntermediateDirectories: true)
            let configPath = home.appendingPathComponent("config.json")
            if !fileManager.fileExists(atPath: configPath.path) {
                let config: [String: Any] = [
                    "stateServer": ["enabled": false],
                    "deepseek": ["apiKeyEnv": keyEnvironment],
                ]
                let data = try JSONSerialization.data(withJSONObject: config)
                try data.write(to: configPath, options: .atomic)
            }
            try fileManager.createDirectory(at: testRoot, withIntermediateDirectories: true)
            try Data(home.path.utf8).write(
                to: testRoot.appendingPathComponent("host-started"),
                options: .atomic
            )
        } catch {
            fatalError("Unable to prepare isolated XCTest home: \(error)")
        }
        for key in [keyEnvironment, "DEEPSEEK_API_KEY"] {
            guard setenv(key, "test-only-placeholder", 1) == 0 else {
                fatalError("Unable to isolate XCTest credential environment")
            }
        }
        return home
    }
    #endif

    func applicationDidFinishLaunching(_ notification: Notification) {
        #if DEBUG
        // The XCTest host is only a loader for the test bundle. It must never
        // create product windows, a status item, or a default-user runtime.
        if testHostHome != nil { return }
        #endif

        openSettingsOnLaunch = ProcessInfo.processInfo.environment[
            SettingsLaunchPolicy.acceptanceEnvironmentKey
        ] == "1"
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
        petWindow.onDragBegan = { [weak self] in
            guard let self else { return }
            draggingPet = true
            gazeStabilizer.reset()
            if globalGazeActive {
                engine.clearGaze()
                globalGazeActive = false
            }
        }
        petWindow.onDragMoved = { [weak self] in self?.refreshDuringDrag() }
        petWindow.onDragEnded = { [weak self] in
            self?.draggingPet = false
            self?.gazeStabilizer.reset()
        }
        bubblePanel = BubblePanel()
        bubblePanel.onReply = { [weak self] in self?.openConversationHistory() }
        bubblePanel.onHoverChanged = { [weak self] hovered in
            self?.setBubblePaused(hovered)
        }
        editPanel = EditButtonPanel()
        editPanel.onOpen = { [weak self] in self?.openComposer() }
        composerPanel = ComposerPanel()
        composerPanel.onSend = { [weak self] text in
            self?.conversationDraft = ""
            self?.engine.startConversation(text)
        }
        composerPanel.onCancel = { [weak self] in self?.engine.cancelConversation() }
        composerPanel.onClose = { [weak self] in
            self?.conversationDraft = self?.composerPanel.draft() ?? ""
            self?.composerSide = nil
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

    func applicationShouldHandleReopen(_ sender: NSApplication,
                                       hasVisibleWindows flag: Bool) -> Bool {
        openSettings()
        return true
    }

    func applicationWillTerminate(_ notification: Notification) {
        tickTimer?.invalidate()
        tickTimer = nil
        engine.shutdown()
        #if DEBUG
        if let testHostHome { try? FileManager.default.removeItem(at: testHostHome) }
        #endif
    }

    func windowWillClose(_ notification: Notification) {
        guard let window = notification.object as? NSWindow,
              window === settingsWindow || window === conversationHistoryWindow else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self,
                  self.settingsWindow?.isVisible != true,
                  self.conversationHistoryWindow?.isVisible != true else { return }
            self.setDockVisible(false)
        }
    }

    private func setDockVisible(_ visible: Bool) {
        let policy: NSApplication.ActivationPolicy = visible ? .regular : .accessory
        guard NSApp.activationPolicy() != policy else { return }
        if !NSApp.setActivationPolicy(policy) {
            engine.reportError(visible ? "无法在 Dock 显示设置窗口" : "无法恢复菜单栏模式")
        }
    }

    private func makeMenu() -> NSMenu {
        let menu = NSMenu()
        menu.addItem(withTitle: "设置…", action: #selector(openSettings), keyEquivalent: ",")
        menu.addItem(withTitle: "聊天记录", action: #selector(openConversationHistory), keyEquivalent: "")
        let petsItem = NSMenuItem(title: "选择宠物", action: nil, keyEquivalent: "")
        petsItem.submenu = NSMenu(title: "选择宠物")
        menu.addItem(petsItem)
        let scaleItem = NSMenuItem(title: "缩放", action: nil, keyEquivalent: "")
        scaleItem.submenu = makeScaleMenu()
        menu.addItem(scaleItem)
        menu.addItem(.separator())
        menu.addItem(withTitle: "显示 / 隐藏宠物", action: #selector(togglePet), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "退出 Petsona", action: #selector(quit), keyEquivalent: "q")
        for item in menu.items { item.target = self }
        return menu
    }

    private func scheduleTick() {
        tickTimer?.invalidate()
        let nextMilliseconds = engine.tick()
        if engine.snapshot.faulted != 0 {
            // A second instance or an unrecoverable worker fault must not
            // leave an inert menu-bar process behind.
            NSApp.terminate(nil)
            return
        }
        if engine.snapshot.ready != 0,
           SettingsLaunchPolicy.shouldPresentSettings(
               acceptanceLaunchRequested: openSettingsOnLaunch,
               hasPet: engine.snapshot.has_pet != 0
           ),
           !didPresentEmptyLibrary {
            didPresentEmptyLibrary = true
            openSettings()
        }
        let pointerPollMilliseconds: UInt32 = engine.snapshot.has_pet == 0
            ? 1_000
            : (globalGazeActive ? 16 : 33)
        let overlayPollMilliseconds: UInt32 = (bubblePanel?.needsRefresh == true || editPanel?.needsRefresh == true)
            ? 16
            : 1_000
        let next = max(0.016,
                      min(Double(max(min(nextMilliseconds,
                                          min(pointerPollMilliseconds, overlayPollMilliseconds)), 16)) / 1000.0,
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

    @objc func openSettings() {
        if settingsWindow == nil {
            let navigation = SettingsNavigationState()
            let root = SettingsView(engine: engine,
                                    navigation: navigation,
                                    onOpenConversationHistory: { [weak self] in
                                        self?.openConversationHistory()
                                    })
            let hosting = NSHostingView(rootView: root)
            let window = SettingsWindow(contentRect: NSRect(x: 0, y: 0, width: 960, height: 700),
                                        navigationState: navigation)
            window.delegate = self
            window.contentView = hosting
            window.center()
            settingsWindow = window
        }
        setDockVisible(true)
        NSApp.activate(ignoringOtherApps: true)
        settingsWindow?.makeKeyAndOrderFront(nil)
        settingsWindow?.orderFrontRegardless()
        settingsWindow?.makeKey()
        settingsWindow?.makeMain()
    }

    @objc func openConversationHistory() {
        if conversationHistoryWindow == nil {
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 560),
                                  styleMask: [.titled, .closable, .miniaturizable, .resizable],
                                  backing: .buffered,
                                  defer: false)
            window.title = "聊天记录"
            window.minSize = NSSize(width: 480, height: 360)
            window.collectionBehavior = [.moveToActiveSpace]
            window.isReleasedWhenClosed = false
            window.delegate = self
            window.contentView = NSHostingView(rootView: ConversationHistoryView(engine: engine))
            window.center()
            conversationHistoryWindow = window
        }
        setDockVisible(true)
        NSApp.activate(ignoringOtherApps: true)
        conversationHistoryWindow?.makeKeyAndOrderFront(nil)
    }

    @objc private func togglePet() {
        engine.send(kind: PETSONA_COMMAND_SET_VISIBILITY,
                    value: engine.snapshot.pet_visible == 0 ? 1 : 0)
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

        let history = NSMenuItem(title: "聊天记录",
                                 action: #selector(openConversationHistory),
                                 keyEquivalent: "")
        history.target = self
        menu.addItem(history)

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
        let current = Double(engine.snapshot.scale)
        for (value, title) in values {
            let item = NSMenuItem(title: title,
                                  action: #selector(setScaleFromMenu),
                                  keyEquivalent: "")
            item.target = self
            item.representedObject = value
            // Same steps as the settings slider: show which one is active.
            item.state = abs(current - value) < 0.01 ? .on : .off
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
        let petFrame = petWindow.screenFrame()
        let workFrame = workArea(for: petFrame)
        composerSide = MacOverlayLayout.chooseSide(pet: petFrame,
                                                   work: workFrame,
                                                   current: lastOverlaySide)
        lastOverlaySide = composerSide
        composerPanel.show(near: petFrame,
                           workFrame: workFrame,
                           side: composerSide ?? .bottom,
                           draft: conversationDraft)
    }

    private func updateOverlays() {
        let petFrame = petWindow.screenFrame()
        let workFrame = workArea(for: petFrame)
        composerPanel.setConversationState(engine.conversation.inFlight,
                                           error: engine.conversation.error)
        guard engine.snapshot.pet_visible != 0, engine.snapshot.has_pet != 0 else {
            bubblePanel.update(text: "", timing: nil, petFrame: petFrame, workFrame: workFrame)
            editPanel.hide()
            return
        }

        let bubbleText = engine.text(PETSONA_TEXT_BUBBLE)
        if bubbleText.isEmpty {
            if pausedBubbleGeneration != -1 {
                engine.send(kind: PETSONA_COMMAND_SET_BUBBLE_PAUSED, value: 0)
            }
            activeBubbleGeneration = -1
            pausedBubbleGeneration = -1
            bubblePanel.update(text: "", timing: nil, petFrame: petFrame, workFrame: workFrame)
        } else {
            let timing = BubbleTiming.parse(engine.text(PETSONA_TEXT_BUBBLE_TIMING))
            if let timing, timing.generation != activeBubbleGeneration {
                if pausedBubbleGeneration != -1 {
                    engine.send(kind: PETSONA_COMMAND_SET_BUBBLE_PAUSED, value: 0)
                    pausedBubbleGeneration = -1
                }
                activeBubbleGeneration = timing.generation
            }
            bubblePanel.update(text: bubbleText,
                               timing: timing,
                               petFrame: petFrame,
                               workFrame: workFrame)
            if bubblePanel.isHovered {
                setBubblePaused(true)
            }
        }

        if composerPanel.isVisible {
            composerSide = MacOverlayLayout.chooseSide(pet: petFrame,
                                                       work: workFrame,
                                                       current: composerSide ?? lastOverlaySide,
                                                       height: composerPanel.desiredHeight)
            lastOverlaySide = composerSide
            composerPanel.updatePosition(near: petFrame,
                                         workFrame: workFrame,
                                         side: composerSide ?? .bottom)
            editPanel.hide()
        } else {
            let side = MacOverlayLayout.chooseSide(pet: petFrame,
                                                   work: workFrame,
                                                   current: lastOverlaySide)
            lastOverlaySide = side
            editPanel.update(petFrame: petFrame,
                             workFrame: workFrame,
                             side: side,
                             now: CACurrentMediaTime())
        }
    }

    private func setBubblePaused(_ paused: Bool) {
        guard activeBubbleGeneration != -1 else { return }
        if paused {
            guard pausedBubbleGeneration != activeBubbleGeneration else { return }
            engine.send(kind: PETSONA_COMMAND_SET_BUBBLE_PAUSED, value: 1)
            pausedBubbleGeneration = activeBubbleGeneration
        } else if pausedBubbleGeneration != -1 {
            engine.send(kind: PETSONA_COMMAND_SET_BUBBLE_PAUSED, value: 0)
            pausedBubbleGeneration = -1
        }
    }

    private func workArea(for petFrame: NSRect) -> NSRect {
        let center = NSPoint(x: petFrame.midX, y: petFrame.midY)
        if let screen = NSScreen.screens.first(where: { $0.frame.contains(center) }) {
            return screen.visibleFrame
        }
        return NSScreen.main?.visibleFrame
            ?? NSScreen.screens.first?.visibleFrame
            ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
    }

    private func updateGlobalGaze() {
        guard !composerPanel.isVisible,
              !draggingPet,
              engine.snapshot.pet_visible != 0,
              engine.snapshot.has_pet != 0 else {
            if globalGazeActive {
                engine.clearGaze()
                globalGazeActive = false
            }
            gazeStabilizer.reset()
            return
        }
        let frame = petWindow.screenFrame()
        let cursor = NSEvent.mouseLocation
        if let target = PetGazeGeometry.target(cursor: cursor,
                                              petFrame: frame,
                                              active: globalGazeActive) {
            let direction = gazeStabilizer.update(dx: target.x, dy: target.y)
            let vector = GazeStabilizer.unitVector(direction: direction)
            // Re-send the held direction every poll so a cross-row transition
            // can finish even when the cursor stops moving.
            engine.setGazeTarget(dx: vector.x, dy: vector.y)
            globalGazeActive = true
            return
        }
        if globalGazeActive {
            engine.clearGaze()
            globalGazeActive = false
        }
        gazeStabilizer.reset()
    }

    private func refreshDuringDrag() {
        let now = CACurrentMediaTime()
        guard now - lastDragRefreshTime >= 0.008 else { return }
        lastDragRefreshTime = now
        _ = engine.tick()
        petWindow.update()
        updateOverlays()
    }
}
