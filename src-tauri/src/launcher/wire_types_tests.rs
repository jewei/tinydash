//! Test-only schema generation. Serde remains the runtime wire implementation.
use super::{result::*, *};
use crate::settings;
use std::{collections::BTreeMap, path::Path};
use syn::{GenericArgument, PathArguments, Type, visit::Visit};
use ts_rs::TS;

fn declaration<T: TS + 'static>(types: &mut BTreeMap<String, String>, config: &ts_rs::Config) {
    assert!(types.insert(T::ident(config), T::decl(config)).is_none());
}

// Parse real signatures, not sample values. Unknown syntax/types fail closed.
fn wire_type(ty: &Type, types: &BTreeMap<String, String>) -> String {
    if let Type::Tuple(tuple) = ty {
        assert!(
            tuple.elems.is_empty(),
            "Non-unit tuple needs contract support"
        );
        return "void".into();
    }
    let Type::Path(path) = ty else {
        panic!("Unsupported command type")
    };
    let segment = path.path.segments.last().unwrap();
    let name = segment.ident.to_string();
    let arguments = match &segment.arguments {
        PathArguments::None => vec![],
        PathArguments::AngleBracketed(args) => args
            .args
            .iter()
            .map(|arg| {
                let GenericArgument::Type(ty) = arg else {
                    panic!("Unsupported generic argument")
                };
                ty
            })
            .collect(),
        _ => panic!("Unsupported command type arguments"),
    };
    match name.as_str() {
        "Result" => {
            assert_eq!(arguments.len(), 2);
            assert_eq!(
                wire_type(arguments[1], types),
                "string",
                "Review changed command error contract"
            );
            wire_type(arguments[0], types)
        }
        "Option" | "Vec" => {
            assert_eq!(arguments.len(), 1);
            let inner = wire_type(arguments[0], types);
            if name == "Option" {
                format!("{inner} | null")
            } else {
                format!("Array<{inner}>")
            }
        }
        "BTreeMap" => {
            assert_eq!(arguments.len(), 2);
            assert_eq!(wire_type(arguments[0], types), "string");
            format!("Record<string, {}>", wire_type(arguments[1], types))
        }
        _ => {
            assert!(arguments.is_empty());
            match name.as_str() {
                "String" => "string".into(),
                "bool" => "boolean".into(),
                "u32" | "u64" | "i64" | "usize" => "number".into(),
                _ => {
                    assert!(
                        types.contains_key(&name),
                        "Add ts-rs declaration for {name}"
                    );
                    name
                }
            }
        }
    }
}

#[derive(Default)]
struct Registrations(Vec<String>);
impl<'ast> Visit<'ast> for Registrations {
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if node.path.segments.last().unwrap().ident == "generate_handler" {
            use syn::parse::Parser;
            let paths = syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated
                .parse2(node.tokens.clone())
                .unwrap();
            self.0.extend(
                paths
                    .iter()
                    .map(|p| p.segments.last().unwrap().ident.to_string()),
            );
        }
        syn::visit::visit_macro(self, node);
    }
}

// ts-rs treats conditional skips conservatively for bidirectional types. These
// declarations describe serialized responses, so Option::is_none must have a
// test-only ts(optional). Check both directions and reject unsupported overrides.
fn check_attributes(attrs: &[syn::Attribute], field: bool) {
    let mut skipped_none = false;
    let mut optional = false;
    for attr in attrs {
        if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip_serializing_if") && field {
                    let predicate: syn::LitStr = meta.value()?.parse()?;
                    assert_eq!(
                        predicate.value(),
                        "Option::is_none",
                        "Review serialization predicate"
                    );
                    skipped_none = true;
                } else if [
                    "rename",
                    "rename_all",
                    "rename_all_fields",
                    "tag",
                    "content",
                ]
                .iter()
                .any(|key| meta.path.is_ident(key))
                {
                    let _: syn::LitStr = meta.value()?.parse()?;
                } else if field && meta.path.is_ident("flatten") {
                    // ts-rs expands the same named struct fields as serde.
                } else if !field
                    && (meta.path.is_ident("default") || meta.path.is_ident("deny_unknown_fields"))
                {
                    // Deserialization-only rules do not make serialized keys optional.
                } else {
                    panic!("Unsupported serde rule in wire declaration");
                }
                Ok(())
            })
            .unwrap();
        }
        if attr.path().is_ident("ts") {
            panic!("Wire overrides must be test-only and checked");
        }
        if attr.path().is_ident("cfg_attr") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("test") {
                    return Ok(());
                }
                if meta.path.is_ident("ts") {
                    meta.parse_nested_meta(|meta| {
                        assert!(
                            field && meta.path.is_ident("optional"),
                            "Unsupported ts override"
                        );
                        optional = true;
                        Ok(())
                    })?;
                } else if meta.path.is_ident("derive") {
                    let content;
                    syn::parenthesized!(content in meta.input);
                    let _: syn::Path = content.parse()?;
                } else {
                    panic!("Unsupported conditional wire attribute");
                }
                Ok(())
            })
            .unwrap();
        }
    }
    assert_eq!(
        skipped_none, optional,
        "ts(optional) must exactly match serde's Option::is_none skip"
    );
}

