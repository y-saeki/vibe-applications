import SwiftUI

/// A screen with the shape of the visible frame, shrunk to `size`, and on it
/// where the row being edited puts a window: a tinted, rounded rectangle
/// whose loose edges, those of a kept width or height, are dashed.
struct PreviewView: View {
    static let size = CGSize(width: 360, height: 200)
    /// The library lays the picture out at this many units a point, so that
    /// edges that meet at a fraction of a point still meet.
    static let subpixel: CGFloat = 4

    let preview: PlacementPreview?

    var body: some View {
        Canvas { context, _ in
            guard let preview else { return }
            let screen = rect(preview.screen)
            let outline = Path(roundedRect: screen, cornerRadius: 2)
            context.fill(outline, with: .color(Color(nsColor: .textBackgroundColor)))
            context.stroke(outline, with: .color(Color(nsColor: .separatorColor)), lineWidth: 1)
            if let window = preview.window {
                draw(window, in: context)
            }
        }
        .frame(width: PreviewView.size.width, height: PreviewView.size.height)
    }

    private func rect(_ f: Frame) -> CGRect {
        let s = PreviewView.subpixel
        return CGRect(
            x: CGFloat(f.left) / s,
            y: CGFloat(f.top) / s,
            width: CGFloat(f.right - f.left) / s,
            height: CGFloat(f.bottom - f.top) / s
        )
    }

    /// The fill, the rounded corners, and each straight edge either solid or
    /// in dashes laid out by the library.
    private func draw(_ window: PreviewedWindow, in context: GraphicsContext) {
        let (stroke, radius, dash, gap): (CGFloat, CGFloat, Double, Double) = (2, 6, 6, 4)
        let accent = GraphicsContext.Shading.color(.accentColor)
        let r = rect(window.frame).insetBy(dx: stroke / 2, dy: stroke / 2)
        context.fill(Path(roundedRect: r, cornerRadius: radius), with: .color(.accentColor.opacity(0.22)))

        var lines = Path()
        let corners: [(CGPoint, Double)] = [
            (CGPoint(x: r.minX + radius, y: r.minY + radius), 180),
            (CGPoint(x: r.maxX - radius, y: r.minY + radius), 270),
            (CGPoint(x: r.maxX - radius, y: r.maxY - radius), 0),
            (CGPoint(x: r.minX + radius, y: r.maxY - radius), 90),
        ]
        for (center, start) in corners {
            var arc = Path()
            arc.addArc(center: center, radius: radius, startAngle: .degrees(start), endAngle: .degrees(start + 90), clockwise: false)
            lines.addPath(arc)
        }
        let edges: [(Bool, CGPoint, CGPoint)] = [
            (window.looseLeft, CGPoint(x: r.minX, y: r.minY + radius), CGPoint(x: r.minX, y: r.maxY - radius)),
            (window.looseTop, CGPoint(x: r.minX + radius, y: r.minY), CGPoint(x: r.maxX - radius, y: r.minY)),
            (window.looseRight, CGPoint(x: r.maxX, y: r.minY + radius), CGPoint(x: r.maxX, y: r.maxY - radius)),
            (window.looseBottom, CGPoint(x: r.minX + radius, y: r.maxY), CGPoint(x: r.maxX - radius, y: r.maxY)),
        ]
        for (loose, a, b) in edges {
            let vertical = a.x == b.x
            let from = Double(vertical ? a.y : a.x)
            let to = Double(vertical ? b.y : b.x)
            let pieces = loose ? dashes(from: from, to: to, dash: dash, gap: gap) : [Dash(from: from, to: to)]
            for piece in pieces {
                let start = vertical ? CGPoint(x: a.x, y: piece.from) : CGPoint(x: piece.from, y: a.y)
                let end = vertical ? CGPoint(x: a.x, y: piece.to) : CGPoint(x: piece.to, y: a.y)
                lines.move(to: start)
                lines.addLine(to: end)
            }
        }
        context.stroke(lines, with: accent, lineWidth: stroke)
    }
}
