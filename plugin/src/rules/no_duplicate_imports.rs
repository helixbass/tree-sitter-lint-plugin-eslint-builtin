use std::{borrow::Cow, collections::HashMap, sync::Arc};

use itertools::Itertools;
use serde::Deserialize;
use squalid::{CowStrExt, EverythingExt, NonEmpty};
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule,
};

use crate::{
    assert_kind,
    kind::{
        ExportClause, ExportSpecifier, ExportStatement, ImportClause, ImportSpecifier,
        ImportStatement, NamedImports, NamespaceExport, NamespaceImport,
    },
    utils::ast_utils,
};

#[derive(Default, Deserialize)]
struct Options {
    include_exports: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ImportOrExport {
    Import,
    Export,
}

struct ModuleSpec<'a> {
    node: Node<'a>,
    declaration_type: ImportOrExport,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ImportExportType {
    ExportNamespaceSpecifier,
    ExportAll,
    SideEffectImport,
    ImportNamespaceSpecifier,
    ImportSpecifier,
    ExportSpecifier,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum NamedOrNamespace {
    Named,
    Namespace,
}

fn is_import_export_specifier(
    import_export_type: ImportExportType,
    type_: NamedOrNamespace,
) -> bool {
    #[allow(clippy::match_like_matches_macro)]
    match (import_export_type, type_) {
        (
            ImportExportType::ImportSpecifier | ImportExportType::ExportSpecifier,
            NamedOrNamespace::Named,
        ) => true,
        (
            ImportExportType::ImportNamespaceSpecifier | ImportExportType::ExportNamespaceSpecifier,
            NamedOrNamespace::Namespace,
        ) => true,
        _ => false,
    }
}

fn get_specifiers(node: Node) -> Option<Vec<Node>> {
    match node.kind() {
        ImportStatement => node
            .first_non_comment_named_child(SupportedLanguage::Javascript)
            .when(|child| child.kind() == ImportClause)
            .and_then(|import_clause| {
                import_clause.maybe_first_child_of_kind(NamedImports)
                    .map(|named_imports| named_imports.children_of_kind(ImportSpecifier).collect_vec())
                    .or_else(|| {
                        import_clause.first_non_comment_named_child(SupportedLanguage::Javascript)
                            .when(|child| child.kind() == NamespaceImport)
                            .map(|namespace_import| vec![namespace_import])
                    })
            }),
        ExportStatement => node
            .first_non_comment_named_child(SupportedLanguage::Javascript)
            .when(|child| child.kind() == ExportClause)
            .map(|child| child.children_of_kind(ExportSpecifier).collect_vec()),
        _ => Default::default(),
    }
}

fn get_export_all_namespace_export_or_star(node: Node) -> Option<Node> {
    assert_kind!(node, ExportStatement);
    node.non_comment_children(SupportedLanguage::Javascript)
        .nth(1)
        .unwrap()
        .when(|child| matches!(child.kind(), "*" | NamespaceExport,))
}

fn is_export_all_declaration(node: Node) -> bool {
    node.kind() == ExportStatement && get_export_all_namespace_export_or_star(node).is_some()
}

fn specifier_to_import_export_type(node: Node) -> ImportExportType {
    match node.kind() {
        ImportSpecifier => ImportExportType::ImportSpecifier,
        ExportSpecifier => ImportExportType::ExportSpecifier,
        NamespaceImport => ImportExportType::ImportNamespaceSpecifier,
        _ => unreachable!(),
    }
}

fn get_import_export_type(node: Node) -> ImportExportType {
    if let Some(node_specifiers) = get_specifiers(node)
        .non_empty()
    {
        return node_specifiers
            .iter()
            .find(|&specifier| {
                specifier_to_import_export_type(*specifier).thrush(|type_| {
                    is_import_export_specifier(type_, NamedOrNamespace::Named)
                        || is_import_export_specifier(type_, NamedOrNamespace::Namespace)
                })
            })
            .copied()
            .unwrap_or(node_specifiers[0])
            .thrush(specifier_to_import_export_type);
    }
    if is_export_all_declaration(node) {
        return match get_export_all_namespace_export_or_star(node)
            .unwrap()
            .kind()
        {
            NamespaceExport => ImportExportType::ExportNamespaceSpecifier,
            "*" => ImportExportType::ExportAll,
            _ => unreachable!(),
        };
    }
    ImportExportType::SideEffectImport
}

fn is_import_export_can_be_merged(node1: Node, node2: Node) -> bool {
    let import_export_type1 = get_import_export_type(node1);
    let import_export_type2 = get_import_export_type(node2);

    if import_export_type1 == ImportExportType::ExportAll
        && !matches!(
            import_export_type2,
            ImportExportType::ExportAll | ImportExportType::SideEffectImport
        )
        || !matches!(
            import_export_type1,
            ImportExportType::ExportAll | ImportExportType::SideEffectImport
        ) && import_export_type2 == ImportExportType::ExportAll
    {
        return false;
    }
    if is_import_export_specifier(import_export_type1, NamedOrNamespace::Namespace)
        && is_import_export_specifier(import_export_type2, NamedOrNamespace::Named)
        || is_import_export_specifier(import_export_type2, NamedOrNamespace::Namespace)
            && is_import_export_specifier(import_export_type1, NamedOrNamespace::Named)
    {
        return false;
    }
    true
}

fn should_report_import_export(node: Node, previous_nodes: &[Node]) -> bool {
    previous_nodes
        .into_iter()
        .any(|&previous_node| is_import_export_can_be_merged(node, previous_node))
}

fn get_nodes_by_declaration_type<'a, 'b>(
    nodes: &'b [ModuleSpec<'a>],
    type_: ImportOrExport,
) -> impl Iterator<Item = Node<'a>> + 'b {
    nodes.into_iter().filter_map(move |module_spec| {
        (module_spec.declaration_type == type_).then_some(module_spec.node)
    })
}

fn get_module<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> Option<Cow<'a, str>> {
    node.child_by_field_name("source").map(|node_source| {
        ast_utils::get_static_string_value(node_source, context)
            .unwrap()
            .trimmed()
    })
}

