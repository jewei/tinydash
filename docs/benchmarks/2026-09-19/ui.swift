import AppKit
import ApplicationServices

func attr(_ e: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(e, name as CFString, &value) == .success else { return nil }
    return value
}

func nodes(_ e: AXUIElement, depth: Int = 0) -> [[String: String]] {
    guard depth < 22 else { return [] }
    var row: [String: String] = ["depth": String(depth)]
    for name in ["AXRole", "AXTitle", "AXValue", "AXDescription", "AXIdentifier"] {
        if let value = attr(e, name) as? String, !value.isEmpty { row[name] = value }
    }
    var result = [row]
    for child in attr(e, "AXChildren") as? [AXUIElement] ?? [] {
        result += nodes(child, depth: depth + 1)
    }
    return result
}

func hasOutput(_ e: AXUIElement, expected: String, depth: Int = 0) -> Bool {
    guard depth < 22 else { return false }
    let role = attr(e, "AXRole") as? String ?? ""
    if !["AXTextField", "AXTextArea", "AXComboBox"].contains(role) {
        for name in ["AXValue", "AXTitle", "AXDescription"] {
            if let value = attr(e, name) as? String,
               value == expected || value == "= " + expected { return true }
        }
    }
    // Do not match a search field's internal text descendants.
    if ["AXTextField", "AXTextArea", "AXComboBox"].contains(role) { return false }
    for child in attr(e, "AXChildren") as? [AXUIElement] ?? [] {
        if hasOutput(child, expected: expected, depth: depth + 1) { return true }
    }
    return false
}

func onScreen(_ pid: pid_t) -> Bool {
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.contains { window in
        guard window[kCGWindowOwnerPID as String] as? Int == Int(pid),
              let bounds = window[kCGWindowBounds as String] as? [String: Any],
              let width = bounds["Width"] as? Double, let height = bounds["Height"] as? Double
        else { return false }
        return width > 200 && height > 60
    }
}

func hasInput(_ e: AXUIElement, depth: Int = 0) -> Bool {
    guard depth < 22 else { return false }
    let role = attr(e, "AXRole") as? String ?? ""
    if ["AXTextField", "AXTextArea", "AXComboBox"].contains(role) { return true }
    return (attr(e, "AXChildren") as? [AXUIElement] ?? []).contains { hasInput($0, depth: depth + 1) }
}

func key(_ code: CGKeyCode, flags: CGEventFlags = [], pid: pid_t? = nil) {
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
        event.flags = flags
        if let pid { event.postToPid(pid) } else { event.post(tap: .cghidEventTap) }
    }
}

func typeText(_ value: String, pid: pid_t) {
    let units = Array(value.utf16)
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)!
        event.flags = []
        event.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
        event.postToPid(pid)
    }
}

func replace(_ value: String, pid: pid_t) {
    key(0, flags: .maskCommand, pid: pid)
    typeText(value, pid: pid)
}

func waitFor(_ predicate: () -> Bool, seconds: Double = 3) -> Bool {
    let end = ProcessInfo.processInfo.systemUptime + seconds
    repeat {
        if predicate() { return true }
        Thread.sleep(forTimeInterval: 0.002)
    } while ProcessInfo.processInfo.systemUptime < end
    return false
}

func output(_ value: Any) {
    let data = try! JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    print(String(data: data, encoding: .utf8)!)
}

guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else {
    output(["error": "Accessibility and event access are required"])
    exit(1)
}
let args = CommandLine.arguments
guard args.count >= 3, let pid = pid_t(args[2]),
      let app = NSRunningApplication(processIdentifier: pid) else { exit(2) }
let element = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(element, 1)
let shortcut: CGEventFlags = [.maskControl, .maskAlternate, .maskShift]

switch args[1] {
case "dump":
    output(nodes(element))
case "status":
    output(["active": app.isActive, "onscreen": onScreen(pid), "finished": app.isFinishedLaunching, "input": hasInput(element)])
case "show":
    if !onScreen(pid) { key(49, flags: shortcut) }
    let ready = waitFor { onScreen(pid) && hasInput(element) }
    output(["ready": ready])
    if !ready { exit(1) }
case "hide":
    key(53, pid: pid)
    Thread.sleep(forTimeInterval: 0.07)
    key(53, pid: pid)
    let hidden = waitFor { !onScreen(pid) }
    output(["hidden": hidden])
    if !hidden { exit(1) }
case "text":
    replace(args[3], pid: pid)
    Thread.sleep(forTimeInterval: 0.3)
    output(nodes(element))
case "queries":
    let count = args.count > 3 ? Int(args[3])! : 10
    let cases = [("apps", "saf", "Safari"), ("apps", "term", "Terminal"),
                 ("arithmetic", "12 * 8", "96"), ("arithmetic", "123 + 456", "579")]
    var rows: [[String: Any]] = []
    for iteration in -2..<count {
        for (group, query, expected) in cases {
            replace("zzzxqvbenchmarknomatch", pid: pid)
            let absent = waitFor { !hasOutput(element, expected: expected) }
            Thread.sleep(forTimeInterval: 0.08)
            key(0, flags: .maskCommand, pid: pid)
            Thread.sleep(forTimeInterval: 0.015)
            let started = ProcessInfo.processInfo.systemUptime
            typeText(query, pid: pid)
            let success = waitFor { hasOutput(element, expected: expected) }
            let elapsed = (ProcessInfo.processInfo.systemUptime - started) * 1000
            rows.append(["group": group, "query": query, "expected": expected,
                         "warmup": iteration < 0, "iteration": iteration,
                         "prior_result_absent": absent, "success": success, "ms": elapsed])
            if !success || !absent {
                output(["error": "Result validation failed", "samples": rows, "tree": nodes(element)])
                exit(1)
            }
        }
    }
    output(["samples": rows])
case "summon":
    let count = args.count > 3 ? Int(args[3])! : 10
    var rows: [[String: Any]] = []
    for iteration in -2..<count {
        key(53, pid: pid)
        Thread.sleep(forTimeInterval: 0.08)
        key(53, pid: pid)
        guard waitFor({ !onScreen(pid) }) else { output(["error": "Could not hide palette"]); exit(1) }
        Thread.sleep(forTimeInterval: 0.12)
        let started = ProcessInfo.processInfo.systemUptime
        key(49, flags: shortcut)
        let ready = waitFor { onScreen(pid) && hasInput(element) }
        rows.append(["warmup": iteration < 0, "iteration": iteration,
                     "success": ready, "ms": (ProcessInfo.processInfo.systemUptime - started) * 1000])
        if !ready { output(["error": "Could not show palette", "samples": rows]); exit(1) }
        Thread.sleep(forTimeInterval: 0.08)
    }
    output(["samples": rows])
default: exit(2)
}
