import AppKit

private func installRoundedCorners(on window: NSWindow) -> Bool {
    guard let contentView = window.contentView else {
        return false
    }

    window.isOpaque = false
    window.backgroundColor = .clear
    contentView.wantsLayer = true
    guard let layer = contentView.layer else {
        return false
    }
    layer.cornerRadius = 10
    layer.cornerCurve = .continuous
    layer.masksToBounds = true
    contentView.needsLayout = true
    contentView.layoutSubtreeIfNeeded()
    contentView.needsDisplay = true
    contentView.displayIfNeeded()
    window.invalidateShadow()
    return true
}

@_cdecl("gds3d_install_window_style")
public func installWindowStyle(_ windowPointer: UnsafeMutableRawPointer?) -> Int32 {
    guard let windowPointer else {
        return 0
    }
    let update = {
        let window = Unmanaged<NSWindow>.fromOpaque(windowPointer).takeUnretainedValue()
        return installRoundedCorners(on: window)
    }
    let succeeded = Thread.isMainThread ? update() : DispatchQueue.main.sync(execute: update)
    return succeeded ? 1 : 0
}
