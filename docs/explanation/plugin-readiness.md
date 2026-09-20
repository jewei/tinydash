# Future plugin support

Decision recorded on 20 September 2026: defer the plugin system and public software development kit, or SDK. Keep the current Rust, Tauri 2, and SolidJS architecture. Memory use remains the first performance priority, followed by response time.

This decision sets rules for current development. TinyDash does not provide an end-user plugin loader or SDK. The Tauri integration plugins used by the desktop application are separate from this future feature.

## Decision

Keep search, ranking, built-in command logic, storage, and OS operations in Rust. Keep presentation and immediate interaction state in Solid. Extend these modules when a concrete plugin requires it.

The goal is to reduce the work needed for a future SDK without adding current runtime cost or unused infrastructure. Continue to use direct provider methods and enums for implemented behavior. Do not add a generic provider trait, plugin enum variants, empty registries, runtime dependencies, or separate crates only for possible future use.

No plugin language, package format, public interface version, or delivery date is selected. Raycast compatibility is not a goal. The current Rust types and Tauri commands are internal interfaces, not a public SDK contract.

## Interfaces to preserve during current development

| Responsibility                    | Current code                                                                                                                                         | Rule for changes                                                                                                                                                         |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Search and ranking                | [SearchManager](../../src-tauri/src/launcher/search.rs), [ranking](../../src-tauri/src/ranking/mod.rs)                                               | Search prepared local data. Keep network requests, database access, and external execution outside the search mutex. Preserve the result limit and built-in ordering.    |
| Result data and frontend commands | [Result types](../../src-tauri/src/launcher/result.rs), [query types](../../src-tauri/src/launcher/query.rs), [frontend bridge](../../src/bridge.ts) | Use typed, serializable data. Keep Rust and TypeScript definitions consistent. Route application command calls through the frontend bridge.                              |
| User interaction                  | [Launcher view](../../src/App.tsx)                                                                                                                   | Solid owns focus, selection, navigation, and rendering. Rust owns matching and command behavior. Reuse view components when implemented features need the same behavior. |
| Actions and OS access             | [Action execution](../../src-tauri/src/launcher/actions.rs), [platform module](../../src-tauri/src/platform/mod.rs)                                  | Resolve result IDs in Rust, validate the requested action, and release the search lock before OS work. Keep confirmation checks in Rust.                                 |
| Storage                           | [Storage module](../../src-tauri/src/launcher/storage.rs), [database module](../../src-tauri/src/db/mod.rs)                                          | Keep database connections and persistence rules inside Rust. Expose operations that callers need, rather than database handles or internal table layouts.                |
| Background work                   | [Currency refresh](../../src-tauri/src/launcher/currency.rs), [file worker](../../src-tauri/src/launcher/file_watch.rs)                              | Give work and retained data an owner. Keep slow work outside search. Set bounds for new caches and queues, and define when their contents are released.                  |

Keep command logic independent of Tauri handles and window state where possible. The launcher modules can connect that logic to the application. Do not refactor existing modules only to satisfy a hypothetical SDK interface.

The frontend currently keeps one search request in flight, replaces waiting input, and rejects stale replies. This does not cancel backend work that has already started. Preserve that distinction when adding asynchronous work.

During review, check whether a change couples search to external work, moves command logic into the UI, exposes internal storage, or retains data without a bound. Repair that coupling within the implemented feature. Add an abstraction only when actual callers need it.

## Constraints for a future SDK

These constraints guide a later implementation. They do not require plugin infrastructure now.

- Rust will search plugin command metadata without loading or executing plugin code. Live plugin queries will require command activation or an explicit scope. Searching command names does not search remote plugin data.
- Built-in providers will keep their native Rust path. Plugin execution will have a separate module outside `SearchManager` and its mutex. A minimal helper process for one foreground session is the preferred first experiment.
- Plugins will return bounded view data and named actions. Solid will render that data and own keyboard behavior. Start with the views a real command needs; do not expose arbitrary HTML or privileged frontend JavaScript.
- Add a generic plugin command and view route when the first plugin needs it. Installed plugins must not each require a new built-in result kind or search mode. Keep plugin messages separate from internal launcher types so the public interface can be versioned independently.
- Rust will mediate network, storage, credentials, and OS access through explicit permissions. Validate actions against the live plugin session. A helper process permits independent termination but does not, by itself, enforce those permissions.
- Sessions will own requests and action handles. New queries must reject stale updates and cancel superseded work. Ending a session must stop its outstanding work and release its resources. Bound host buffers and retained UI data as well as runtime memory.
- Hiding the launcher and ending a command are separate lifecycle events. Define which query, selection, navigation state, and form drafts survive each event before selecting a runtime retention policy. Background execution requires a separate decision.
- Keep durable plugin values in host-managed storage with plugin-specific ownership, lazy reads, and limits. Avoid loading all plugin caches into settings or copying them into each session.

Choose one production runtime after measurement. Lua, QuickJS, Node, and Wasm remain candidates where the intended commands justify them. A small interpreter alone does not establish lower application memory or faster commands.

## When to start implementation

Revisit the deferral when a concrete user task requires programmable commands. Start with one useful command and the smallest host interface it needs. Check whether a declarative action can satisfy the task before adding a runtime.

Use that command to compare suitable runtimes with identical data, UI, and lifecycle behavior. Set useful memory and latency targets before measurement. Compare the current application with the prototype, including the Rust host, WebView processes, helper, and retained data. Measure cold activation, repeated use, active memory, and memory after close.

Before publishing an SDK, verify cancellation, action validation, resource limits, and state restoration. Test helper startup and termination on macOS, Windows, and Linux. Follow the [verification procedure](../how-to/verify.md), and record native evidence separately from browser tests with mocked IPC.

Keep prototype plans and measurements in ignored local folders. Update this decision when implementation starts. Until then, current feature work follows the interfaces above without adding a plugin runtime.
