//! The registry: the tools served are exactly the surface, each in its own file, each described.
use lxagents_gitlab::controllers::{MODULES, ToolSpec, build_registry, get, specs, tools};
use regex::Regex;

use crate::surface::{NO_REPO_ARGUMENT, kind, kind_hints, tool_names};

#[test]
fn the_tools_registered_are_exactly_the_surface_this_server_is_meant_to_serve() {
    let mut names: Vec<String> = tools().iter().map(|t| t.name().to_string()).collect();
    names.sort();
    assert_eq!(names, tool_names());
    assert_eq!(get("repo_create").map(ToolSpec::name), Some("repo_create"));
    assert!(get("nope").is_none());
}

#[test]
fn every_tool_module_is_a_tool_and_every_tool_is_a_module_named_alike() {
    // Everything in the directory but the registry and its prelude is a tool, and its file name is its tool
    // name. Both directions: a stray file fails, and so does a tool in the wrong file.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/controllers");
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|f| f.ends_with(".rs") && f != "mod.rs" && f != "prelude.rs")
        .map(|f| f.trim_end_matches(".rs").to_string())
        .collect();
    files.sort();
    let mut modules: Vec<String> = MODULES.iter().map(|m| m.to_string()).collect();
    modules.sort();
    assert_eq!(modules, files);
    for (module, spec) in MODULES.iter().zip(specs()) {
        assert_eq!(*module, spec.name(), "{module}.rs serves {}", spec.name());
    }
}

#[test]
fn every_tool_carries_a_description_an_output_schema_and_annotations() {
    for tool in tools() {
        let def = &tool.def;
        assert!(
            def.description.len() > 20,
            "{} has no useful description",
            def.name
        );
        assert!(
            def.description.chars().count() <= 600,
            "{} description is too long to sit in context 45 times",
            def.name
        );
        assert!(
            def.output_schema["properties"]
                .as_object()
                .is_some_and(|p| !p.is_empty()),
            "{} has no output schema",
            def.name
        );
        let hints = def.annotations.to_json();
        for hint in [
            "readOnlyHint",
            "destructiveHint",
            "idempotentHint",
            "openWorldHint",
        ] {
            assert!(hints[hint].is_boolean(), "{} lacks {hint}", def.name);
        }
    }
}

#[test]
fn every_tools_annotations_match_the_kind_of_change_it_makes() {
    for tool in tools() {
        let kind = kind(tool.name());
        assert_eq!(
            tool.def.annotations.to_json(),
            kind_hints(kind),
            "{} is a {kind} but is annotated otherwise",
            tool.name()
        );
    }
}

#[test]
fn every_tool_but_the_two_that_cannot_takes_a_required_repo() {
    for tool in tools() {
        let schema = &tool.def.input_schema;
        let has = schema["properties"].get("repo").is_some();
        if NO_REPO_ARGUMENT.contains(&tool.name()) {
            assert!(!has, "{} should not take a repo", tool.name());
        } else {
            assert!(has, "{} takes no repo", tool.name());
            let required = schema["required"].as_array().cloned().unwrap_or_default();
            assert!(
                required.contains(&"repo".into()),
                "{} makes repo optional",
                tool.name()
            );
        }
    }
}

#[test]
fn no_tool_takes_a_credential_as_an_argument() {
    let credential =
        Regex::new(r"(?i)token|secret|password|api_?key|authorization|credential").unwrap();
    for tool in tools() {
        for key in tool.def.input_schema["properties"]
            .as_object()
            .unwrap()
            .keys()
        {
            assert!(!credential.is_match(key), "{}.{key}", tool.name());
        }
    }
}

fn named(name: &'static str) -> ToolSpec {
    let mut spec = specs().remove(0);
    spec.def.name = name;
    spec
}

#[test]
fn the_registry_refuses_two_tools_with_one_name() {
    let problem = build_registry(vec![named("same"), named("same")])
        .err()
        .unwrap();
    assert_eq!(problem, "Duplicate tool name 'same'.");
}

#[test]
fn the_registry_refuses_a_tool_with_no_name() {
    let problem = build_registry(vec![named("")]).err().unwrap();
    assert!(problem.contains("has no 'name'"), "{problem}");
}

#[test]
fn every_tool_in_the_golden_file_is_served_as_it_was_published() {
    // The 0.7.0 `tools/list`, captured from the TypeScript server: names, descriptions, input and output
    // schemas and annotations must not change in the rewrite.
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/tools-list.json")).expect("tools-list.json");
    let golden = golden.as_array().unwrap();
    assert_eq!(golden.len(), tools().len());
    for want in golden {
        let name = want["name"].as_str().unwrap();
        let got = lxagents_gitlab::routes::server::advertise(&get(name).unwrap().def);
        let got = serde_json::to_value(&got).unwrap();
        for key in want.as_object().unwrap().keys() {
            assert_eq!(got[key], want[key], "{name}.{key} differs from 0.7.0");
        }
        for key in got.as_object().unwrap().keys() {
            assert!(
                want.get(key).is_some(),
                "{name} has {key}, which 0.7.0 did not"
            );
        }
    }
}
