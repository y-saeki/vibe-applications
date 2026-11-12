import SwiftUI
import UIKit

// Where the screens put their buttons follows two layouts:
//
// - Bars at the top and bottom, as on an iPhone or iPhone Duo's inner display
//   held upright. Buttons go where a thumb reaches them, at the bottom.
// - A vertical bar at the side, as on iPhone Duo's outer display or its inner
//   display held sideways. Buttons stay in the vertical bar.
//
// The in-app browser adds a third: on iPhone Duo's inner display held
// sideways it gives up the vertical bar for one along the top, so that the
// page takes the whole width and its middle sits in the middle of the screen.

extension ToolbarContent {
    /// Keeps the item in a horizontal bar, out of the vertical one, so that it
    /// stays at the top where the system would otherwise move it to the side.
    @ToolbarContentBuilder
    func horizontalBarOnly() -> some ToolbarContent {
        if #available(iOS 27.1, *) {
            axisBehavior(.horizontalOnly)
        } else {
            self
        }
    }
}

extension View {
    /// Lays the bars out along the top and bottom, even where the system
    /// would put them in a vertical bar at the side.
    @ViewBuilder
    func verticalBarDisabled(_ isDisabled: Bool) -> some View {
        if #available(iOS 27.1, *) {
            toolbarVerticalBehavior(isDisabled ? .disabled : .automatic)
        } else {
            self
        }
    }

    /// Presents a sheet in the middle of the screen rather than wherever the
    /// system would choose, which on iPhone Duo's inner display keeps the
    /// sheet's bars horizontal.
    @ViewBuilder
    func centeredPresentation() -> some View {
        if #available(iOS 27.0, *) {
            presentationPlacement(.center)
        } else {
            self
        }
    }

    /// Hides the system's back button, for a screen that puts its own
    /// `BackButton` in the bottom bar, and keeps the swipe from the screen's
    /// leading edge going back, which hiding the button turns off.
    func backButtonMoved(_ isMoved: Bool) -> some View {
        navigationBarBackButtonHidden(isMoved)
            .background(BackSwipeKeeper())
    }
}

/// The system's back button, for a bar of the screen's own choosing.
struct BackButton: View {
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        Button("戻る", systemImage: "chevron.backward") { dismiss() }
    }
}

/// Lets the navigation controller's back swipe start on a screen that hides
/// its back button. UIKit's own delegate for the swipe turns it down on such a
/// screen before asking whether it should begin, so this one takes its place
/// for good rather than passing questions on to it. Where the back button
/// shows, it still asks the original whether to begin.
private struct BackSwipeKeeper: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> Controller { Controller() }
    func updateUIViewController(_ uiViewController: Controller, context: Context) {}

    final class Controller: UIViewController {
        override func loadView() {
            view = UIView()
            view.isUserInteractionEnabled = false
        }

        override func viewDidAppear(_ animated: Bool) {
            super.viewDidAppear(animated)
            // The navigation controller may be reachable only once on screen.
            guard let navigationController,
                  let gesture = navigationController.interactivePopGestureRecognizer
            else { return }
            BackSwipeDelegate.install(on: gesture, of: navigationController)
        }
    }
}

private final class BackSwipeDelegate: NSObject, UIGestureRecognizerDelegate {
    private weak var navigationController: UINavigationController?
    private weak var original: (any UIGestureRecognizerDelegate)?

    private static var key: UInt8 = 0

    private init(navigationController: UINavigationController, original: (any UIGestureRecognizerDelegate)?) {
        self.navigationController = navigationController
        self.original = original
    }

    static func install(on gesture: UIGestureRecognizer, of navigationController: UINavigationController) {
        guard !(gesture.delegate is BackSwipeDelegate) else { return }
        let delegate = BackSwipeDelegate(navigationController: navigationController, original: gesture.delegate)
        // The recognizer holds its delegate weakly, so the recognizer keeps it here.
        objc_setAssociatedObject(gesture, &key, delegate, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        gesture.delegate = delegate
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard let navigationController,
              navigationController.viewControllers.count > 1,
              // Not while a push or pop is still running, which UIKit cannot take.
              navigationController.transitionCoordinator == nil
        else { return false }
        if navigationController.topViewController?.navigationItem.hidesBackButton == true {
            return true
        }
        return original?.gestureRecognizerShouldBegin?(gestureRecognizer) ?? true
    }
}
