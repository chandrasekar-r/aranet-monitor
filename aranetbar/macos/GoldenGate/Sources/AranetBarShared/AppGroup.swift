import Foundation

public enum AranetBarAppGroup {
    public static let identifier = "group.com.20deg.aranetbar"
    public static let stateFileName = "golden_gate_state.json"
    public static let muteControlKind = "com.20deg.aranetbar.mute-co2"

    public static var containerURL: URL {
        if let url = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: identifier) {
            return url
        }
        let fallback = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/AranetBar", isDirectory: true)
        try? FileManager.default.createDirectory(at: fallback, withIntermediateDirectories: true)
        return fallback
    }

    public static var stateURL: URL {
        containerURL.appendingPathComponent(stateFileName)
    }
}