fn check_shape(item: &syn::Item, types: &BTreeMap<String, String>) {
    let (attrs, fields): (_, Vec<_>) = match item {
        syn::Item::Struct(item) if types.contains_key(&item.ident.to_string()) => {
            (&item.attrs, item.fields.iter().collect())
        }
        syn::Item::Enum(item) if types.contains_key(&item.ident.to_string()) => {
            for variant in &item.variants {
                check_attributes(&variant.attrs, false);
            }
            (
                &item.attrs,
                item.variants.iter().flat_map(|v| &v.fields).collect(),
            )
        }
        _ => return,
    };
    check_attributes(attrs, false);
    for field in fields {
        check_attributes(&field.attrs, true);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CommandSignature {
    args: Vec<(String, String)>,
    result: String,
}

impl CommandSignature {
    fn declaration(&self) -> String {
        let args = self
            .args
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{{ args: [{args}]; result: {} }}", self.result)
    }
}

fn commands(
    dir: &Path,
    types: &BTreeMap<String, String>,
    output: &mut BTreeMap<String, CommandSignature>,
) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            commands(&path, types, output);
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = syn::parse_file(&std::fs::read_to_string(path).unwrap()).unwrap();
        for item in source.items {
            check_shape(&item, types);
            let syn::Item::Fn(function) = item else {
                continue;
            };
            let Some(attr) = function.attrs.iter().find(|attr| {
                let parts: Vec<_> = attr
                    .path()
                    .segments
                    .iter()
                    .map(|p| p.ident.to_string())
                    .collect();
                parts == ["tauri", "command"]
            }) else {
                continue;
            };
            assert!(
                matches!(attr.meta, syn::Meta::Path(_)),
                "Review command options"
            );
            let generics = &function.sig.generics;
            assert!(generics.where_clause.is_none(), "Review command bounds");
            let runtime_generic = !generics.params.is_empty();
            if runtime_generic {
                assert_eq!(generics.params.len(), 1, "Review command generics");
                let syn::GenericParam::Type(parameter) = &generics.params[0] else {
                    panic!("Only the injected Tauri runtime may be generic")
                };
                assert!(parameter.ident == "R" && parameter.default.is_none());
                assert!(parameter.attrs.is_empty());
                assert_eq!(parameter.bounds.len(), 1);
                assert!(
                    matches!(&parameter.bounds[0], syn::TypeParamBound::Trait(bound)
                    if bound.path.is_ident("Runtime") && bound.lifetimes.is_none()
                        && matches!(bound.modifier, syn::TraitBoundModifier::None))
                );
            }
            let mut args = vec![];
            for arg in &function.sig.inputs {
                let syn::FnArg::Typed(arg) = arg else {
                    panic!("Unexpected receiver")
                };
                if let Type::Path(path) = &*arg.ty
                    && path.qself.is_none()
                    && (path.path.segments.len() == 1
                        || (path.path.segments.len() == 2
                            && path.path.segments[0].ident == "tauri"))
                    && let segment = path.path.segments.last().unwrap()
                    && (segment.ident == "AppHandle"
                        || segment.ident == "WebviewWindow"
                        || segment.ident == "State")
                {
                    if segment.ident == "State" {
                        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                            panic!("Review injected state type");
                        };
                        assert_eq!(arguments.args.len(), 2);
                        assert!(matches!(&arguments.args[0], GenericArgument::Lifetime(_)));
                        assert!(matches!(
                            &arguments.args[1],
                            GenericArgument::Type(Type::Path(_))
                        ));
                        continue;
                    }
                    match &segment.arguments {
                        PathArguments::None => {}
                        PathArguments::AngleBracketed(arguments) => {
                            assert!(runtime_generic && arguments.args.len() == 1);
                            assert!(
                                matches!(&arguments.args[0], GenericArgument::Type(Type::Path(path))
                                if path.path.is_ident("R"))
                            );
                        }
                        _ => panic!("Review injected handle type"),
                    }
                    continue;
                }
                let syn::Pat::Ident(id) = &*arg.pat else {
                    panic!("Unsupported parameter pattern")
                };
                args.push((id.ident.to_string(), wire_type(&arg.ty, types)));
            }
            let result = match &function.sig.output {
                syn::ReturnType::Default => "void".into(),
                syn::ReturnType::Type(_, ty) => wire_type(ty, types),
            };
            let name = function.sig.ident.to_string();
            assert!(
                output
                    .insert(name, CommandSignature { args, result })
                    .is_none()
            );
        }
    }
}

