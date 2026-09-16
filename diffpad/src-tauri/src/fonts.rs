//! System font family enumeration for the Preferences panel.

/// Sorted, de-duplicated list of installed font family names.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn families() -> Vec<String> {
    use font_kit::source::SystemSource;

    let mut families = SystemSource::new().all_families().unwrap_or_default();
    // CoreText reports hidden system faces whose names start with a dot.
    families.retain(|name| !name.starts_with('.'));
    families.sort_by_cached_key(|name| name.to_lowercase());
    families.dedup();
    families
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn families() -> Vec<String> {
    Vec::new()
}
