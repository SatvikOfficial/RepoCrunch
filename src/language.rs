use std::collections::HashMap;
use std::path::Path;

/// Supported programming languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Java,
    Cpp,
    C,
    Json,
    Yaml,
    Toml,
    Markdown,
    Html,
    Css,
    Shell,
    Dockerfile,
    Makefile,
    Other,
}

impl Language {
    pub fn name(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Python => "python",
            Language::Go => "go",
            Language::Java => "java",
            Language::Cpp => "cpp",
            Language::C => "c",
            Language::Json => "json",
            Language::Yaml => "yaml",
            Language::Toml => "toml",
            Language::Markdown => "markdown",
            Language::Html => "html",
            Language::Css => "css",
            Language::Shell => "shell",
            Language::Dockerfile => "dockerfile",
            Language::Makefile => "makefile",
            Language::Other => "other",
        }
    }

    /// Whether this language supports Tree-sitter parsing.
    pub fn has_tree_sitter(&self) -> bool {
        matches!(
            self,
            Language::Rust
                | Language::TypeScript
                | Language::JavaScript
                | Language::Python
                | Language::Go
                | Language::Java
                | Language::Cpp
                | Language::C
        )
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Detect the language of a file from its path, with shebang fallback.
pub fn detect_language(path: &Path, first_line: Option<&str>) -> Language {
    // Check filename first (Dockerfile, Makefile, etc.)
    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
        match filename {
            "Dockerfile" | "Containerfile" => return Language::Dockerfile,
            "Makefile" | "GNUmakefile" | "makefile" => return Language::Makefile,
            "Justfile" | "justfile" => return Language::Makefile,
            _ => {}
        }
    }

    // Check extension
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext.to_lowercase().as_str() {
            "rs" => return Language::Rust,
            "ts" | "tsx" | "mts" | "cts" => return Language::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => return Language::JavaScript,
            "py" | "pyi" | "pyw" => return Language::Python,
            "go" => return Language::Go,
            "java" => return Language::Java,
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" => return Language::Cpp,
            "c" | "h" => return Language::C,
            "json" | "jsonc" | "json5" => return Language::Json,
            "yaml" | "yml" => return Language::Yaml,
            "toml" => return Language::Toml,
            "md" | "mdx" | "markdown" => return Language::Markdown,
            "html" | "htm" => return Language::Html,
            "css" | "scss" | "sass" | "less" => return Language::Css,
            "sh" | "bash" | "zsh" | "fish" => return Language::Shell,
            _ => {}
        }
    }

    // Shebang fallback
    if let Some(line) = first_line {
        if line.starts_with("#!") {
            if line.contains("python") {
                return Language::Python;
            }
            if line.contains("node") || line.contains("deno") || line.contains("bun") {
                return Language::JavaScript;
            }
            if line.contains("bash") || line.contains("sh") || line.contains("zsh") {
                return Language::Shell;
            }
            if line.contains("ruby") {
                return Language::Other;
            }
        }
    }

    Language::Other
}

/// Extracted information from a source file parsed via Tree-sitter.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ParsedFile {
    pub imports: Vec<String>,
    pub symbols: Vec<String>,
}

/// Parse a source file with Tree-sitter to extract imports and symbols.
pub fn parse_file(lang: Language, source: &str) -> ParsedFile {
    if !lang.has_tree_sitter() {
        return ParsedFile::default();
    }

    let ts_language = match get_tree_sitter_language(lang) {
        Some(l) => l,
        None => return ParsedFile::default(),
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_language).is_err() {
        return ParsedFile::default();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return ParsedFile::default(),
    };

    let root = tree.root_node();
    let mut result = ParsedFile::default();

    extract_nodes(lang, root, source.as_bytes(), &mut result);

    result
}

fn get_tree_sitter_language(lang: Language) -> Option<tree_sitter::Language> {
    match lang {
        Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
        Language::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
        Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
        Language::Go => Some(tree_sitter_go::LANGUAGE.into()),
        Language::Java => Some(tree_sitter_java::LANGUAGE.into()),
        Language::Cpp | Language::C => Some(tree_sitter_cpp::LANGUAGE.into()),
        _ => None,
    }
}

/// Walk the AST and extract imports and top-level symbol definitions.
fn extract_nodes(
    lang: Language,
    node: tree_sitter::Node,
    source: &[u8],
    result: &mut ParsedFile,
) {
    match lang {
        Language::Rust => extract_rust(node, source, result),
        Language::TypeScript | Language::JavaScript => extract_js_ts(node, source, result),
        Language::Python => extract_python(node, source, result),
        Language::Go => extract_go(node, source, result),
        Language::Java => extract_java(node, source, result),
        Language::Cpp | Language::C => extract_cpp(node, source, result),
        _ => {}
    }
}

