//! Keeping AppKit's own items out of the Edit menu (macOS only).
//!
//! AppKit adds four items to whichever menu it takes for the Edit menu,
//! however that menu was built: Writing Tools, AutoFill, "Start Dictation…"
//! and "Emoji & Symbols". None of them has anything to do with a draft, and
//! they are the menu bar's share of what issue #76 asked to be cleared away.
//!
//! Two of them have a documented switch, a default read while the app starts.
//! The other two have none, so they are taken off the menu once the app has
//! finished launching — which is when AppKit has finished putting them there,
//! and therefore the earliest moment they can be removed.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSMenu};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSUserDefaults};

/// Keeps "Start Dictation…" and "Emoji & Symbols" off the menu.
///
/// Registered rather than written, so that none of this lands in draftpad's
/// own preferences file. Has to run before AppKit fills the menu in.
pub fn disable_input_items() {
    let yes = NSNumber::new_bool(true);
    let yes: &AnyObject = &yes;
    let values = NSDictionary::from_slices(
        &[
            &*NSString::from_str("NSDisabledDictationMenuItem"),
            &*NSString::from_str("NSDisabledCharacterPaletteMenuItem"),
        ],
        &[yes, yes],
    );
    // SAFETY: registering defaults only reads the dictionary handed to it.
    unsafe { NSUserDefaults::standardUserDefaults().registerDefaults(&values) };
}

/// Takes off whatever AppKit added to the `menu_title` menu after draftpad's
/// own last item, the one titled `last`.
///
/// Everything past that item is AppKit's, so this does not need to know what
/// it is taking off — which is just as well, since the two items with no
/// switch are also the two with no documented name.
pub fn trim(menu_title: &str, last: &str) {
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("draftpad: the {menu_title} menu can only be trimmed on the main thread");
        return;
    };
    let Some(menu) = submenu(mtm, menu_title) else {
        eprintln!("draftpad: failed to find the {menu_title} menu");
        return;
    };
    let ours = menu.indexOfItemWithTitle(&NSString::from_str(last));
    if ours < 0 {
        eprintln!("draftpad: failed to find {last} in the {menu_title} menu");
        return;
    }
    while menu.numberOfItems() > ours + 1 {
        menu.removeItemAtIndex(menu.numberOfItems() - 1);
    }
}

fn submenu(mtm: MainThreadMarker, title: &str) -> Option<Retained<NSMenu>> {
    NSApplication::sharedApplication(mtm)
        .mainMenu()?
        .itemWithTitle(&NSString::from_str(title))?
        .submenu()
}