#[test]
fn command_signature_parser_tracks_success_and_argument_domains() {
    let dir = tempfile::tempdir().unwrap();
    let source = "#[tauri::command] pub fn hide_launcher(app: AppHandle) -> Result<(), String> {}";
    let mut baseline = BTreeMap::new();
    std::fs::write(dir.path().join("commands.rs"), source).unwrap();
    commands(dir.path(), &BTreeMap::new(), &mut baseline);
    assert_eq!(
        baseline["hide_launcher"].declaration(),
        "{ args: []; result: void }"
    );
    for changed in [
        source.replace("Result<(), String>", "Result<u32, String>"),
        source.replace("app: AppHandle", "app: AppHandle, value: bool"),
        source.replace("app: AppHandle", "app: AppHandle, value: Option<bool>"),
    ] {
        std::fs::write(dir.path().join("commands.rs"), changed).unwrap();
        let mut actual = BTreeMap::new();
        commands(dir.path(), &BTreeMap::new(), &mut actual);
        assert_ne!(
            actual, baseline,
            "Signature drift must change the checked declaration"
        );
    }
    std::fs::write(
        dir.path().join("commands.rs"),
        source.replace("Result<(), String>", "UnknownResponse"),
    )
    .unwrap();
    assert!(
        std::panic::catch_unwind(|| commands(dir.path(), &BTreeMap::new(), &mut BTreeMap::new()))
            .is_err()
    );
}

#[test]
fn ipc_generic_commands_only_ignore_the_injected_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let source = "#[tauri::command] pub fn sync_appearance<R: Runtime>(app: AppHandle<R>, change: bool) -> Result<(), String> {}";
    std::fs::write(dir.path().join("commands.rs"), source).unwrap();
    let mut signatures = BTreeMap::new();
    commands(dir.path(), &BTreeMap::new(), &mut signatures);
    assert_eq!(
        signatures["sync_appearance"].declaration(),
        "{ args: [change: boolean]; result: void }"
    );
    for changed in [
        source.replace("R: Runtime", "R: Clone"),
        source.replace("AppHandle<R>", "AppHandle<String>"),
        source.replace("change: bool", "change: R"),
        source.replace("R: Runtime", "R: Runtime, T"),
    ] {
        std::fs::write(dir.path().join("commands.rs"), changed).unwrap();
        assert!(
            std::panic::catch_unwind(|| commands(
                dir.path(),
                &BTreeMap::new(),
                &mut BTreeMap::new()
            ))
            .is_err()
        );
    }
}

