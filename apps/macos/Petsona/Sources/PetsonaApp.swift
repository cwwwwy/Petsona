import SwiftUI

@main
struct PetsonaApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    var body: some Scene {
        // The menu-bar app owns its settings window from AppDelegate. Keeping
        // the SwiftUI scene empty avoids creating a second hidden window.
        Settings {
            EmptyView()
        }
    }
}
