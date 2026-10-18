import AppKit

/// winwin's icon, drawn by the library from the same pixels as on Windows.
enum Icon {
    /// The icon `points` square, sharp on a Retina screen.
    static func image(points: Int) -> NSImage {
        let pixels = points * 2
        let rgba = iconRgba(size: UInt32(pixels))
        let image = NSImage(size: NSSize(width: points, height: points))
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
        ), let data = rep.bitmapData else { return image }
        rgba.copyBytes(to: data, count: min(rgba.count, pixels * pixels * 4))
        rep.size = image.size
        image.addRepresentation(rep)
        return image
    }
}