fn node_text<'a>(node: tree_sitter::Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

// ── Rust ──────────────────────────────────────────────

fn extract_rust(node: tree_sitter::Node, source: &[u8], result: &mut ParsedFile) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "use_declaration" => {
                let text = node_text(child, source);
                // Extract the module path from `use foo::bar::baz;`
                let import = text
                    .trim_start_matches("use ")
                    .trim_end_matches(';')
                    .trim();
                result.imports.push(import.to_string());
            }
            "mod_item" => {
                // `mod foo;`
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.imports.push(node_text(name_node, source).to_string());
                }
            }
            "function_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "struct_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "enum_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "trait_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "impl_item" => {
                if let Some(name_node) = child.child_by_field_name("type") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            _ => {}
        }
    }
}

// ── JavaScript / TypeScript ───────────────────────────

fn extract_js_ts(node: tree_sitter::Node, source: &[u8], result: &mut ParsedFile) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                // Extract the source string from import ... from "path"
                if let Some(src) = child.child_by_field_name("source") {
                    let text = node_text(src, source)
                        .trim_matches('"')
                        .trim_matches('\'');
                    result.imports.push(text.to_string());
                }
            }
            "export_statement" => {
                // Extract exported names
                let mut inner = child.walk();
                for sub in child.children(&mut inner) {
                    match sub.kind() {
                        "function_declaration" | "class_declaration" => {
                            if let Some(name_node) = sub.child_by_field_name("name") {
                                result.symbols.push(node_text(name_node, source).to_string());
                            }
                        }
                        "lexical_declaration" => {
                            extract_variable_names(sub, source, result);
                        }
                        _ => {}
                    }
                }
            }
            "function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "class_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                extract_variable_names(child, source, result);
            }
            // Handle dynamic require() at expression level
            "expression_statement" => {
                let mut sub_cursor = child.walk();
                for sub in child.children(&mut sub_cursor) {
                    if sub.kind() == "call_expression" {
                        if let Some(func) = sub.child_by_field_name("function") {
                            if node_text(func, source) == "require" {
                                if let Some(args) = sub.child_by_field_name("arguments") {
                                    let mut ac = args.walk();
                                    for arg in args.children(&mut ac) {
                                        if arg.kind() == "string" {
                                            let text = node_text(arg, source)
                                                .trim_matches('"')
                                                .trim_matches('\'');
                                            result.imports.push(text.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_variable_names(node: tree_sitter::Node, source: &[u8], result: &mut ParsedFile) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name_node) = child.child_by_field_name("name") {
                result.symbols.push(node_text(name_node, source).to_string());
            }
        }
    }
}

// ── Python ────────────────────────────────────────────

fn extract_python(node: tree_sitter::Node, source: &[u8], result: &mut ParsedFile) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                // import foo, bar
                let mut inner = child.walk();
                for sub in child.children(&mut inner) {
                    if sub.kind() == "dotted_name" {
                        result.imports.push(node_text(sub, source).to_string());
                    }
                }
            }
            "import_from_statement" => {
                // from foo import bar
                if let Some(module) = child.child_by_field_name("module_name") {
                    result.imports.push(node_text(module, source).to_string());
                }
                // Fallback: look for dotted_name children
                let mut inner = child.walk();
                let mut found_from = false;
                for sub in child.children(&mut inner) {
                    if sub.kind() == "from" {
                        found_from = true;
                        continue;
                    }
                    if found_from && sub.kind() == "dotted_name" {
                        result.imports.push(node_text(sub, source).to_string());
                        break;
                    }
                }
            }
            "function_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "class_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            _ => {}
        }
    }
}

// ── Go ────────────────────────────────────────────────

fn extract_go(node: tree_sitter::Node, source: &[u8], result: &mut ParsedFile) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_declaration" => {
                // Single or grouped imports
                let mut inner = child.walk();
                for sub in child.children(&mut inner) {
                    if sub.kind() == "import_spec" || sub.kind() == "interpreted_string_literal" {
                        let text = node_text(sub, source)
                            .trim_matches('"');
                        result.imports.push(text.to_string());
                    }
                    if sub.kind() == "import_spec_list" {
                        let mut list_cursor = sub.walk();
                        for spec in sub.children(&mut list_cursor) {
                            if spec.kind() == "import_spec" {
                                if let Some(path_node) = spec.child_by_field_name("path") {
                                    let text = node_text(path_node, source).trim_matches('"');
                                    result.imports.push(text.to_string());
                                }
                            }
                        }
                    }
                }
            }
            "function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "type_declaration" => {
                let mut inner = child.walk();
                for sub in child.children(&mut inner) {
                    if sub.kind() == "type_spec" {
                        if let Some(name_node) = sub.child_by_field_name("name") {
                            result.symbols.push(node_text(name_node, source).to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// ── Java ──────────────────────────────────────────────

fn extract_java(node: tree_sitter::Node, source: &[u8], result: &mut ParsedFile) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_declaration" => {
                let text = node_text(child, source);
                let import = text
                    .trim_start_matches("import ")
                    .trim_start_matches("static ")
                    .trim_end_matches(';')
                    .trim();
                result.imports.push(import.to_string());
            }
            "class_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "interface_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "enum_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            _ => {}
        }
    }
}

