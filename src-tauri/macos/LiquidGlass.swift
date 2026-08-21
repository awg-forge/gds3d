import AppKit

private let glassIdentifier = NSUserInterfaceItemIdentifier("SeaLanternLiquidGlass")
// window-vibrancy uses this tag for the NSVisualEffectView installed by Tauri.
private let tauriVibrancyTag = 91_376_254

private func clearTauriVibrancy(from window: NSWindow) {
    while let vibrancyView = window.contentView?.viewWithTag(tauriVibrancyTag) {
        vibrancyView.removeFromSuperview()
    }
}

@available(macOS 26.0, *)
private func installLiquidGlass(on window: NSWindow) -> Bool {
    if let glassView = window.contentView as? NSGlassEffectView,
       glassView.identifier == glassIdentifier
    {
        return true
    }
    guard let contentView = window.contentView else {
        return false
    }

    let glassView = NSGlassEffectView(frame: contentView.frame)
    glassView.identifier = glassIdentifier
    glassView.style = .regular
    window.isOpaque = false
    window.backgroundColor = .clear
    window.contentView = glassView
    glassView.contentView = contentView
    glassView.needsLayout = true
    glassView.layoutSubtreeIfNeeded()
    glassView.needsDisplay = true
    glassView.displayIfNeeded()
    window.invalidateShadow()
    return true
}

@available(macOS 26.0, *)
private func removeLiquidGlass(from window: NSWindow) -> Bool {
    guard
        let glassView = window.contentView as? NSGlassEffectView,
        glassView.identifier == glassIdentifier,
        let contentView = glassView.contentView
    else {
        return true
    }

    glassView.contentView = nil
    window.contentView = contentView
    contentView.needsLayout = true
    contentView.layoutSubtreeIfNeeded()
    contentView.needsDisplay = true
    contentView.displayIfNeeded()
    window.invalidateShadow()
    return true
}

@_cdecl("sealantern_supports_liquid_glass")
public func supportsLiquidGlass() -> Int32 {
    if #available(macOS 26.0, *) {
        return 1
    }
    return 0
}

@_cdecl("sealantern_set_liquid_glass")
public func setLiquidGlass(
    _ windowPointer: UnsafeMutableRawPointer?,
    _ enabled: Int32
) -> Int32 {
    guard let windowPointer else {
        return 0
    }
    let update = {
        let window = Unmanaged<NSWindow>.fromOpaque(windowPointer).takeUnretainedValue()
        clearTauriVibrancy(from: window)
        if #available(macOS 26.0, *) {
            return enabled == 0
                ? removeLiquidGlass(from: window)
                : installLiquidGlass(on: window)
        }
        return enabled == 0
    }
    let succeeded = Thread.isMainThread ? update() : DispatchQueue.main.sync(execute: update)
    return succeeded ? 1 : 0
}
