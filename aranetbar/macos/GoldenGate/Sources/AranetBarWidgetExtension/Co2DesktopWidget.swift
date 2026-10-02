import AppIntents
import AranetBarShared
import SwiftUI
import WidgetKit

struct Co2Entry: TimelineEntry {
    let date: Date
    let state: GoldenGateState
}

struct Co2Provider: TimelineProvider {
    func placeholder(in context: Context) -> Co2Entry {
        Co2Entry(date: Date(), state: GoldenGateState())
    }

    func getSnapshot(in context: Context, completion: @escaping (Co2Entry) -> Void) {
        completion(Co2Entry(date: Date(), state: GoldenGateState.load()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<Co2Entry>) -> Void) {
        let state = GoldenGateState.load()
        let entry = Co2Entry(date: Date(), state: state)
        let interval = max(60, state.measurementIntervalSecs)
        let next = Date().addingTimeInterval(TimeInterval(interval))
        let policy = TimelineReloadPolicy.after(next)
        completion(Timeline(entries: [entry], policy: policy))
    }
}

struct Co2DesktopWidgetView: View {
    var entry: Co2Entry

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(entry.state.pinnedNickname.isEmpty ? "CO₂" : entry.state.pinnedNickname)
                .font(.caption)
                .foregroundStyle(.secondary)
            if let ppm = entry.state.latestCo2 {
                Text("\(ppm)")
                    .font(.system(size: 28, weight: .semibold, design: .rounded))
                Text("ppm · \(entry.state.tone)")
                    .font(.caption2)
                    .foregroundStyle(toneColor(entry.state.tone))
            } else {
                Text("—")
                    .font(.title2)
                Text("Waiting for BLE")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            if entry.state.isSnoozed {
                Label("Alerts muted", systemImage: "bell.slash")
                    .font(.caption2)
                    .foregroundStyle(.orange)
            } else if #available(macOS 14.0, *) {
                Button(intent: SnoozeCo2FromWidgetIntent()) {
                    Label("Mute 1h", systemImage: "bell.slash")
                        .font(.caption2)
                }
                .buttonStyle(.plain)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .padding(8)
    }

    func toneColor(_ tone: String) -> Color {
        switch tone {
        case "fresh": return .green
        case "rising": return .orange
        case "high": return .red
        default: return .secondary
        }
    }
}

struct Co2DesktopWidget: Widget {
    let kind = "com.20deg.aranetbar.co2-widget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: Co2Provider()) { entry in
            Co2DesktopWidgetView(entry: entry)
                .containerBackground(.fill.tertiary, for: .widget)
        }
        .configurationDisplayName("AranetBar CO₂")
        .description("Pinned sensor CO₂ with tone color.")
        .supportedFamilies([.systemSmall])
    }
}

@main
struct AranetBarWidgetBundle: WidgetBundle {
    var body: some WidgetBundle {
        Co2DesktopWidget()
    }
}
