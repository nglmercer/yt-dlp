#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_runtime_specific_invocations() {
        let info = RuntimeInfo {
            kind: RuntimeKind::Deno,
            name: "deno".to_owned(),
            path: PathBuf::from("deno"),
            version: "2.3.0".to_owned(),
            version_tuple: vec![2, 3, 0],
            supported: true,
        };
        let runtime = JavascriptRuntime::from_info(info).unwrap();
        let invocation = runtime.invocation(&RuntimeOptions::default());
        assert_eq!(invocation.args.last().map(String::as_str), Some("-"));
        assert!(invocation.args.iter().any(|arg| arg == "--no-remote"));
    }

    #[test]
    fn ejs_wrapper_is_data_driven_and_json_round_trips() {
        let script = build_ejs_script(
            "const lib = {};",
            "function jsc(value) { return value; }",
            &serde_json::json!({"type": "test"}),
        );
        assert!(script.contains("Object.assign(globalThis, lib);"));
        assert!(script.contains(r#"jsc({"type":"test"})"#));
        assert_eq!(
            parse_json_output("noise\n{\"ok\":true}\n").unwrap()["ok"],
            true
        );
    }

    #[test]
    fn probes_and_executes_node_when_available() {
        let Some(runtime) = JavascriptRuntime::probe(RuntimeKind::Node, None).unwrap() else {
            return;
        };
        if !runtime.info().supported {
            return;
        }
        let output = runtime
            .execute("console.log(2 + 2)", &RuntimeOptions::default())
            .unwrap();
        assert_eq!(output.stdout.trim(), "4");
    }
}
