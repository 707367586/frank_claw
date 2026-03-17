# SwiftUI Coding Rules

## Style
- Follow Swift API Design Guidelines
- Use `camelCase` for variables/functions, `PascalCase` for types
- Prefer `struct` over `class` for data models
- Use `@Observable` (iOS 17+) over `ObservableObject` where possible

## Architecture
- MVVM pattern: View → ViewModel → Service → Rust Core (via FFI)
- Views should be thin — no business logic
- ViewModels handle state and call into services
- Services wrap Rust FFI calls

## Rust-Swift FFI
- Use `swift-bridge` or `uniffi` for Rust ↔ Swift interop
- Keep FFI boundary thin — pass simple types (String, Int, Bool, Data)
- Complex data: serialize to JSON at FFI boundary
- All FFI calls are async from Swift side

## macOS Integration
- Use native macOS APIs: NSUserNotification, FSEvents, NSTouchBar
- Respect system appearance (dark/light mode)
- Support keyboard shortcuts for all major actions
- Menu bar integration for background daemon status

## Localization
- All user-visible strings via `LocalizedStringKey`
- Support English + Simplified Chinese from day one
- No hardcoded strings in Views
