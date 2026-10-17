//! eleven-zip's icon as pixels, drawn from `icon.svg`: a red zipper on
//! white, over crossing orange and green bands. build.rs draws it into the executable's icon at
//! every size the icon resource holds; everything else (Explorer, the
//! taskbar, the context menu, the settings window's title bar) takes it from
//! there.
//!
//! Nothing here touches Win32, so it is tested on every platform, and
//! build.rs includes this file as it is.

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

const SVG: &[u8] = include_bytes!("../icon.svg");

/// The icon at `size` × `size` pixels, top-down ARGB with straight (not
/// premultiplied) alpha, as an icon resource stores it.
pub fn pixels(size: usize) -> Vec<u32> {
    let tree = Tree::from_data(SVG, &Options::default()).expect("icon.svg is valid SVG");
    let mut pixmap = Pixmap::new(size as u32, size as u32).expect("the size is not zero");
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .pixels()
        .iter()
        .map(|p| {
            let c = p.demultiply();
            u32::from_be_bytes([c.alpha(), c.red(), c.green(), c.blue()])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORANGE: u32 = 0xFF_F0_83_00;
    const GREEN: u32 = 0xFF_01_88_37;
    const RED: u32 = 0xFF_FF_00_01;
    const WHITE: u32 = 0xFF_FF_FF_FF;

    #[test]
    fn the_bands_and_the_zipper() {
        let size = 256;
        let p = pixels(size);
        let at = |x: usize, y: usize| p[y * size + x];
        assert_eq!(p.len(), size * size);
        // Rounded corners stay transparent.
        assert_eq!(at(0, 0) >> 24, 0);
        assert_eq!(at(size - 1, size - 1) >> 24, 0);
        // White at the top, orange on the left above green at the bottom,
        // the zipper in the middle.
        assert_eq!(at(30, 20), WHITE);
        assert_eq!(at(30, 150), ORANGE);
        assert_eq!(at(128, 236), GREEN);
        assert_eq!(at(128, 128), RED);
    }

    #[test]
    fn every_size_is_drawn_whole() {
        for size in [16, 32, 48] {
            let p = pixels(size);
            assert_eq!(p.len(), size * size);
            // The middle is opaque at every size.
            assert_eq!(p[size / 2 * size + size / 2] >> 24, 0xFF, "{size}");
        }
    }
}
