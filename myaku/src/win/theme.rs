//! The colours Windows is set to: light or dark, for apps and for the
//! taskbar, and the accent colour.

use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RRF_RT_REG_DWORD, RegGetValueW,
};
use windows::core::{PCWSTR, w};

const PERSONALIZE: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
const ACCENT: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent");

/// An sRGB colour with alpha, each 0.0 to 1.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Colour {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn alpha(self, a: f32) -> Self {
        Self {
            a: self.a * a,
            ..self
        }
    }
}

/// What the list is drawn in.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub text: Colour,
    pub secondary: Colour,
    /// Behind each graph.
    pub track: Colour,
    /// Behind a button under the pointer.
    pub hover: Colour,
    /// The window, where Windows draws no backdrop of its own.
    pub background: Colour,
    pub accent: Colour,
}

impl Palette {
    /// The colours of WinUI's flyouts, in the app mode Windows is set to.
    pub fn current() -> Self {
        let dark = apps_use_dark();
        let accent = accent(dark);
        if dark {
            let white = Colour::rgb(0xFF, 0xFF, 0xFF);
            Self {
                dark,
                text: white,
                secondary: white.alpha(0.77),
                track: white.alpha(0.06),
                hover: white.alpha(0.08),
                background: Colour::rgb(0x2C, 0x2C, 0x2C),
                accent,
            }
        } else {
            let black = Colour::rgb(0, 0, 0);
            Self {
                dark,
                text: black.alpha(0.9),
                secondary: black.alpha(0.62),
                track: black.alpha(0.05),
                hover: black.alpha(0.06),
                background: Colour::rgb(0xF9, 0xF9, 0xF9),
                accent,
            }
        }
    }
}

fn dword(key: PCWSTR, name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(value)
}

/// Whether apps are set to dark mode.
pub fn apps_use_dark() -> bool {
    dword(PERSONALIZE, w!("AppsUseLightTheme")) == Some(0)
}

/// Whether the taskbar is dark, which decides the colour of the icon in it.
/// Before Windows 10 1903 it was always dark and the value is missing.
pub fn taskbar_is_dark() -> bool {
    dword(PERSONALIZE, w!("SystemUsesLightTheme")) != Some(1)
}

/// The accent colour in the shade WinUI fills with: the second lightest on
/// dark, the first darker one on light. Windows' default blue when the
/// palette cannot be read.
fn accent(dark: bool) -> Colour {
    // Eight colours of R, G, B, A, from the lightest to the darkest, the
    // accent itself the fourth.
    let mut palette = [0u8; 32];
    let mut size = palette.len() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            ACCENT,
            w!("AccentPalette"),
            RRF_RT_REG_BINARY,
            None,
            Some(palette.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() || size < 32 {
        return if dark {
            Colour::rgb(0x4C, 0xC2, 0xFF)
        } else {
            Colour::rgb(0x00, 0x5F, 0xB8)
        };
    }
    let i = if dark { 1 } else { 4 } * 4;
    Colour::rgb(palette[i], palette[i + 1], palette[i + 2])
}

/// Windows' build number: 22621 and later (Windows 11 22H2) draw the
/// translucent backdrop flyouts have. 0 when it cannot be read.
pub fn windows_build() -> u32 {
    let mut text = [0u16; 16];
    let mut size = (text.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
            w!("CurrentBuildNumber"),
            windows::Win32::System::Registry::RRF_RT_REG_SZ,
            None,
            Some(text.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return 0;
    }
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..len]).parse().unwrap_or(0)
}
