fn extractor_info_argument(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("--extractor-info requires exactly one URL".to_owned());
    }
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    let extractor = registry
        .find(&args[0])
        .ok_or_else(|| format!("no extractor matched URL: {}", args[0]))?;
    let descriptor = extractor.descriptor();
    let output = serde_json::json!({
        "key": descriptor.key,
        "name": descriptor.name,
        "working": descriptor.working,
        "source_module": descriptor.source_module,
        "source_class": descriptor.source_class,
        "pattern_count": extractor.pattern_count(),
        "native_matcher_count": extractor.native_matcher_count(),
        "matcher_error_count": extractor.matcher_error_count(),
        "native": extractor.is_native(),
        "implementation": if extractor.is_native() { "native" } else { "TODO" },
    });
    serde_json::to_writer(io::stdout(), &output).map_err(|error| error.to_string())
}

fn print_migration_status() -> Result<(), String> {
    println!("yt-dlp-rs {MIGRATION_VERSION}");
    println!("active backend: Rust-only");
    println!("Rust capabilities:");
    for capability in INITIAL_CAPABILITIES {
        println!("  {}: {:?}", capability.name, capability.mode);
    }
    let cli_manifest = cli_inventory()?;
    let records =
        serde_json::from_str::<Vec<CliOptionRecord>>(include_str!("../../data/options.json"))
            .map_err(|error| format!("invalid generated CLI manifest: {error}"))?;
    let supported = rust_supported_option_aliases();
    let native_definitions = records
        .iter()
        .filter(|record| {
            record
                .aliases
                .iter()
                .any(|alias| supported.contains(&alias.as_str()))
        })
        .count();
    let native_spellings = records
        .iter()
        .flat_map(|record| record.aliases.iter())
        .filter(|alias| supported.contains(&alias.as_str()))
        .count();
    println!(
        "CLI inventory: {} definitions, {} spellings, {} groups",
        cli_manifest["count"], cli_manifest["spelling_count"], cli_manifest["group_count"],
    );
    println!(
        "CLI parser coverage: {} definitions, {} aliases; remaining options are TODO",
        native_definitions, native_spellings,
    );
    println!("JavaScript runtimes:");
    for kind in [
        RuntimeKind::Deno,
        RuntimeKind::Node,
        RuntimeKind::QuickJs,
        RuntimeKind::Bun,
    ] {
        match JavascriptRuntime::probe(kind, None) {
            Ok(Some(runtime)) => println!(
                "  {} {} at {} ({})",
                runtime.info().name,
                runtime.info().version,
                runtime.info().path.display(),
                if runtime.info().supported {
                    "supported"
                } else {
                    "unsupported version"
                }
            ),
            Ok(None) => println!("  {}: unavailable", kind.name()),
            Err(error) => println!("  {}: {error}", kind.name()),
        }
    }
    let registry = ExtractorRegistry::generated().map_err(|error| error.to_string())?;
    println!(
        "extractor inventory: {} entries, {} native-matchable, {} pattern errors",
        registry.len(),
        registry.native_matchable_count(),
        registry.pattern_error_count(),
    );
    for extractor in registry
        .iter()
        .filter(|extractor| extractor.matcher_error_count() > 0)
    {
        println!(
            "  pattern TODO: {} ({})",
            extractor.descriptor().key,
            extractor.matcher_errors().join("; "),
        );
    }
    println!(
        "extractor implementations: {} native, remaining extractors are TODO",
        registry.native_implementation_count(),
    );
    Ok(())
}
