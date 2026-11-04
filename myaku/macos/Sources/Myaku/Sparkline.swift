import SwiftUI

/// The last minute as a filled graph on a rounded track. Values are in
/// layers: 1.0 fills the graph once, and what is above draws over it in a
/// stronger shade (for CPU, one layer per core).
struct Sparkline: View {
    let values: [Float]

    /// The first layer a little translucent, the middle two lighter over
    /// it, and the fourth solid.
    private static let opacities: [Double] = [0.7, 0.5, 0.5, 1.0]

    var body: some View {
        let peak = values.max() ?? 0
        let layers = Int(chartLayers())
        ZStack {
            Color.primary.opacity(0.06)
            ForEach(0..<min(layers, Self.opacities.count), id: \.self) { layer in
                if peak > Float(layer) {
                    Area(values: values, layer: Float(layer))
                        .fill(Color.accentColor.opacity(Self.opacities[layer]))
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 3, style: .continuous))
    }

    private struct Area: Shape {
        let values: [Float]
        let layer: Float

        func path(in rect: CGRect) -> Path {
            var path = Path()
            guard values.count > 1 else { return path }
            let step = rect.width / CGFloat(values.count - 1)
            path.move(to: CGPoint(x: rect.minX, y: rect.maxY))
            for (i, value) in values.enumerated() {
                let level = CGFloat(min(max(value - layer, 0), 1))
                path.addLine(to: CGPoint(x: rect.minX + CGFloat(i) * step, y: rect.maxY - level * rect.height))
            }
            path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
            path.closeSubpath()
            return path
        }
    }
}