// ── C / C++ ───────────────────────────────────────────

fn extract_cpp(node: tree_sitter::Node, source: &[u8], result: &mut ParsedFile) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "preproc_include" => {
                if let Some(path_node) = child.child_by_field_name("path") {
                    let text = node_text(path_node, source)
                        .trim_matches('"')
                        .trim_matches('<')
                        .trim_matches('>');
                    result.imports.push(text.to_string());
                }
            }
            "function_definition" | "declaration" => {
                if let Some(declarator) = child.child_by_field_name("declarator") {
                    // Function declarator → get name
                    if let Some(name_node) = declarator.child_by_field_name("declarator") {
                        result.symbols.push(node_text(name_node, source).to_string());
                    } else {
                        // Direct name
                        let name = node_text(declarator, source);
                        if !name.is_empty() && !name.contains(' ') && name.len() < 100 {
                            result.symbols.push(name.to_string());
                        }
                    }
                }
            }
            "class_specifier" | "struct_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            "namespace_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    result.symbols.push(node_text(name_node, source).to_string());
                }
            }
            _ => {}
        }
    }
}

/// Build the extension → Language mapping table (used by other modules).
pub fn extension_language_map() -> HashMap<&'static str, Language> {
    let mut m = HashMap::new();
    m.insert("rs", Language::Rust);
    m.insert("ts", Language::TypeScript);
    m.insert("tsx", Language::TypeScript);
    m.insert("js", Language::JavaScript);
    m.insert("jsx", Language::JavaScript);
    m.insert("py", Language::Python);
    m.insert("go", Language::Go);
    m.insert("java", Language::Java);
    m.insert("cpp", Language::Cpp);
    m.insert("cxx", Language::Cpp);
    m.insert("cc", Language::Cpp);
    m.insert("hpp", Language::Cpp);
    m.insert("c", Language::C);
    m.insert("h", Language::C);
    m.insert("json", Language::Json);
    m.insert("yaml", Language::Yaml);
    m.insert("yml", Language::Yaml);
    m.insert("toml", Language::Toml);
    m.insert("md", Language::Markdown);
    m.insert("html", Language::Html);
    m.insert("css", Language::Css);
    m.insert("sh", Language::Shell);
    m.insert("bash", Language::Shell);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust() {
        assert_eq!(detect_language(Path::new("main.rs"), None), Language::Rust);
    }

    #[test]
    fn test_detect_typescript() {
        assert_eq!(detect_language(Path::new("app.tsx"), None), Language::TypeScript);
    }

    #[test]
    fn test_detect_dockerfile() {
        assert_eq!(detect_language(Path::new("Dockerfile"), None), Language::Dockerfile);
    }

    #[test]
    fn test_detect_shebang_python() {
        assert_eq!(
            detect_language(Path::new("script"), Some("#!/usr/bin/env python3")),
            Language::Python
        );
    }

    #[test]
    fn test_parse_rust_imports() {
        let source = r#"
use std::collections::HashMap;
use crate::scanner;

fn main() {}

struct Config {}
"#;
        let parsed = parse_file(Language::Rust, source);
        assert!(parsed.imports.iter().any(|i| i.contains("HashMap")));
        assert!(parsed.symbols.contains(&"main".to_string()));
        assert!(parsed.symbols.contains(&"Config".to_string()));
    }

    #[test]
    fn test_parse_js_imports() {
        let source = r#"
import { foo, bar } from "./utils";
import React from "react";

export function MyComponent() {}

const helper = 42;
"#;
        let parsed = parse_file(Language::JavaScript, source);
        assert!(parsed.imports.contains(&"./utils".to_string()));
        assert!(parsed.imports.contains(&"react".to_string()));
        assert!(parsed.symbols.contains(&"MyComponent".to_string()));
    }

    #[test]
    fn test_parse_python_imports() {
        let source = r#"
import os
from pathlib import Path

def main():
    pass

class Config:
    pass
"#;
        let parsed = parse_file(Language::Python, source);
        assert!(parsed.imports.iter().any(|i| i.contains("os")));
        assert!(parsed.symbols.contains(&"main".to_string()));
        assert!(parsed.symbols.contains(&"Config".to_string()));
    }
}
