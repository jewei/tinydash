// Functional checks only. The latency benchmark uses the original ui.swift.
import AppKit

let args = CommandLine.arguments
guard args.count >= 2 else { exit(2) }
switch args[1] {
case "category":
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)!
        event.flags = [.maskControl, .maskAlternate, .maskShift]
        event.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.02)
    }
    Thread.sleep(forTimeInterval: 0.10)
case "windows":
    guard args.count == 3, let pid = Int(args[2]) else { exit(2) }
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    let matching = windows.filter { $0[kCGWindowOwnerPID as String] as? Int == pid }
    print(String(data: try! JSONSerialization.data(withJSONObject: matching), encoding: .utf8)!)
default: exit(2)
}
