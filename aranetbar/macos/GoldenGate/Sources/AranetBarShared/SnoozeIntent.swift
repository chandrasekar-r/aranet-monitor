import AppIntents
import Foundation
#if canImport(WidgetKit)
import WidgetKit
#endif

@available(macOS 14.0, *)
public struct ToggleCo2SnoozeIntent: AppIntent {
    public static var title: LocalizedStringResource = "Mute CO₂ Alerts"
    public static var description = IntentDescription("Snoozes or clears CO₂ notifications for one hour.")

    @Parameter(title: "Muted", default: false)
    public var muted: Bool

    public init() {}
    public init(muted: Bool) {
        self.muted = muted
    }

    public func perform() async throws -> some IntentResult {
        var state = GoldenGateState.load()
        if muted {
            state.co2SnoozeUntil = Date().addingTimeInterval(3600)
        } else {
            state.co2SnoozeUntil = nil
        }
        state.save()
        #if canImport(WidgetKit)
        if #available(macOS 26.0, *) {
            ControlCenter.shared.reloadControls(ofKind: AranetBarAppGroup.muteControlKind)
        }
        #endif
        return .result()
    }
}

@available(macOS 14.0, *)
public struct SnoozeCo2FromWidgetIntent: AppIntent {
    public static var title: LocalizedStringResource = "Snooze CO₂ Alerts"
    public static var description = IntentDescription("Mutes CO₂ alerts for one hour from the widget.")

    public init() {}

    public func perform() async throws -> some IntentResult {
        try await ToggleCo2SnoozeIntent(muted: true).perform()
        return .result()
    }
}
