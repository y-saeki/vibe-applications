//! The executable's icon as a compiled resource file (.res), which the MSVC
//! linker takes as an input like an object file. build.rs writes it from
//! art.rs, so that no resource compiler and no icon file is needed.
//!
//! The format is a series of resources, each a header followed by its data,
//! both padded to 4 bytes: an empty one first, then one RT_ICON per size and
//! an RT_GROUP_ICON that lists them. The group's id is 1, the lowest, so
//! Windows takes it as the executable's icon.

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
/// MOVEABLE | PURE | DISCARDABLE, and MOVEABLE | DISCARDABLE: what rc.exe
/// writes for these two types.
const ICON_FLAGS: u16 = 0x1010;
const GROUP_FLAGS: u16 = 0x1030;
const GROUP_ID: u16 = 1;

/// The sizes drawn: the small and large icons at 100% to 200%, and the
/// 256-pixel one Explorer shows in its large views.
pub const SIZES: [usize; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

fn resource(out: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]) {
    let header_size: u32 = 32;
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(header_size.to_le_bytes());
    // Type and name as ordinals.
    out.extend([0xFFFF, kind, 0xFFFF, id].map(u16::to_le_bytes).concat());
    out.extend(0u32.to_le_bytes()); // DataVersion
    out.extend(flags.to_le_bytes());
    out.extend(0u16.to_le_bytes()); // LanguageId: neutral
    out.extend(0u32.to_le_bytes()); // Version
    out.extend(0u32.to_le_bytes()); // Characteristics
    out.extend(data);
    out.resize(out.len().next_multiple_of(4), 0);
}

/// One icon image: a BITMAPINFOHEADER of twice the height (the color image
/// and the mask), the color rows bottom up, then a mask of zeros, since the
/// alpha channel decides what is transparent.
fn icon_image(size: usize, argb: &[u32]) -> Vec<u8> {
    let mask_row = size.div_ceil(32) * 4;
    let image_size = size * size * 4 + mask_row * size;
    let mut out = Vec::with_capacity(40 + image_size);
    out.extend(40u32.to_le_bytes());
    out.extend((size as i32).to_le_bytes());
    out.extend((size as i32 * 2).to_le_bytes());
    out.extend(1u16.to_le_bytes()); // planes
    out.extend(32u16.to_le_bytes()); // bits per pixel
    out.extend(0u32.to_le_bytes()); // BI_RGB
    out.extend((image_size as u32).to_le_bytes());
    out.extend([0u8; 16]); // resolution and palette: unused
    for row in argb.chunks(size).rev() {
        // 0xAARRGGBB, little endian, is the B, G, R, A the format wants.
        for px in row {
            out.extend(px.to_le_bytes());
        }
    }
    out.resize(out.len() + mask_row * size, 0);
    out
}

/// The .res for an icon drawn by `draw` (pixels for an edge, top-down ARGB)
/// at each of [`SIZES`].
pub fn icon_resource(draw: impl Fn(usize) -> Vec<u32>) -> Vec<u8> {
    let mut out = Vec::new();
    resource(&mut out, 0, 0, 0, &[]);
    let mut group = Vec::new();
    group.extend([0u16, 1, SIZES.len() as u16].map(u16::to_le_bytes).concat());
    for (i, &size) in SIZES.iter().enumerate() {
        let id = i as u16 + 1;
        let image = icon_image(size, &draw(size));
        resource(&mut out, RT_ICON, id, ICON_FLAGS, &image);
        // 256 is written as 0 in the byte-sized width and height.
        let edge = if size >= 256 { 0 } else { size as u8 };
        group.extend([edge, edge, 0, 0]);
        group.extend(1u16.to_le_bytes()); // planes
        group.extend(32u16.to_le_bytes()); // bits per pixel
        group.extend((image.len() as u32).to_le_bytes());
        group.extend(id.to_le_bytes());
    }
    resource(&mut out, RT_GROUP_ICON, GROUP_ID, GROUP_FLAGS, &group);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_at(b: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([b[at], b[at + 1]])
    }

    fn u32_at(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    /// Walks the file: (type, name, data) of every resource.
    fn resources(b: &[u8]) -> Vec<(u16, u16, &[u8])> {
        let mut at = 0;
        let mut found = Vec::new();
        while at < b.len() {
            let data = u32_at(b, at) as usize;
            let header = u32_at(b, at + 4) as usize;
            assert_eq!(header, 32);
            assert_eq!((u16_at(b, at + 8), u16_at(b, at + 12)), (0xFFFF, 0xFFFF));
            let start = at + header;
            found.push((
                u16_at(b, at + 10),
                u16_at(b, at + 14),
                &b[start..start + data],
            ));
            at = (start + data).next_multiple_of(4);
        }
        found
    }

    #[test]
    fn lists_every_size_in_the_group() {
        let res = icon_resource(|size| vec![0xFF_00_00_FF; size * size]);
        let found = resources(&res);
        assert_eq!(found.len(), SIZES.len() + 2);
        assert_eq!((found[0].0, found[0].2.len()), (0, 0));
        let (kind, id, group) = *found.last().unwrap();
        assert_eq!((kind, id), (RT_GROUP_ICON, GROUP_ID));
        assert_eq!(u16_at(group, 4) as usize, SIZES.len());
        for (i, &size) in SIZES.iter().enumerate() {
            let entry = &group[6 + i * 14..6 + (i + 1) * 14];
            let (kind, icon_id, image) = found[i + 1];
            assert_eq!(kind, RT_ICON);
            assert_eq!(entry[0], if size == 256 { 0 } else { size as u8 });
            assert_eq!(u32_at(entry, 8) as usize, image.len());
            assert_eq!(u16_at(entry, 12), icon_id);
            // The header's height covers the image and its mask.
            assert_eq!(u32_at(image, 8) as usize, size * 2);
        }
    }

    #[test]
    fn rows_are_stored_bottom_up() {
        // A 16-pixel icon whose top row alone is red.
        let res = icon_resource(|size| {
            let mut p = vec![0u32; size * size];
            p[..size].fill(0xFF_FF_00_00);
            p
        });
        let image = resources(&res)[1].2;
        let row = 16 * 4;
        let last = &image[40 + 15 * row..40 + 16 * row];
        assert_eq!(&last[..4], &[0x00, 0x00, 0xFF, 0xFF]);
        assert!(image[40..40 + row].iter().all(|&b| b == 0));
    }
}
