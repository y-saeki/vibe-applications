import ApplicationServices
import Foundation

/// An application's menu bar, through the accessibility API: its items with
/// the shortcuts they show, and choosing one of them. Both need the
/// Accessibility permission meta-gesture already has.
enum Menus {
    /// What came of choosing a menu item.
    enum Pressed {
        case done
        /// The menu bar has no item with those titles.
        case missing
        /// The item is there but cannot be chosen now.
        case disabled
    }

    /// Menus nest no deeper than this; deeper is taken for a loop.
    private static let depth = 8

    /// The items of the menu bar of the process `pid` that do something,
    /// leaving out separators and the items that open a submenu. Asks the
    /// application for each item, so it is better called off the main
    /// thread.
    static func entries(of pid: pid_t) -> [MenuEntry] {
        guard let bar = menuBar(of: pid) else { return [] }
        var entries: [MenuEntry] = []
        for top in children(bar) {
            guard let title = title(top) else { continue }
            for menu in children(top) {
                collect(menu, path: [title], into: &entries)
            }
        }
        return entries
    }

    private static func collect(_ menu: AXUIElement, path: [String], into entries: inout [MenuEntry]) {
        guard path.count < depth else { return }
        for item in children(menu) {
            guard let title = title(item) else { continue }
            let path = path + [title]
            let submenus = children(item)
            if !submenus.isEmpty {
                for submenu in submenus {
                    collect(submenu, path: path, into: &entries)
                }
                continue
            }
            entries.append(MenuEntry(
                path: path,
                character: value(item, kAXMenuItemCmdCharAttribute) as? String,
                code: (value(item, kAXMenuItemCmdVirtualKeyAttribute) as? NSNumber).map { $0.uint16Value },
                modifiers: (value(item, kAXMenuItemCmdModifiersAttribute) as? NSNumber)?.uint32Value ?? 0
            ))
        }
    }

    /// Chooses the item of the menu bar of the process `pid` whose titles,
    /// from the menu bar down, are `path`.
    static func press(_ path: [String], in pid: pid_t) -> Pressed {
        guard let bar = menuBar(of: pid), let first = path.first,
              var element = children(bar).first(where: { title($0) == first })
        else { return .missing }
        for name in path.dropFirst() {
            // An item's children are its menus, whose children are items.
            guard let next = children(element).flatMap(children).first(where: { title($0) == name }) else {
                return .missing
            }
            element = next
        }
        if (value(element, kAXEnabledAttribute) as? Bool) == false {
            return .disabled
        }
        switch AXUIElementPerformAction(element, kAXPressAction as CFString) {
        // An application that answers once its menu command is done may
        // answer too late, without the command failing.
        case .success, .cannotComplete: return .done
        default: return .missing
        }
    }

    private static func menuBar(of pid: pid_t) -> AXUIElement? {
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 1)
        guard let bar = value(app, kAXMenuBarAttribute), CFGetTypeID(bar) == AXUIElementGetTypeID() else {
            return nil
        }
        return (bar as! AXUIElement)
    }

    private static func value(_ element: AXUIElement, _ attribute: String) -> CFTypeRef? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success else { return nil }
        return value
    }

    private static func children(_ element: AXUIElement) -> [AXUIElement] {
        (value(element, kAXChildrenAttribute) as? [AXUIElement]) ?? []
    }

    /// The title, unless the item has none, as a separator has not.
    private static func title(_ element: AXUIElement) -> String? {
        guard let title = value(element, kAXTitleAttribute) as? String,
              !title.trimmingCharacters(in: .whitespaces).isEmpty
        else { return nil }
        return title
    }
}
