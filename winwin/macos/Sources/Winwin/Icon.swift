import AppKit

/// winwin's icons, drawn by the library: the app icon from the same drawing
/// as on Windows, and the menu-bar glyph.
enum Icon {
    /// The menu-bar glyph `points` square, sharp on a Retina screen. A
    /// template image: the menu bar colours it for light, dark and selected.
    static func menuBarImage(points: Int) -> NSImage {
        let image = NSImage(size: NSSize(width: points, height: points))
        if let rep = bitmap(menuBarIconRgba(size: UInt32(points * 2)), pixels: points * 2) {
            rep.size = image.size
            image.addRepresentation(rep)
        }
        image.isTemplate = true
        return image
    }

    private static func bitmap(_ rgba: Data, pixels: Int) -> NSBitmapImageRep? {
        guard let rep = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: pixels,
            pixelsHigh: pixels,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bitmapFormat: .alphaNonpremultiplied,
            bytesPerRow: pixels * 4,
            bitsPerPixel: 32
        ), let data = rep.bitmapData else { return nil }
        rgba.copyBytes(to: data, count: min(rgba.count, pixels * pixels * 4))
        return rep
    }

    /// Writes the images of an `.iconset` folder into `folder`, which
    /// build.sh turns into the app's icon with iconutil.
    static func writeIconset(to folder: URL) throws {
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        for points in [16, 32, 128, 256, 512] {
            for scale in [1, 2] {
                let name = scale == 1 ? "icon_\(points)x\(points).png" : "icon_\(points)x\(points)@2x.png"
                let pixels = points * scale
                guard let png = bitmap(appIconRgba(size: UInt32(pixels)), pixels: pixels)?
                    .representation(using: .png, properties: [:]) else {
                    continue
                }
                try png.write(to: folder.appendingPathComponent(name))
            }
        }
    }
}
