// Run with `swift tests/native/focus-macos.swift` while TinyDash is running.
// Requires Accessibility access. Set TINYDASH_TEST_SHORTCUT for a custom shortcut.
import AppKit
import ApplicationServices

func waitFor(_ condition: () -> Bool, timeout: TimeInterval = 3) -> Bool {
    let deadline = Date().addingTimeInterval(timeout)
    while !condition() && Date() < deadline {
        if let event = NSApp.nextEvent(
            matching: .any, until: Date().addingTimeInterval(0.02),
            inMode: .default, dequeue: true
        ) {
            NSApp.sendEvent(event)
        }
        NSApp.updateWindows()
    }
    return condition()
}

func key(_ code: CGKeyCode, flags: CGEventFlags = []) {
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
        event.flags = flags
        event.post(tap: .cghidEventTap)
    }
}

func typeText(_ text: String) {
    for character in text {
        let units = Array(String(character).utf16)
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)!
            event.flags = []
            event.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
            event.post(tap: .cghidEventTap)
        }
    }
}

func containsText(_ element: AXUIElement, _ text: String, depth: Int = 0) -> Bool {
    guard depth < 20 else { return false }
    for attribute in [kAXValueAttribute, kAXTitleAttribute] {
        var value: CFTypeRef?
        if AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success,
            value as? String == text {
            return true
        }
    }
    var children: CFTypeRef?
    guard AXUIElementCopyAttributeValue(
        element, kAXChildrenAttribute as CFString, &children
    ) == .success, let elements = children as? [AXUIElement] else { return false }
    return elements.contains { containsText($0, text, depth: depth + 1) }
}

let shortcut = ProcessInfo.processInfo.environment["TINYDASH_TEST_SHORTCUT"]
    ?? "Control+Shift+Space"
var shortcutFlags: CGEventFlags = []
for part in shortcut.lowercased().split(separator: "+") {
    switch part {
    case "control", "ctrl": shortcutFlags.insert(.maskControl)
    case "shift": shortcutFlags.insert(.maskShift)
    case "super", "command", "cmd": shortcutFlags.insert(.maskCommand)
    case "alt", "option": shortcutFlags.insert(.maskAlternate)
    case "space": break
    default: fatalError("This check supports shortcuts that use Space.")
    }
}
guard shortcut.lowercased().hasSuffix("space") else {
    fatalError("This check supports shortcuts that use Space.")
}

guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else {
    fatalError("The test process needs Accessibility access to send keys.")
}
guard let launcher = NSRunningApplication.runningApplications(
    withBundleIdentifier: "dev.tinydash.launcher"
).first else {
    fatalError("Start TinyDash before running this check.")
}
let launcherElement = AXUIElementCreateApplication(launcher.processIdentifier)

let previousApp = NSWorkspace.shared.frontmostApplication
let pasteboard = NSPasteboard.general
let savedClipboard = (pasteboard.pasteboardItems ?? []).map { item in
    item.types.compactMap { type in item.data(forType: type).map { (type, $0) } }
}
var copiedCalculation = false
let app = NSApplication.shared
app.setActivationPolicy(.regular)
app.finishLaunching()
let otherWindow = NSWindow(
    contentRect: NSRect(x: 180, y: 180, width: 400, height: 140),
    styleMask: [.titled], backing: .buffered, defer: false
)
otherWindow.title = "TinyDash inactive test window"
otherWindow.makeKeyAndOrderFront(nil)
let window = NSWindow(
    contentRect: NSRect(x: 120, y: 120, width: 400, height: 140),
    styleMask: [.titled], backing: .buffered, defer: false
)
window.title = "TinyDash focus check"
let input = NSTextField(frame: NSRect(x: 20, y: 50, width: 360, height: 30))
window.contentView!.addSubview(input)
window.makeKeyAndOrderFront(nil)
window.makeFirstResponder(input)
app.activate(ignoringOtherApps: true)

func finish(_ failure: String? = nil) -> Never {
    window.orderOut(nil)
    otherWindow.orderOut(nil)
    if copiedCalculation {
        pasteboard.clearContents()
        pasteboard.writeObjects(savedClipboard.map { values in
            let item = NSPasteboardItem()
            for (type, data) in values { item.setData(data, forType: type) }
            return item
        })
    }
    previousApp?.activate(options: [.activateIgnoringOtherApps])
    if let failure {
        print("FAIL: \(failure)")
        exit(1)
    }
    exit(0)
}

