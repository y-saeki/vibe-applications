import AppKit

/// winwin's icon, drawn by the library from the same pixels as on Windows.
enum Icon {
    /// The icon `points` square, sharp on a Retina screen.
    static func image(points: Int) -> NSImage {
        let image = NSImage(size: NSSize(width: points, height: points))
        if let rep = bitmap(pixels: points * 2) {
            rep.size = image.size
            image.addRepresentation(rep)
        }
        return image
    }

    private static func bitmap(pixels: Int) -> NSBitmapImageRep? {
        let rgba = iconRgba(size: UInt32(pixels))
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
                guard let png = bitmap(pixels: points * scale)?.representation(using: .png, properties: [:]) else {
                    continue
                }
                try png.write(to: folder.appendingPathComponent(name))
            }
        }
    }
}
