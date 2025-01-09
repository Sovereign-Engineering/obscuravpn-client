import Cocoa
import SwiftUI

// https://github.com/j-f1/MenuBuilder/blob/ba0202c5ff6d63f0fd7ec6b1da11a769eff15000/Sources/MenuBuilder/MenuItemView.swift#L59 (MIT)
// https://github.com/attheodo/Pingu/blob/affc3e4ccf88962d4bbb98dbef774c35801102e6/Pingu/Source/Views/HostMenuItemView/HostMenuItemView.swift
// https://developer.apple.com/documentation/appkit/nsvisualeffectview
// https://developer.apple.com/documentation/appkit/nsview/1514865-enclosingmenuitem

class MenuItemView<ContentView: View>: NSView {
    private let effectView: NSVisualEffectView
    let contentView: ContentView
    let hostView: NSHostingView<AnyView>

    init(_ view: ContentView) {
        self.effectView = NSVisualEffectView()

        self.effectView.state = .active
        self.effectView.material = .selection
        self.effectView.isEmphasized = true
        self.effectView.blendingMode = .behindWindow
        self.effectView.wantsLayer = true
        self.effectView.layer?.cornerRadius = 4
        self.effectView.layer?.cornerCurve = .continuous

        // only enable when highlighted
        self.effectView.isHidden = true

        self.contentView = view
        self.hostView = NSHostingView(rootView: AnyView(self.contentView))

        let frame = CGRect(origin: .zero, size: hostView.fittingSize)

        super.init(frame: frame)

        addSubview(self.effectView)
        addSubview(self.hostView)

        self.setUpConstraints()
    }

    @available(*, unavailable)
    required init?(coder decoder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil {
            frame = NSRect(
                origin: frame.origin,
                size: CGSize(width: enclosingMenuItem!.menu!.size.width, height: frame.height)
            )

            self.effectView.frame = NSRect(
                origin: CGPoint(x: frame.origin.x + 5, y: frame.origin.y),
                size: CGSize(width: enclosingMenuItem!.menu!.size.width - 10, height: frame.height)
            )
            self.hostView.frame = frame
        }
    }

    // https://stackoverflow.com/q/6054331/7732434
    override func draw(_ dirtyRect: NSRect) {
        // Without this, it is possible for a Toggle/NSSwitch inside the status
        // menu dropdown to appear "inactive". That is, without the app tint
        // and greyed-out, even when the Toggle is in the "ON" position.
        //
        // This fix was discovered by observing that the only reliable
        // difference between instances where the Toggle was and wasn't tinted
        // was whether the `NSStatusBarWindow` (a private API class) had
        // `isKeyWindow` true or false.
        //
        // References for possibly related problems and references:
        //   - https://developer.apple.com/documentation/swiftui/environmentvalues/controlactivestate
        //   - https://stackoverflow.com/a/59655207
        //   - https://medium.com/@acwrightdesign/creating-a-macos-menu-bar-application-using-swiftui-54572a5d5f87
        if let window = self.window {
            if window.isVisible {
                window.becomeKey()
            }
        }
        // NOTE: an action must be defined in the NSMenuItem
        // Sample usage; let menuItem = NSMenuItem(title: "", action: #selector(menuItemAction), keyEquivalent: "")
        let highlighted = enclosingMenuItem?.isHighlighted ?? false
        self.effectView.isHidden = !highlighted
        // Note: I removed rehosting the view depending on highlighting
        // I removed it because it would
        // // NOTE: I removed it because on the first ever draw of the toggle, the vpn state would be visibly delayed by 0.5s
        // if we ever want our subview to know if it's highlighted, we can use its own .onHover,
        //  or for broader highlighting: `@Binding var menuItemIsHighlighted`
        //  @State var menuItemIsHighlighted = false
        //  which does require providing this class with the view struct and not an instance
        super.draw(dirtyRect)
    }

    private func setUpConstraints() {
        self.effectView.translatesAutoresizingMaskIntoConstraints = false
        self.hostView.translatesAutoresizingMaskIntoConstraints = false
        translatesAutoresizingMaskIntoConstraints = false

        let margin: CGFloat = 5
        self.effectView.topAnchor.constraint(equalTo: topAnchor).isActive = true
        self.effectView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: margin).isActive = true
        self.effectView.bottomAnchor.constraint(equalTo: bottomAnchor).isActive = true
        self.effectView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -margin).isActive = true

        self.hostView.trailingAnchor.constraint(equalTo: trailingAnchor).isActive = true
        self.hostView.leadingAnchor.constraint(equalTo: leadingAnchor).isActive = true
        self.hostView.topAnchor.constraint(equalTo: topAnchor).isActive = true
        self.hostView.bottomAnchor.constraint(equalTo: bottomAnchor).isActive = true
    }
}