#[test]
fn optional_wire_annotations_fail_closed() {
    let types = BTreeMap::from([("Example".into(), String::new())]);
    let source = r#"struct Example {
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(test, ts(optional))]
        value: Option<String>,
    }"#;
    check_shape(&syn::parse_str(source).unwrap(), &types);
    for changed in [
        source.replace("#[cfg_attr(test, ts(optional))]", ""),
        source.replace("#[serde(skip_serializing_if = \"Option::is_none\")]", ""),
        source.replace("Option::is_none", "custom_predicate"),
        source.replace("ts(optional)", "ts(type = \"string\")"),
        source.replace(
            "value: Option<String>",
            "#[serde(serialize_with = \"custom\")] value: Option<String>",
        ),
    ] {
        let item = syn::parse_str(&changed).unwrap();
        assert!(
            std::panic::catch_unwind(|| check_shape(&item, &types)).is_err(),
            "Unchecked serde/ts drift must fail: {changed}"
        );
    }
}

#[test]
fn generated_ipc_wire_types_match_frontend() {
    // JSON numbers (including SQLite i64 IDs) arrive as JS numbers, not bigint.
    let config = ts_rs::Config::new().with_large_int("number");
    let mut types = BTreeMap::new();
    macro_rules! register { ($($ty:ty),+ $(,)?) => { $(declaration::<$ty>(&mut types, &config);)+ }; }
    register!(
        Action,
        transfer::DragOutcome,
        paste_queue::PasteQueueAction,
        paste_queue::PasteQueueStatus,
        crate::appearance::AppearanceChange,
        ActionConfirmation,
        ResultKind,
        SearchResult,
        SearchResponse,
        ToolDetail,
        query::SearchMode,
        pins::ResultPin,
        warning::LauncherWarning,
        warning::WarningCode,
        files::FileStatus,
        files::FilePhase,
        currency::CurrencyStatus,
        window::LauncherAppearance,
        Settings,
        settings::ClipboardDefaultAction,
        settings::AppPreference,
        settings::ItemPreference,
        settings::CategoryShortcut,
        settings::WebSearch,
        LauncherInfo,
        preferences::SettingsInfo,
        portability::SettingsImport,
        updates::UpdateStatus,
        crate::providers::clipboard::ClipboardEntry,
        clipboard::formats::RichKind,
        clipboard::formats::RichEntry,
        clipboard::formats::RichHistory,
        clipboard::formats::RichPreview,
        file_actions::FilePreview,
        file_actions::PreviewContent,
        file_actions::FileAction,
        library::LibraryKind,
        library::LibraryDraft,
        library::LibraryEntry,
        library::LibraryItem,
        library::LibraryAction,
        utilities::Capabilities,
        utilities::ProcessConfirmation,
        utilities::ColorFormat,
        utilities::MediaAction,
        utilities::WindowAction,
        utilities::awake::Status,
        utilities::color::Color,
        utilities::process::ProcessInfo,
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut signatures = BTreeMap::new();
    commands(&root.join("src"), &types, &mut signatures);
    let mut registered = Registrations::default();
    registered.visit_file(
        &syn::parse_file(&std::fs::read_to_string(root.join("src/lib.rs")).unwrap()).unwrap(),
    );
    registered.0.sort();
    assert_eq!(registered.0, signatures.keys().cloned().collect::<Vec<_>>());
    let mut source =
        "// Generated by generated_ipc_wire_types_match_frontend. Do not edit.\n".to_string();
    for declaration in types.values() {
        source.push_str(&format!("export {declaration}\n"));
    }
    source.push_str("export type Commands = {\n");
    for (name, shape) in &signatures {
        source.push_str(&format!("  {name}: {};\n", shape.declaration()));
    }
    source.push_str("};\n");
    // Runtime wrapper probes consume the same parsed argument metadata, not a
    // second regex parser which could mistake Rust test strings for handlers.
    let arguments: BTreeMap<_, _> = signatures
        .iter()
        .map(|(name, signature)| (name, &signature.args))
        .collect();
    source.push_str(&format!(
        "export const commandArguments = {} as const;\n",
        serde_json::to_string(&arguments).unwrap()
    ));
    let path = root.join("../tests/fixtures/ipc-wire.ts");
    if std::env::var_os("TINYDASH_UPDATE_CONTRACTS").is_some() {
        std::fs::write(&path, &source).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        source,
        "Wire declarations drifted; regenerate with TINYDASH_UPDATE_CONTRACTS=1 and review"
    );
}
