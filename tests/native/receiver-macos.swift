// Disposable native target for drag, app quit, and window-placement checks.
// swiftc tests/native/receiver-macos.swift -o .local/native-receiver
// .local/native-receiver FIXTURE_ROOT EVIDENCE_DIRECTORY
// For an app bundle, set FixtureRoot and FixtureEvidence in Info.plist.
import AppKit
import CryptoKit

func fail(_ message: String) -> Never {
    fputs(message + "\n", stderr)
    exit(1)
}

let arguments = CommandLine.arguments
let rootPath: String
let evidencePath: String
if arguments.count == 3 {
    rootPath = arguments[1]
    evidencePath = arguments[2]
} else if arguments.count == 1,
    let root = Bundle.main.object(forInfoDictionaryKey: "FixtureRoot") as? String,
    let evidence = Bundle.main.object(forInfoDictionaryKey: "FixtureEvidence") as? String {
    rootPath = root
    evidencePath = evidence
} else {
    fail("Usage: native-receiver FIXTURE_ROOT EVIDENCE_DIRECTORY")
}

let fixtureRoot = URL(fileURLWithPath: rootPath).standardizedFileURL.resolvingSymlinksInPath()
let evidence = URL(fileURLWithPath: evidencePath).standardizedFileURL.resolvingSymlinksInPath()
var isDirectory: ObjCBool = false
guard FileManager.default.fileExists(atPath: fixtureRoot.path, isDirectory: &isDirectory),
    isDirectory.boolValue, fixtureRoot.path != "/" else {
    fail("Fixture root must be an existing directory containing only test files.")
}
guard !FileManager.default.fileExists(atPath: evidence.path) else {
    fail("Use a new evidence directory for each receiver process.")
}
try FileManager.default.createDirectory(at: evidence, withIntermediateDirectories: true)

func writeRecord(_ name: String, _ value: [String: Any]) throws {
    try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
        .write(to: evidence.appendingPathComponent(name), options: .atomic)
}

final class Receiver: NSView {
    private var dropCount = 0
    private var offerCount = 0

    private func files(_ sender: NSDraggingInfo) -> [URL] {
        let urls = sender.draggingPasteboard.readObjects(
            forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]
        ) as? [URL] ?? []
        return urls.map { $0.standardizedFileURL.resolvingSymlinksInPath() }
    }

    private func accepts(_ urls: [URL]) -> Bool {
        !urls.isEmpty && urls.count <= 64
            && urls.allSatisfy { $0.path.hasPrefix(fixtureRoot.path + "/") }
    }

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        offerCount += 1
        let urls = files(sender)
        let accepted = accepts(urls)
        try? writeRecord("offer-\(offerCount).json", [
            "accepted": accepted,
            "fileCount": urls.count,
            "sourceOperationMask": sender.draggingSourceOperationMask.rawValue,
        ])
        return accepted ? .copy : []
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        let urls = files(sender)
        guard accepts(urls) else { return false }
        dropCount += 1
        do {
            let destination = evidence.appendingPathComponent("drop-\(dropCount)")
            try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: false)
            var copies: [[String: Any]] = []
            for (index, file) in urls.enumerated() {
                // A separate directory preserves duplicate basenames without overwriting.
                let directory = destination.appendingPathComponent(String(index))
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
                let target = directory.appendingPathComponent(file.lastPathComponent)
                try FileManager.default.copyItem(at: file, to: target)
                var record: [String: Any] = ["source": file.path, "copy": target.path]
                let values = try target.resourceValues(forKeys: [.isRegularFileKey])
                if values.isRegularFile == true {
                    record["sha256"] = SHA256.hash(data: try Data(contentsOf: target))
                        .map { String(format: "%02x", $0) }.joined()
                }
                copies.append(record)
            }
            try writeRecord("drop-\(dropCount).json", [
                "copies": copies,
                "sourceOperationMask": sender.draggingSourceOperationMask.rawValue,
                "pid": ProcessInfo.processInfo.processIdentifier,
            ])
            return true
        } catch {
            try? writeRecord("drop-\(dropCount)-failure.json", ["error": String(describing: error)])
            return false
        }
    }
}

final class Delegate: NSObject, NSApplicationDelegate {
    func applicationWillTerminate(_ notification: Notification) {
        try? writeRecord("quit.json", ["pid": ProcessInfo.processInfo.processIdentifier])
    }
}

let app = NSApplication.shared
let delegate = Delegate()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.finishLaunching()
let window = NSWindow(
    contentRect: NSRect(x: 10, y: 30, width: 260, height: 160),
    styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false
)
window.title = "Orbit verification receiver"
// Keep the small target visible beside an always-on-top launcher.
window.level = NSWindow.Level(rawValue: NSWindow.Level.floating.rawValue + 1)
let view = Receiver(frame: NSRect(x: 0, y: 0, width: 260, height: 160))
view.registerForDraggedTypes([.fileURL, NSPasteboard.PasteboardType("NSFilenamesPboardType")])
view.wantsLayer = true
view.layer?.backgroundColor = NSColor.systemGreen.cgColor
window.contentView = view
window.makeKeyAndOrderFront(nil)
app.activate(ignoringOtherApps: true)
try writeRecord("ready.json", [
    "pid": ProcessInfo.processInfo.processIdentifier,
    "fixtureRoot": fixtureRoot.path,
    "frame": NSStringFromRect(window.frame),
])
print("Ready: \(evidence.path)")
fflush(stdout)
app.run()