func openPalette() {
    key(49, flags: shortcutFlags)
    guard waitFor({ launcher.isActive }) else {
        finish("The global shortcut did not open TinyDash.")
    }
    // Allow the palette's input to receive keys after native activation.
    _ = waitFor({ false }, timeout: 0.3)
}

func expectTyping(_ label: String, _ text: String) {
    guard waitFor({ app.isActive && window.isKeyWindow }) else {
        let active = NSWorkspace.shared.frontmostApplication?.localizedName ?? "none"
        finish("\(label) did not return focus. Active app: \(active).")
    }
    let expected = input.stringValue + text
    typeText(text)
    guard waitFor({ input.stringValue == expected }) else {
        finish("Typing did not continue in the previous input after \(label).")
    }
    print("PASS: \(label) returns focus and typing to the previous window.")
}

guard waitFor({ app.isActive && window.isKeyWindow }) else {
    finish("The test input did not receive focus. Active: \(app.isActive), key: \(window.isKeyWindow).")
}
key(0) // A
guard waitFor({ input.stringValue == "a" }) else {
    finish("The test input did not receive the initial key.")
}
for attempt in 1...3 {
    openPalette()
    key(53) // Escape
    expectTyping("Escape, attempt \(attempt)", "b")
}

openPalette()
key(49, flags: shortcutFlags)
expectTyping("The global shortcut", "c")

openPalette()
let reopen = Process()
reopen.executableURL = launcher.executableURL
try reopen.run()
guard waitFor({ !reopen.isRunning }), reopen.terminationStatus == 0 else {
    finish("The second launch did not reach the running app.")
}
_ = waitFor({ false }, timeout: 0.3)
key(53)
expectTyping("Escape after a second launch", "d")

openPalette()
key(0, flags: [.maskCommand]) // Select any saved query.
typeText("12 * 8")
guard waitFor({ containsText(launcherElement, "96") }) else {
    finish("The palette did not show the calculation result.")
}
key(36) // Enter copies the selected result and closes the palette.
copiedCalculation = true
guard waitFor({ pasteboard.string(forType: .string) == "96" }) else {
    finish("The calculation result was not copied.")
}
expectTyping("Copying a calculation", "e")
key(53) // Escape is harmless when copying already closed the palette.
expectTyping("Escape after copying", "f")

openPalette()
key(43, flags: [.maskCommand]) // Command+comma opens Settings.
guard waitFor({ containsText(launcherElement, "TinyDash Settings") }) else {
    finish("Settings did not open.")
}
_ = waitFor({ false }, timeout: 0.3)
guard launcher.isActive && !app.isActive else {
    finish("Opening Settings returned focus to the previous app.")
}
print("PASS: Opening Settings keeps focus in TinyDash.")
var settingsWindow: CFTypeRef?
guard AXUIElementCopyAttributeValue(
    launcherElement, kAXFocusedWindowAttribute as CFString, &settingsWindow
) == .success, let settingsWindow else {
    finish("Settings has no focused window.")
}
var closeButton: CFTypeRef?
guard AXUIElementCopyAttributeValue(
    settingsWindow as! AXUIElement, kAXCloseButtonAttribute as CFString, &closeButton
) == .success, let closeButton else {
    finish("Settings has no close button.")
}
guard AXUIElementPerformAction(closeButton as! AXUIElement, kAXPressAction as CFString) == .success else {
    finish("Could not close Settings.")
}
app.activate(ignoringOtherApps: true)
guard waitFor({ app.isActive && window.isKeyWindow }) else {
    finish("The test input did not regain focus after Settings.")
}

openPalette()
let switchApp = Process()
switchApp.executableURL = URL(fileURLWithPath: "/usr/bin/open")
switchApp.arguments = ["-a", "Finder"]
try switchApp.run()
guard waitFor({ NSWorkspace.shared.frontmostApplication?.bundleIdentifier == "com.apple.finder" }) else {
    finish("The app switch did not activate Finder.")
}
_ = waitFor({ false }, timeout: 0.3)
guard NSWorkspace.shared.frontmostApplication?.bundleIdentifier == "com.apple.finder",
    !app.isActive && !launcher.isActive else {
    finish("Hiding on blur changed the selected app.")
}
print("PASS: Switching apps keeps focus in the selected app.")
finish()
