import AppIntents
import AranetBarShared
import Foundation
import SQLite3
#if canImport(WidgetKit)
import WidgetKit
#endif

@available(macOS 14.0, *)
struct AranetSensorEntity: AppEntity, IndexedEntity {
    static var typeDisplayRepresentation = TypeDisplayRepresentation(name: "Aranet4 Sensor")
    static var defaultQuery = AranetSensorQuery()

    var id: String
    var displayName: String
    var co2Ppm: Int?

    var displayRepresentation: DisplayRepresentation {
        if let co2 = co2Ppm {
            return DisplayRepresentation(title: "\(displayName)", subtitle: "\(co2) ppm CO₂")
        }
        return DisplayRepresentation(title: "\(displayName)")
    }
}

@available(macOS 14.0, *)
struct AranetSensorQuery: EntityQuery {
    func entities(for identifiers: [AranetSensorEntity.ID]) async throws -> [AranetSensorEntity] {
        try await suggestedEntities().filter { identifiers.contains($0.id) }
    }

    func suggestedEntities() async throws -> [AranetSensorEntity] {
        let state = GoldenGateState.load()
        var entities: [AranetSensorEntity] = []
        if !state.pinnedSensor.isEmpty {
            entities.append(
                AranetSensorEntity(
                    id: state.pinnedSensor,
                    displayName: state.pinnedNickname.isEmpty ? state.pinnedSensor : state.pinnedNickname,
                    co2Ppm: state.latestCo2
                )
            )
        }
        if let extra = loadSensorsFromSharedDb(state: state) {
            for row in extra where !entities.contains(where: { $0.id == row.id }) {
                entities.append(row)
            }
        }
        return entities
    }

    private func loadSensorsFromSharedDb(state: GoldenGateState) -> [AranetSensorEntity]? {
        let path = state.sharedDbPath.isEmpty ? state.dbPath : state.sharedDbPath
        guard !path.isEmpty, FileManager.default.fileExists(atPath: path) else { return nil }
        var db: OpaquePointer?
        guard sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK, let db else { return nil }
        defer { sqlite3_close(db) }
        let sql = """
        SELECT r.sensor, COALESCE(s.room, r.sensor), r.co2
        FROM readings r
        LEFT JOIN sensors s ON s.sensor = r.sensor
        WHERE r.time = (SELECT MAX(time) FROM readings r2 WHERE r2.sensor = r.sensor)
        ORDER BY r.time DESC LIMIT 8
        """
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK, let stmt else { return nil }
        defer { sqlite3_finalize(stmt) }
        var out: [AranetSensorEntity] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            let id = String(cString: sqlite3_column_text(stmt, 0))
            let name = String(cString: sqlite3_column_text(stmt, 1))
            let co2 = sqlite3_column_type(stmt, 2) != SQLITE_NULL ? Int(sqlite3_column_int(stmt, 2)) : nil
            out.append(AranetSensorEntity(id: id, displayName: name, co2Ppm: co2))
        }
        return out
    }
}

@available(macOS 14.0, *)
struct QueryCo2Intent: AppIntent {
    static var title: LocalizedStringResource = "What's the CO₂ level?"
    static var description = IntentDescription("Reads the latest CO₂ from your pinned Aranet4 sensor.")

    @Parameter(title: "Sensor")
    var sensor: AranetSensorEntity?

    static var parameterSummary: some ParameterSummary {
        Summary("CO₂ in \(\.$sensor)")
    }

    func perform() async throws -> some IntentResult & ReturnsValue<String> {
        let state = GoldenGateState.load()
        let entity = sensor ?? (try await AranetSensorQuery().suggestedEntities().first)
        guard let entity else {
            return .result(value: "No Aranet4 sensors are available yet.")
        }
        if let ppm = entity.co2Ppm {
            return .result(value: "\(entity.displayName) is at \(ppm) ppm CO₂.")
        }
        if let ppm = state.latestCo2 {
            return .result(value: "\(state.pinnedNickname.isEmpty ? state.pinnedSensor : state.pinnedNickname) is at \(ppm) ppm CO₂.")
        }
        return .result(value: "No recent CO₂ reading for \(entity.displayName).")
    }
}

@available(macOS 14.0, *)
struct AranetBarFocusFilter: SetFocusFilterIntent {
    static var title: LocalizedStringResource = "AranetBar Alert Profile"

    @Parameter(title: "Pinned room only during Focus")
    var pinnedOnly: Bool

    func perform() async throws -> some IntentResult {
        var state = GoldenGateState.load()
        state.focusAlertsMode = pinnedOnly ? .pinnedOnly : .allSensors
        state.save()
        return .result()
    }
}

@available(macOS 14.0, *)
struct AranetBarShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: QueryCo2Intent(),
            phrases: [
                "What's the CO₂ in \(.applicationName)",
                "CO₂ level in \(.applicationName)",
            ],
            shortTitle: "CO₂ reading",
            systemImageName: "aqi.medium"
        )
    }
}

@_cdecl("AranetBarRefreshAppIntents")
public func AranetBarRefreshAppIntents() {
    if #available(macOS 14.0, *) {
        Task {
            let entities = (try? await AranetSensorQuery().suggestedEntities()) ?? []
            for entity in entities {
                let donation = AranetSensorEntity(id: entity.id, displayName: entity.displayName, co2Ppm: entity.co2Ppm)
                try? await donation.donate()
            }
        }
    }
}

import SQLite3