pub fn no_duplicate_imports_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-duplicate-imports",
        languages => [Javascript],
        messages => [
            import => "'{{module}}' import is duplicated.",
            import_as => "'{{module}}' import is duplicated as export.",
            export => "'{{module}}' export is duplicated.",
            export_as => "'{{module}}' export is duplicated as import.",
        ],
        options_type => Options,
        state => {
            [per-config]
            include_exports: bool = options.include_exports,

            [per-file-run]
            modules: HashMap<String, Vec<ModuleSpec<'a>>>,
        },
        methods => {
            fn check_and_report(&self, context: &QueryMatchContext, node: Node, declaration_type: ImportOrExport, module: &str) {
                if let Some(previous_nodes) = self.modules.get(module) {
                    let mut message_ids: Vec<&'static str> = Default::default();
                    let import_nodes = get_nodes_by_declaration_type(previous_nodes, ImportOrExport::Import).collect_vec();
                    let export_nodes = self.include_exports.then(|| {
                        get_nodes_by_declaration_type(previous_nodes, ImportOrExport::Export).collect_vec()
                    });
                    match declaration_type {
                        ImportOrExport::Import => {
                            if should_report_import_export(node, &import_nodes) {
                                message_ids.push("import");
                            }
                            #[allow(clippy::collapsible_if)]
                            if self.include_exports {
                                if should_report_import_export(node, export_nodes.as_ref().unwrap()) {
                                    message_ids.push("import_as");
                                }
                            }
                        }
                        ImportOrExport::Export => {
                            if should_report_import_export(node, export_nodes.as_ref().unwrap()) {
                                message_ids.push("export");
                            }
                            #[allow(clippy::collapsible_if)]
                            if self.include_exports {
                                if should_report_import_export(node, &import_nodes) {
                                    message_ids.push("export_as");
                                }
                            }
                        }
                    }
                    message_ids.into_iter().for_each(|message_id| {
                        context.report(violation! {
                            node => node,
                            message_id => message_id,
                            data => {
                                module => module,
                            }
                        });
                    });
                }
            }

            fn handle_imports_exports(&mut self, node: Node<'a>, context: &QueryMatchContext<'a, '_>, declaration_type: ImportOrExport) {
                if let Some(module) = get_module(node, context) {
                    self.check_and_report(
                        context, node, declaration_type, &module,
                    );
                }
            }
        },
        listeners => [
            r#"
              (import_statement) @c
            "# => |node, context| {
                self.handle_imports_exports(node, context, ImportOrExport::Import);
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_no_debugger_rule() {
        RuleTester::run(
            no_duplicate_imports_rule(),
            rule_tests! {
                valid => [
                    "import os from \"os\";\nimport fs from \"fs\";",
                    "import { merge } from \"lodash-es\";",
                    "import _, { merge } from \"lodash-es\";",
                    "import * as Foobar from \"async\";",
                    "import \"foo\"",
                    "import os from \"os\";\nexport { something } from \"os\";",
                    "import * as bar from \"os\";\nimport { baz } from \"os\";",
                    "import foo, * as bar from \"os\";\nimport { baz } from \"os\";",
                    "import foo, { bar } from \"os\";\nimport * as baz from \"os\";",
                    {
                        code => "import os from \"os\";\nexport { hello } from \"hello\";",
                        options => { include_exports => true }
                    },
                    {
                        code => "import os from \"os\";\nexport * from \"hello\";",
                        options => { include_exports => true }
                    },
                    {
                        code => "import os from \"os\";\nexport { hello as hi } from \"hello\";",
                        options => { include_exports => true }
                    },
                    {
                        code => "import os from \"os\";\nexport default function(){};",
                        options => { include_exports => true }
                    },
                    {
                        code => "import { merge } from \"lodash-es\";\nexport { merge as lodashMerge }",
                        options => { include_exports => true }
                    },
                    {
                        code => "export { something } from \"os\";\nexport * as os from \"os\";",
                        options => { include_exports => true }
                    },
                    {
                        code => "import { something } from \"os\";\nexport * as os from \"os\";",
                        options => { include_exports => true }
                    },
                    {
                        code => "import * as os from \"os\";\nexport { something } from \"os\";",
                        options => { include_exports => true }
                    },
                    {
                        code => "import os from \"os\";\nexport * from \"os\";",
                        options => { include_exports => true }
                    },
                    {
                        code => "export { something } from \"os\";\nexport * from \"os\";",
                        options => { include_exports => true }
                    }
                ],
                invalid => [
                    {
                        code => "import \"fs\";\nimport \"fs\"",
                        errors => [{ message_id => "import", data => { module => "fs" }, type => "ImportDeclaration" }]
                    },
                    {
                        code => "import { merge } from \"lodash-es\";\nimport { find } from \"lodash-es\";",
                        errors => [{ message_id => "import", data => { module => "lodash-es" }, type => "ImportDeclaration" }]
                    },
                    {
                        code => "import { merge } from \"lodash-es\";\nimport _ from \"lodash-es\";",
                        errors => [{ message_id => "import", data => { module => "lodash-es" }, type => "ImportDeclaration" }]
                    },
                    {
                        code => "import os from \"os\";\nimport { something } from \"os\";\nimport * as foobar from \"os\";",
                        errors => [
                            { message_id => "import", data => { module => "os" }, type => "ImportDeclaration" },
                            { message_id => "import", data => { module => "os" }, type => "ImportDeclaration" }
                        ]
                    },
                    {
                        code => "import * as modns from \"lodash-es\";\nimport { merge } from \"lodash-es\";\nimport { baz } from \"lodash-es\";",
                        errors => [{ message_id => "import", data => { module => "lodash-es" }, type => "ImportDeclaration" }]
                    },
                    {
                        code => "export { os } from \"os\";\nexport { something } from \"os\";",
                        options => { include_exports => true },
                        errors => [{ message_id => "export", data => { module => "os" }, type => "ExportNamedDeclaration" }]
                    },
                    {
                        code => "import os from \"os\";\nexport { os as foobar } from \"os\";\nexport { something } from \"os\";",
                        options => { include_exports => true },
                        errors => [
                            { message_id => "export_as", data => { module => "os" }, type => "ExportNamedDeclaration" },
                            { message_id => "export", data => { module => "os" }, type => "ExportNamedDeclaration" },
                            { message_id => "export_as", data => { module => "os" }, type => "ExportNamedDeclaration" }
                        ]
                    },
                    {
                        code => "import os from \"os\";\nexport { something } from \"os\";",
                        options => { include_exports => true },
                        errors => [{ message_id => "export_as", data => { module => "os" }, type => "ExportNamedDeclaration" }]
                    },
                    {
                        code => "import os from \"os\";\nexport * as os from \"os\";",
                        options => { include_exports => true },
                        errors => [{ message_id => "export_as", data => { module => "os" }, type => "ExportAllDeclaration" }]
                    },
                    {
                        code => "export * as os from \"os\";\nimport os from \"os\";",
                        options => { include_exports => true },
                        errors => [{ message_id => "import_as", data => { module => "os" }, type => "ImportDeclaration" }]
                    },
                    {
                        code => "import * as modns from \"mod\";\nexport * as  modns from \"mod\";",
                        options => { include_exports => true },
                        errors => [{ message_id => "export_as", data => { module => "mod" }, type => "ExportAllDeclaration" }]
                    },
                    {
                        code => "export * from \"os\";\nexport * from \"os\";",
                        options => { include_exports => true },
                        errors => [{ message_id => "export", data => { module => "os" }, type => "ExportAllDeclaration" }]
                    },
                    {
                        code => "import \"os\";\nexport * from \"os\";",
                        options => { include_exports => true },
                        errors => [{ message_id => "export_as", data => { module => "os" }, type => "ExportAllDeclaration" }]
                    }
                ]
            },
        )
    }
}
