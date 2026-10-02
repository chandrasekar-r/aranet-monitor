import Foundation

public enum FocusAlertsMode: String, Codable, Sendable {
    case allSensors = "all_sensors"
    case pinnedOnly = "pinned_only"
}

public struct GoldenGateState: Codable, Sendable {
    public var co2SnoozeUntil: Date?
    public var focusAlertsMode: FocusAlertsMode
    public var pinnedSensor: String
    public var pinnedNickname: String
    public var latestCo2: Int?
    public var tone: String
    public var measurementIntervalSecs: Int
    public var dbPath: String
    public var sharedDbPath: String
    public var updatedAt: Date?

    enum CodingKeys: String, CodingKey {
        case co2SnoozeUntil = "co2_snooze_until"
        case focusAlertsMode = "focus_alerts_mode"
        case pinnedSensor = "pinned_sensor"
        case pinnedNickname = "pinned_nickname"
        case latestCo2 = "latest_co2"
        case tone
        case measurementIntervalSecs = "measurement_interval_secs"
        case dbPath = "db_path"
        case sharedDbPath = "shared_db_path"
        case updatedAt = "updated_at"
    }

    public init() {
        co2SnoozeUntil = nil
        focusAlertsMode = .allSensors
        pinnedSensor = ""
        pinnedNickname = ""
        latestCo2 = nil
        tone = "unknown"
        measurementIntervalSecs = 300
        dbPath = ""
        sharedDbPath = ""
        updatedAt = nil
    }

    public static func load() -> GoldenGateState {
        let url = AranetBarAppGroup.stateURL
        guard let data = try? Data(contentsOf: url),
              let decoded = try? JSONDecoder().decode(GoldenGateState.self, from: data)
        else { return GoldenGateState() }
        return decoded
    }

    public func save() {
        var copy = self
        copy.updatedAt = Date()
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        enc.dateEncodingStrategy = .iso8601
        guard let data = try? enc.encode(copy) else { return }
        try? data.write(to: AranetBarAppGroup.stateURL, options: .atomic)
    }

    public var isSnoozed: Bool {
        guard let until = co2SnoozeUntil else { return false }
        return until > Date()
    }
}
