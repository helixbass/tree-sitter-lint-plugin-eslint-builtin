use std::{collections::HashMap, sync::Arc};

use id_arena::Id;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, QueryMatchContext, Rule};

use crate::{
    string_utils::upper_case_first, utils::ast_utils, CodePath, CodePathAnalyzer, CodePathOrigin,
};

const DEFAULT_MAX: usize = 20;

#[derive(Deserialize)]
#[serde(default)]
struct OptionsObject {
    #[serde(alias = "maximum")]
    max: usize,
}

impl Default for OptionsObject {
    fn default() -> Self {
        Self { max: DEFAULT_MAX }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    Usize(usize),
    Object(OptionsObject),
}

impl Options {
    pub fn max(&self) -> usize {
        match self {
            Self::Usize(value) => *value,
            Self::Object(OptionsObject { max }) => *max,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::Usize(DEFAULT_MAX)
    }
}

fn report(
    node: Node,
    context: &QueryMatchContext,
    origin: CodePathOrigin,
    complexity: usize,
    threshold: usize,
) {
    let name = match origin {
        CodePathOrigin::ClassFieldInitializer => "class field initializer".to_owned(),
        CodePathOrigin::ClassStaticBlock => "class static block".to_owned(),
        _ => ast_utils::get_function_name_with_kind(node, context),
    };

    context.report(violation! {
        node => node,
        message_id => "complex",
        data => {
            name => upper_case_first(&name),
            complexity => complexity,
            max => threshold,
        }
    });
}

pub fn complexity_rule() -> Arc<dyn Rule> {
    rule! {
        name => "complexity",
        languages => [Javascript],
        messages => [
            complex => "{{name}} has a complexity of {{complexity}}. Maximum allowed is {{max}}.",
        ],
        options_type => Options,
        state => {
            [per-run]
            threshold: usize = options.max(),
            [per-file-run]
            complexities: HashMap<Id<CodePath<'a>>, (usize, Node<'a>, CodePathOrigin)>,
        },
        listeners => [
            r#"
              (catch_clause) @c
              (ternary_expression) @c
              (binary_expression
                operator: [
                  "&&"
                  "||"
                  "??"
                ]
              ) @c
              (for_statement) @c
              (for_in_statement) @c
              (if_statement) @c
              (while_statement) @c
              (do_statement) @c
              (switch_case) @c
              (augmented_assignment_expression
                operator: [
                  "&&="
                  "||="
                  "??="
                ]
              ) @c
            "# => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                let code_path = code_path_analyzer.get_innermost_code_path(node);
                let code_path_instance = &code_path_analyzer.code_path_arena[code_path];

                if !matches!(
                    code_path_instance.origin,
                    CodePathOrigin::Function |
                    CodePathOrigin::ClassFieldInitializer |
                    CodePathOrigin::ClassStaticBlock
                ) {
                    return;
                }

                self.complexities.entry(code_path).or_insert_with(|| {
                    (
                        1,
                        code_path_instance.root_node(&code_path_analyzer.code_path_segment_arena),
                        code_path_instance.origin,
                    )
                }).0 += 1;
            },
            "program:exit" => |node, context| {
                if self.threshold == 0 {
                    let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                    for &code_path in code_path_analyzer.code_paths[1..].into_iter().filter(|code_path| {
                        matches!(
                            code_path_analyzer.code_path_arena[**code_path].origin,
                            CodePathOrigin::Function |
                            CodePathOrigin::ClassFieldInitializer |
                            CodePathOrigin::ClassStaticBlock
                        )
                    }) {
                        let code_path_instance = &code_path_analyzer.code_path_arena[code_path];
                        report(
                            code_path_instance.root_node(&code_path_analyzer.code_path_segment_arena),
                            context,
                            code_path_instance.origin,
                            self.complexities.get(&code_path).map_or(
                                1,
                                |(complexity, _, _)| *complexity
                            ),
                            self.threshold
                        );
                    }

                    return;
                }

                for (complexity, node, origin) in self.complexities.values().filter(|(complexity, _, _)| {
                    *complexity > self.threshold
                }) {
                    report(*node, context, *origin, *complexity, self.threshold);
                }
            }
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::CodePathAnalyzerInstanceProviderFactory;

    use super::*;

    use tree_sitter_lint::{
        rule_tests, RuleTestExpectedError, RuleTestExpectedErrorBuilder, RuleTester,
    };

    fn create_complexity(complexity: usize) -> String {
        let mut func_string = "function test (a) { if (a === 1) {".to_owned();

        for i in 2..complexity {
            func_string.push_str(&format!("}} else if (a === {i}) {{"));
        }

        func_string.push_str("} };");

        func_string
    }

    fn make_error_builder(
        name: impl Into<String>,
        complexity: usize,
        max: usize,
    ) -> RuleTestExpectedErrorBuilder {
        let name = name.into();

        RuleTestExpectedErrorBuilder::default()
            .message_id("complex")
            .data([
                ("name".to_owned(), name),
                ("complexity".to_owned(), complexity.to_string()),
                ("max".to_owned(), max.to_string()),
            ])
            .clone()
    }

    fn make_error(name: impl Into<String>, complexity: usize, max: usize) -> RuleTestExpectedError {
        make_error_builder(name, complexity, max).build().unwrap()
    }

    #[test]
    fn test_complexity_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            complexity_rule(),
            rule_tests! {
                valid => [
                    "function a(x) {}",
                    { code => "function b(x) {}", options => 1 },
                    { code => "function a(x) {if (true) {return x;}}", options => 2 },
                    { code => "function a(x) {if (true) {return x;} else {return x+1;}}", options => 2 },
                    { code => "function a(x) {if (true) {return x;} else if (false) {return x+1;} else {return 4;}}", options => 3 },
                    { code => "function a(x) {for(var i = 0; i < 5; i ++) {x ++;} return x;}", options => 2 },
                    { code => "function a(obj) {for(var i in obj) {obj[i] = 3;}}", options => 2 },
                    { code => "function a(x) {for(var i = 0; i < 5; i ++) {if(i % 2 === 0) {x ++;}} return x;}", options => 3 },
                    { code => "function a(obj) {if(obj){ for(var x in obj) {try {x.getThis();} catch (e) {x.getThat();}}} else {return false;}}", options => 4 },
                    { code => "function a(x) {try {x.getThis();} catch (e) {x.getThat();}}", options => 2 },
                    { code => "function a(x) {return x === 4 ? 3 : 5;}", options => 2 },
                    { code => "function a(x) {return x === 4 ? 3 : (x === 3 ? 2 : 1);}", options => 3 },
                    { code => "function a(x) {return x || 4;}", options => 2 },
                    { code => "function a(x) {x && 4;}", options => 2 },
                    { code => "function a(x) {x ?? 4;}", options => 2 },
                    { code => "function a(x) {x ||= 4;}", options => 2 },
                    { code => "function a(x) {x &&= 4;}", options => 2 },
                    { code => "function a(x) {x ??= 4;}", options => 2 },
                    { code => "function a(x) {x = 4;}", options => 1 },
                    { code => "function a(x) {x |= 4;}", options => 1 },
                    { code => "function a(x) {x &= 4;}", options => 1 },
                    { code => "function a(x) {x += 4;}", options => 1 },
                    { code => "function a(x) {x >>= 4;}", options => 1 },
                    { code => "function a(x) {x >>>= 4;}", options => 1 },
                    { code => "function a(x) {x == 4;}", options => 1 },
                    { code => "function a(x) {x === 4;}", options => 1 },
                    { code => "function a(x) {switch(x){case 1: 1; break; case 2: 2; break; default: 3;}}", options => 3 },
                    { code => "function a(x) {switch(x){case 1: 1; break; case 2: 2; break; default: if(x == 'foo') {5;};}}", options => 4 },
                    { code => "function a(x) {while(true) {'foo';}}", options => 2 },
                    { code => "function a(x) {do {'foo';} while (true)}", options => 2 },
                    { code => "if (foo) { bar(); }", options => 3 },
                    { code => "var a = (x) => {do {'foo';} while (true)}", options => 2, /*parserOptions: { ecmaVersion: 6 }*/ },

                    // class fields
                    { code => "function foo() { class C { x = a || b; y = c || d; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "function foo() { class C { static x = a || b; static y = c || d; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "function foo() { class C { x = a || b; y = c || d; } e || f; }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "function foo() { a || b; class C { x = c || d; y = e || f; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "function foo() { class C { [x || y] = a || b; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { x = a || b; y() { c || d; } z = e || f; }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { x() { a || b; } y = c || d; z() { e || f; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { x = (() => { a || b }) || (() => { c || d }) }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { x = () => { a || b }; y = () => { c || d } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { x = a || (() => { b || c }); }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { x = class { y = a || b; z = c || d; }; }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { x = a || class { y = b || c; z = d || e; }; }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { x; y = a; static z; static q = b; }", options => 1, /*parserOptions: { ecmaVersion: 2022 }*/ },

                    // class static blocks
                    { code => "function foo() { class C { static { a || b; } static { c || d; } } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "function foo() { a || b; class C { static { c || d; } } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "function foo() { class C { static { a || b; } } c || d; }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "function foo() { class C { static { a || b; } } class D { static { c || d; } } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a || b; } static { c || d; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a || b; } static { c || d; } static { e || f; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { () => a || b; c || d; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a || b; () => c || d; } static { c || d; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a } }", options => 1, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a } static { b } }", options => 1, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a || b; } } class D { static { c || d; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a || b; } static c = d || e; }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static a = b || c; static { c || d; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a || b; } c = d || e; }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { a = b || c; static { d || e; } }", options => 2, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { a || b; c || d; } }", options => 3, /*parserOptions: { ecmaVersion: 2022 }*/ },
                    { code => "class C { static { if (a || b) c = d || e; } }", options => 4, /*parserOptions: { ecmaVersion: 2022 }*/ },

                    // object property options
                    { code => "function b(x) {}", options => { max => 1 } }
                ],
                invalid => [
                    { code => "function a(x) {}", options => 0, errors => [make_error("Function 'a'", 1, 0)] },
                    { code => "var func = function () {}", options => 0, errors => [make_error("Function", 1, 0)] },
                    { code => "var obj = { a(x) {} }", options => 0, /*parserOptions: { ecmaVersion: 6 }*/ errors => [make_error("Method 'a'", 1, 0)] },
                    { code => "class Test { a(x) {} }", options => 0, /*parserOptions: { ecmaVersion: 6 }*/ errors => [make_error("Method 'a'", 1, 0)] },
                    { code => "var a = (x) => {if (true) {return x;}}", options => 1, /*parserOptions: { ecmaVersion: 6 }*/ errors => 1 },
                    { code => "function a(x) {if (true) {return x;}}", options => 1, errors => 1 },
                    { code => "function a(x) {if (true) {return x;} else {return x+1;}}", options => 1, errors => 1 },
                    { code => "function a(x) {if (true) {return x;} else if (false) {return x+1;} else {return 4;}}", options => 2, errors => 1 },
                    { code => "function a(x) {for(var i = 0; i < 5; i ++) {x ++;} return x;}", options => 1, errors => 1 },
                    { code => "function a(obj) {for(var i in obj) {obj[i] = 3;}}", options => 1, errors => 1 },
                    { code => "function a(obj) {for(var i of obj) {obj[i] = 3;}}", options => 1, /*parserOptions: { ecmaVersion: 6 }*/ errors => 1 },
                    { code => "function a(x) {for(var i = 0; i < 5; i ++) {if(i % 2 === 0) {x ++;}} return x;}", options => 2, errors => 1 },
                    { code => "function a(obj) {if(obj){ for(var x in obj) {try {x.getThis();} catch (e) {x.getThat();}}} else {return false;}}", options => 3, errors => 1 },
                    { code => "function a(x) {try {x.getThis();} catch (e) {x.getThat();}}", options => 1, errors => 1 },
                    { code => "function a(x) {return x === 4 ? 3 : 5;}", options => 1, errors => 1 },
                    { code => "function a(x) {return x === 4 ? 3 : (x === 3 ? 2 : 1);}", options => 2, errors => 1 },
                    { code => "function a(x) {return x || 4;}", options => 1, errors => 1 },
                    { code => "function a(x) {x && 4;}", options => 1, errors => 1 },
                    { code => "function a(x) {x ?? 4;}", options => 1, errors => 1 },
                    { code => "function a(x) {x ||= 4;}", options => 1, errors => 1 },
                    { code => "function a(x) {x &&= 4;}", options => 1, errors => 1 },
                    { code => "function a(x) {x ??= 4;}", options => 1, errors => 1 },
                    { code => "function a(x) {switch(x){case 1: 1; break; case 2: 2; break; default: 3;}}", options => 2, errors => 1 },
                    { code => "function a(x) {switch(x){case 1: 1; break; case 2: 2; break; default: if(x == 'foo') {5;};}}", options => 3, errors => 1 },
                    { code => "function a(x) {while(true) {'foo';}}", options => 1, errors => 1 },
                    { code => "function a(x) {do {'foo';} while (true)}", options => 1, errors => 1 },
                    { code => "function a(x) {(function() {while(true){'foo';}})(); (function() {while(true){'bar';}})();}", options => 1, errors => 2 },
                    { code => "function a(x) {(function() {while(true){'foo';}})(); (function() {'bar';})();}", options => 1, errors => 1 },
                    { code => "var obj = { a(x) { return x ? 0 : 1; } };", options => 1, /*parserOptions: { ecmaVersion: 6 }*/ errors => [make_error("Method 'a'", 2, 1)] },
                    { code => "var obj = { a: function b(x) { return x ? 0 : 1; } };", options => 1, errors => [make_error("Method 'a'", 2, 1)] },
                    {
                        code => create_complexity(21),
                        errors => [make_error("Function 'test'", 21, 20)]
                    },
                    {
                        code => create_complexity(21),
                        options => {},
                        errors => [make_error("Function 'test'", 21, 20)]
                    },

                    // class fields
                    {
                        code => "function foo () { a || b; class C { x; } c || d; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { a || b; class C { x = c; } d || e; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { a || b; class C { [x || y]; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { a || b; class C { [x || y] = c; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { class C { [x || y]; } a || b; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { class C { [x || y] = a; } b || c; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { class C { [x || y]; [z || q]; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { class C { [x || y] = a; [z || q] = b; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { a || b; class C { x = c || d; } e || f; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "class C { x(){ a || b; } y = c || d || e; z() { f || g; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class field initializer", 3, 2)]
                    },
                    {
                        code => "class C { x = a || b; y() { c || d || e; } z = f || g; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Method 'y'", 3, 2)]
                    },
                    {
                        code => "class C { x; y() { c || d || e; } z; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Method 'y'", 3, 2)]
                    },
                    {
                        code => "class C { x = a || b; }",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class field initializer", 2, 1)]
                    },
                    {
                        code => "(class { x = a || b; })",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class field initializer", 2, 1)]
                    },
                    {
                        code => "class C { static x = a || b; }",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class field initializer", 2, 1)]
                    },
                    {
                        code => "(class { x = a ? b : c; })",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class field initializer", 2, 1)]
                    },
                    {
                        code => "class C { x = a || b || c; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class field initializer", 3, 2)]
                    },
                    {
                        code => "class C { x = a || b; y = b || c || d; z = e || f; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class field initializer", 3, 2)
                                .line(1_usize)
                                .column(27_usize)
                                .end_line(1_usize)
                                .end_column(38_usize)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "class C { x = a || b || c; y = d || e; z = f || g || h; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class field initializer", 3, 2)
                                .line(1_usize)
                                .column(15_usize)
                                .end_line(1_usize)
                                .end_column(26_usize)
                                .build()
                                .unwrap(),
                            make_error_builder("Class field initializer", 3, 2)
                                .line(1_usize)
                                .column(44_usize)
                                .end_line(1_usize)
                                .end_column(55_usize)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "class C { x = () => a || b || c; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Method 'x'", 3, 2)]
                    },
                    {
                        code => "class C { x = (() => a || b || c) || d; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Arrow function", 3, 2)]
                    },
                    {
                        code => "class C { x = () => a || b || c; y = d || e; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Method 'x'", 3, 2)]
                    },
                    {
                        code => "class C { x = () => a || b || c; y = d || e || f; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error("Method 'x'", 3, 2),
                            make_error_builder("Class field initializer", 3, 2)
                                .line(1_usize)
                                .column(38_usize)
                                .end_line(1_usize)
                                .end_column(49_usize)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "class C { x = function () { a || b }; y = function () { c || d }; }",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error("Method 'x'", 2, 1),
                            make_error("Method 'y'", 2, 1)
                        ]
                    },
                    {
                        code => "class C { x = class { [y || z]; }; }",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class field initializer", 2, 1)
                                .line(1_usize)
                                .column(15_usize)
                                .end_line(1_usize)
                                .end_column(34_usize)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "class C { x = class { [y || z] = a; }; }",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class field initializer", 2, 1)
                                .line(1_usize)
                                .column(15_usize)
                                .end_line(1_usize)
                                .end_column(38_usize)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "class C { x = class { y = a || b; }; }",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class field initializer", 2, 1)
                                .line(1_usize)
                                .column(27_usize)
                                .end_line(1_usize)
                                .end_column(33_usize)
                                .build()
                                .unwrap()
                        ]
                    },

                    // class static blocks
                    {
                        code => "function foo () { a || b; class C { static {} } c || d; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "function foo () { a || b; class C { static { c || d; } } e || f; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Function 'foo'", 3, 2)]
                    },
                    {
                        code => "class C { static { a || b; }  }",
                        options => 1,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 2, 1)]
                    },
                    {
                        code => "class C { static { a || b || c; }  }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 3, 2)]
                    },
                    {
                        code => "class C { static { a || b; c || d; }  }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 3, 2)]
                    },
                    {
                        code => "class C { static { a || b; c || d; e || f; }  }",
                        options => 3,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 4, 3)]
                    },
                    {
                        code => "class C { static { a || b; c || d; { e || f; } }  }",
                        options => 3,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 4, 3)]
                    },
                    {
                        code => "class C { static { if (a || b) c = d || e; } }",
                        options => 3,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 4, 3)]
                    },
                    {
                        code => "class C { static { if (a || b) c = (d => e || f)() || (g => h || i)(); } }",
                        options => 3,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 4, 3)]
                    },
                    {
                        code => "class C { x(){ a || b; } static { c || d || e; } z() { f || g; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 3, 2)]
                    },
                    {
                        code => "class C { x = a || b; static { c || d || e; } y = f || g; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 3, 2)]
                    },
                    {
                        code => "class C { static x = a || b; static { c || d || e; } static y = f || g; }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Class static block", 3, 2)]
                    },
                    {
                        code => "class C { static { a || b; } static(){ c || d || e; } static { f || g; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Method 'static'", 3, 2)]
                    },
                    {
                        code => "class C { static { a || b; } static static(){ c || d || e; } static { f || g; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [make_error("Static method 'static'", 3, 2)]
                    },
                    {
                        code => "class C { static { a || b; } static x = c || d || e; static { f || g; } }",
                        options => 2,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class field initializer", 3, 2)
                                .column(41_usize)
                                .end_column(52_usize)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "class C { static { a || b || c || d; } static { e || f || g; } }",
                        options => 3,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class static block", 4, 3)
                                .column(11_usize)
                                .end_column(39_usize)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "class C { static { a || b || c; } static { d || e || f || g; } }",
                        options => 3,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class static block", 4, 3)
                            .column(35_usize)
                            .end_column(63_usize)
                            .build()
                            .unwrap()
                        ]
                    },
                    {
                        code => "class C { static { a || b || c || d; } static { e || f || g || h; } }",
                        options => 3,
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            make_error_builder("Class static block", 4, 3)
                                .column(11_usize)
                                .end_column(39_usize)
                                .build()
                                .unwrap(),
                            make_error_builder("Class static block", 4, 3)
                                .column(40_usize)
                                .end_column(68_usize)
                                .build()
                                .unwrap(),
                        ]
                    },

                    // object property options
                    { code => "function a(x) {}", options => { max => 0 }, errors => [make_error("Function 'a'", 1, 0)] }
                ]
            },
            Box::new(CodePathAnalyzerInstanceProviderFactory),
        )
    }
}
