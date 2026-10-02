import AranetBarShared
import SwiftUI
import WidgetKit

@available(macOS 26.0, *)
struct MuteCo2Control: ControlWidget {
    static let kind = AranetBarAppGroup.muteControlKind

    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: Self.kind) {
            ControlWidgetToggle(
                "Mute CO₂",
                isOn: MuteCo2ValueProvider(),
                action: ToggleCo2SnoozeControlIntent()
            ) { isOn in
                Label(isOn ? "CO₂ muted" : "CO₂ alerts", systemImage: isOn ? "bell.slash.fill" : "bell.fill")
            }
        }
        .displayName("Mute CO₂ Alerts")
        .description("Silence CO₂ notifications while BLE logging continues.")
    }
}

@available(macOS 26.0, *)
struct MuteCo2ValueProvider: ControlValueProvider {
    func currentValue() async throws -> Bool {
        GoldenGateState.load().isSnoozed
    }

    func previewValue() -> Bool {
        false
    }
}

@available(macOS 26.0, *)
struct ToggleCo2SnoozeControlIntent: SetValueIntent {
    static var title: LocalizedStringResource = "Toggle CO₂ mute"

    @Parameter(title: "Muted")
    var value: Bool

    func perform() async throws -> some IntentResult {
        try await ToggleCo2SnoozeIntent(muted: value).perform()
        return .result()
    }
}

@available(macOS 26.0, *)
@main
struct AranetBarControlBundle: ControlWidgetBundle {
    var body: some WidgetBundle {
        MuteCo2Control()
    }
}
