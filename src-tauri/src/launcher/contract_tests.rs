//! Canonical wire examples are produced by serde, then type-checked by TypeScript.
use super::{Action, ActionConfirmation, ResultKind, SearchResponse, SearchResult, ToolDetail};
use crate::{
    launcher::{
        currency::CurrencyStatus,
        files::{FilePhase, FileStatus},
        pins::ResultPin,
        query::SearchMode,
    },
    settings::{AppPreference, CategoryShortcut, Settings, WebSearch},
};
use serde_json::json;

#[test]
fn serialized_ipc_contracts_match_frontend_fixture() {
    let modes = vec![
        SearchMode::All,
        SearchMode::Apps,
        SearchMode::Files,
        SearchMode::Emoji,
        SearchMode::Calculator,
        SearchMode::Clipboard,
        SearchMode::System,
        SearchMode::Password,
        SearchMode::Timezone,
        SearchMode::Url,
        SearchMode::Web,
    ];
    let actions = vec![
        Action::Launch,
        Action::Open,
        Action::Reveal,
        Action::Copy,
        Action::Delete,
        Action::Run,
        Action::Regenerate,
        Action::Paste,
    ];
    let kinds = [
        ResultKind::App,
        ResultKind::File,
        ResultKind::Folder,
        ResultKind::Calculation,
        ResultKind::Emoji,
        ResultKind::Clipboard,
        ResultKind::SystemCommand,
        ResultKind::Password,
        ResultKind::Timezone,
        ResultKind::CleanedUrl,
        ResultKind::WebSearch,
    ];
    let results = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| SearchResult {
            id: format!("result:{index}"),
            kind,
            title: "Fixture".into(),
            subtitle: "Wire example".into(),
            path: None,
            score: 10,
            icon: None,
            primary_action: Action::Copy,
            secondary_actions: vec![],
            pin: None,
            confirmation: None,
            detail: None,
        })
        .collect();
    let details = vec![
        ToolDetail::Password {
            variant: "password".into(),
            entropy_bits: 128,
            strength: "Strong".into(),
        },
        ToolDetail::Timezone {
            source: "10:00".into(),
            local: "17:00".into(),
            source_zone: "UTC".into(),
            target_zone: None,
            ambiguous: false,
        },
        ToolDetail::Timezone {
            source: "10:00".into(),
            local: "17:00".into(),
            source_zone: "UTC".into(),
            target_zone: Some("Asia/Bangkok".into()),
            ambiguous: true,
        },
        ToolDetail::DateCalculation {
            expression: "tomorrow".into(),
            based_on: "2026-09-28".into(),
            result: "2026-09-29".into(),
        },
        ToolDetail::CleanedUrl {
            original: "https://example.com/?utm_source=test".into(),
            removed: 1,
        },
        ToolDetail::WebSearch {
            engine: "Example".into(),
            query: "hello world".into(),
            url: "https://example.com/?q=hello%20world".into(),
        },
    ];
    let full_result = SearchResult {
        id: "full".into(),
        kind: ResultKind::SystemCommand,
        title: "Restart".into(),
        subtitle: "Confirmation required".into(),
        path: Some("/example/file".into()),
        score: 100,
        icon: Some("data:image/png;base64,example".into()),
        primary_action: Action::Run,
        secondary_actions: actions.clone(),
        pin: Some(ResultPin {
            key: "system:restart".into(),
            categories: modes.clone(),
        }),
        confirmation: Some(ActionConfirmation {
            title: "Restart?".into(),
            description: "Unsaved work can be lost.".into(),
            confirm_label: "Restart".into(),
        }),
        detail: Some(details[0].clone()),
    };
    let settings = Settings {
        category_shortcuts: vec![CategoryShortcut {
            mode: SearchMode::Apps,
            shortcut: "Control+Shift+KeyA".into(),
        }],
        app_preferences: [(
            "app:example".into(),
            AppPreference {
                aliases: vec!["editor".into()],
                hidden: true,
            },
        )]
        .into(),
        web_searches: vec![WebSearch {
            name: "Example".into(),
            keyword: "ex".into(),
            template: "https://example.com/?q={query}".into(),
            enabled: true,
        }],
        file_search_roots: Some(vec!["/example".into()]),
        ..Settings::default()
    };
    // Also exercise deserialization of the same settings sent back by save_settings.
    let settings_json = serde_json::to_value(&settings).unwrap();
    assert_eq!(
        serde_json::from_value::<Settings>(settings_json).unwrap(),
        settings
    );
    let response = SearchResponse {
        preferred_selection_id: None,
        results,
        total: 11,
        indexing: false,
        index_error: None,
        notice: None,
        storage_error: None,
        files: FileStatus {
            total: 3,
            phase: FilePhase::Idle,
            warning: None,
        },
        currency: CurrencyStatus {
            as_of: None,
            refreshing: false,
            warning: None,
        },
    };
    let warning_response = SearchResponse {
        preferred_selection_id: Some("clipboard:1".into()),
        results: vec![],
        total: 0,
        indexing: true,
        index_error: Some("Index unavailable".into()),
        notice: Some("Try a shorter query".into()),
        storage_error: Some(crate::launcher::warning::LauncherWarning::new(
            crate::launcher::warning::WarningCode::StorageUnavailable,
            "Storage unavailable",
            false,
        )),
        files: FileStatus {
            total: 1,
            phase: FilePhase::Scanning,
            warning: Some("Scan incomplete".into()),
        },
        currency: CurrencyStatus {
            as_of: Some("2026-09-28".into()),
            refreshing: true,
            warning: Some("Rates are old".into()),
        },
    };
    let file_statuses: Vec<_> = [
        FilePhase::Disabled,
        FilePhase::Idle,
        FilePhase::Queued,
        FilePhase::Scanning,
        FilePhase::Failed,
    ]
    .into_iter()
    .map(|phase| FileStatus {
        total: 0,
        phase,
        warning: (phase == FilePhase::Failed).then(|| "Cannot start the file scanner".into()),
    })
    .collect();
    let value = json!({
        "fileStatuses": file_statuses,
        "modes": modes, "actions": actions, "response": response, "warningResponse": warning_response,
        "fullResult": full_result, "details": details, "settings": settings,
        "defaults": Settings::default(),
        "launcher": crate::launcher::contract_launcher_info(),
        "settingsInfo": crate::launcher::preferences::contract_settings_info(),
        "clipboard": crate::providers::clipboard::ClipboardEntry { id: 42, content: "Example".into(), created_at: 1, last_used_at: None },
        "usedClipboard": crate::providers::clipboard::ClipboardEntry { id: 42, content: "Used".into(), created_at: 1, last_used_at: Some(2) },
        "imported": crate::launcher::portability::SettingsImport { settings: Settings::default(), ignored_keys: vec!["futureKey".into()], appearance: Some("dark".into()), compact: Some(true), follow_system_glass: None },
        "importedDefaults": crate::launcher::portability::SettingsImport { settings: Settings::default(), ignored_keys: vec![], appearance: None, compact: None, follow_system_glass: Some(true) },
        "update": crate::launcher::updates::UpdateStatus { available: true, version: Some("0.2.0".into()), notes: Some("Release notes".into()), message: "Update available".into() },
        "noUpdate": crate::launcher::updates::UpdateStatus { available: false, version: None, notes: None, message: "Up to date".into() },
    });
    let source = format!(
        "// Generated by serialized_ipc_contracts_match_frontend_fixture. Do not edit.\nimport type {{ ContractFixture }} from \"../ipc-contract\";\n\nexport const contracts = {} satisfies ContractFixture;\n",
        serde_json::to_string_pretty(&value).unwrap()
    );
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/ipc-contract.ts");
    if std::env::var_os("TINYDASH_UPDATE_CONTRACTS").is_some() {
        std::fs::write(&path, &source).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        source,
        "IPC serialization drift. Regenerate with TINYDASH_UPDATE_CONTRACTS=1, review the diff, then typecheck and test."
    );
}
