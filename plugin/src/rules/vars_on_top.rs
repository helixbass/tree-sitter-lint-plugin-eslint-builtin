use std::sync::Arc;

use itertools::Either;
use squalid::OptionExt;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule,
};

use crate::kind::{
    self, ClassStaticBlock, ExportStatement, ExpressionStatement, HashBangLine, ImportStatement,
    LexicalDeclaration, Program, StatementBlock, VariableDeclaration,
};

fn looks_like_directive(node: Node) -> bool {
    node.kind() == ExpressionStatement
        && node
            .first_non_comment_named_child(SupportedLanguage::Javascript)
            .kind()
            == kind::String
}

fn looks_like_import(node: Node) -> bool {
    node.kind() == ImportStatement
}

fn is_variable_declaration(node: Node) -> bool {
    [VariableDeclaration, LexicalDeclaration].contains(&node.kind())
        || node.kind() == ExportStatement
            && node
                .child_by_field_name("declaration")
                .matches(|declaration| {
                    [VariableDeclaration, LexicalDeclaration].contains(&declaration.kind())
                })
}

fn is_var_on_top<'a>(node: Node<'a>, statements: impl Iterator<Item = Node<'a>>, is_static_block: bool) -> bool {
    let statements = match is_static_block {
        true => Either::Left(statements),
        _ => Either::Right(statements.skip_while(|&statement| {
            looks_like_directive(statement) || looks_like_import(statement)
        })),
    };

    for statement in statements {
        if !is_variable_declaration(statement) {
            return false;
        }
        if statement == node {
            return true;
        }
    }

    false
}

fn global_var_check(node: Node, parent: Node, context: &QueryMatchContext) {
    if !is_var_on_top(
        node,
        parent
            .non_comment_named_children(SupportedLanguage::Javascript)
            .skip_while(|child| child.kind() == HashBangLine),
        false,
    ) {
        context.report(violation! {
            node => node,
            message_id => "top",
        });
    }
}

fn block_scope_var_check(node: Node, context: &QueryMatchContext) {
    let parent = node.parent().unwrap();

    if parent.kind() == StatementBlock
        && parent.parent().unwrap().kind().contains("function")
        && is_var_on_top(
            node,
            parent.non_comment_named_children(SupportedLanguage::Javascript),
            false,
        )
    {
        return;
    }

    if parent.kind() == StatementBlock
        && parent.parent().unwrap().kind() == ClassStaticBlock
        && is_var_on_top(
            node,
            parent.non_comment_named_children(SupportedLanguage::Javascript),
            true,
        )
    {
        return;
    }

    context.report(violation! {
        node => node,
        message_id => "top",
    });
}

pub fn vars_on_top_rule() -> Arc<dyn Rule> {
    rule! {
        name => "vars-on-top",
        languages => [Javascript],
        messages => [
            top => "All 'var' declarations must be at the top of the function scope.",
        ],
        listeners => [
            r#"
              (variable_declaration) @c
              (for_in_statement
                kind: "var"
                left: (_) @c
              )
            "# => |node, context| {
                let parent = node.parent().unwrap();
                match parent.kind() {
                    ExportStatement => {
                        global_var_check(parent, parent.parent().unwrap(), context);
                    }
                    Program => {
                        global_var_check(node, parent, context);
                    }
                    _ => {
                        block_scope_var_check(node, context);
                    }
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::{VariableDeclaration, Identifier};

    #[test]
    fn test_vars_on_top_rule() {
        let mut error_builder = RuleTestExpectedErrorBuilder::default();
        error_builder.message_id("top").type_(VariableDeclaration);
        let error = error_builder.clone().build().unwrap();
        RuleTester::run(
            vars_on_top_rule(),
            rule_tests! {
                valid => [
                    "var first = 0;
                    function foo() {
                        first = 2;
                    }",
                    "function foo() {
                    }",
                    "function foo() {
                       var first;
                       if (true) {
                           first = true;
                       } else {
                           first = 1;
                       }
                    }",
                    "function foo() {
                       var first;
                       var second = 1;
                       var third;
                       var fourth = 1, fifth, sixth = third;
                       var seventh;
                       if (true) {
                           third = true;
                       }
                       first = second;
                    }",
                    "function foo() {
                       var i;
                       for (i = 0; i < 10; i++) {
                           alert(i);
                       }
                    }",
                    "function foo() {
                       var outer;
                       function inner() {
                           var inner = 1;
                           var outer = inner;
                       }
                       outer = 1;
                    }",
                    "function foo() {
                       var first;
                       //Hello
                       var second = 1;
                       first = second;
                    }",
                    "function foo() {
                       var first;
                       /*
                           Hello Clarice
                       */
                       var second = 1;
                       first = second;
                    }",
                    "function foo() {
                       var first;
                       var second = 1;
                       function bar(){
                           var first;
                           first = 5;
                       }
                       first = second;
                    }",
                    "function foo() {
                       var first;
                       var second = 1;
                       function bar(){
                           var third;
                           third = 5;
                       }
                       first = second;
                    }",
                    "function foo() {
                       var first;
                       var bar = function(){
                           var third;
                           third = 5;
                       }
                       first = 5;
                    }",
                    "function foo() {
                       var first;
                       first.onclick(function(){
                           var third;
                           third = 5;
                       });
                       first = 5;
                    }",
                    {
                        code =>
                            "function foo() {
                               var i = 0;
                               for (let j = 0; j < 10; j++) {
                                   alert(j);
                               }
                               i = i + 1;
                            }",
                        environment => {
                            ecma_version => 6
                        }
                    },
                    "'use strict'; var x; f();",
                    "'use strict'; 'directive'; var x; var y; f();",
                    "function f() { 'use strict'; var x; f(); }",
                    "function f() { 'use strict'; 'directive'; var x; var y; f(); }",
                    { code => "import React from 'react'; var y; function f() { 'use strict'; var x; var y; f(); }", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "'use strict'; import React from 'react'; var y; function f() { 'use strict'; var x; var y; f(); }", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "import React from 'react'; 'use strict'; var y; function f() { 'use strict'; var x; var y; f(); }", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "import * as foo from 'mod.js'; 'use strict'; var y; function f() { 'use strict'; var x; var y; f(); }", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "import { square, diag } from 'lib'; 'use strict'; var y; function f() { 'use strict'; var x; var y; f(); }", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "import { default as foo } from 'lib'; 'use strict'; var y; function f() { 'use strict'; var x; var y; f(); }", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "import 'src/mylib'; 'use strict'; var y; function f() { 'use strict'; var x; var y; f(); }", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "import theDefault, { named1, named2 } from 'src/mylib'; 'use strict'; var y; function f() { 'use strict'; var x; var y; f(); }", environment => { ecma_version => 6, source_type => "module" } },
                    {
                        code => "export var x;
                        var y;
                        var z;",
                        environment => {
                            ecma_version => 6,
                            source_type => "module"
                        }
                    },
                    {
                        code =>
                            "var x;
                            export var y;
                            var z;",
                        environment => {
                            ecma_version => 6,
                            source_type => "module"
                        }
                    },
                    {
                        code =>
                            "var x;
                            var y;
                            export var z;",
                        environment => {
                            ecma_version => 6,
                            source_type => "module"
                        }
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    var x;
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        },
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    var x;
                                    foo();
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        }
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    var x;
                                    var y;
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        }
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    var x;
                                    var y;
                                    foo();
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        }
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    let x;
                                    var y;
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        }
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    foo();
                                    let x;
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        }
                    }
                ],
                invalid => [
                    {
                        code =>
                            "var first = 0;
                            function foo() {
                                first = 2;
                                second = 2;
                            }
                            var second = 0;",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first;
                               first = 1;
                               first = 2;
                               first = 3;
                               first = 4;
                               var second = 1;
                               second = 2;
                               first = second;
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first;
                               if (true) {
                                   var second = true;
                               }
                               first = second;
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               for (var i = 0; i < 10; i++) {
                                   alert(i);
                               }
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first = 10;
                               var i;
                               for (i = 0; i < first; i ++) {
                                   var second = i;
                               }
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first = 10;
                               var i;
                               switch (first) {
                                   case 10:
                                       var hello = 1;
                                       break;
                               }
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first = 10;
                               var i;
                               try {
                                   var hello = 1;
                               } catch (e) {
                                   alert('error');
                               }
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first = 10;
                               var i;
                               try {
                                   asdf;
                               } catch (e) {
                                   var hello = 1;
                               }
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first = 10;
                               while (first) {
                                   var hello = 1;
                               }
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first = 10;
                               do {
                                   var hello = 1;
                               } while (first == 10);
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "function foo() {
                               var first = [1,2,3];
                               for (var item in first) {
                                   item++;
                               }
                            }",
                        errors => [{ type => Identifier }],
                    },
                    {
                        code =>
                            "function foo() {
                               var first = [1,2,3];
                               var item;
                               for (item in first) {
                                   var hello = item;
                               }
                            }",
                        errors => [error]
                    },
                    {
                        code =>
                            "var foo = () => {
                               var first = [1,2,3];
                               var item;
                               for (item in first) {
                                   var hello = item;
                               }
                            }",
                        environment => { ecma_version => 6 },
                        errors => [error]
                    },
                    {
                        code => "'use strict'; 0; var x; f();",
                        errors => [error]
                    },
                    {
                        code => "'use strict'; var x; 'directive'; var y; f();",
                        errors => [error]
                    },
                    {
                        code => "function f() { 'use strict'; 0; var x; f(); }",
                        errors => [error]
                    },
                    {
                        code => "function f() { 'use strict'; var x; 'directive';  var y; f(); }",
                        errors => [error]
                    },
                    {
                        code =>
                            "export function f() {}
                            var x;",
                        environment => {
                            ecma_version => 6,
                            source_type => "module"
                        },
                        errors => [error]
                    },
                    {
                        code =>
                            "var x;
                            export function f() {}
                            var y;",
                        environment => {
                            ecma_version => 6,
                            source_type => "module"
                        },
                        errors => [error]
                    },
                    {
                        code =>
                            "import {foo} from 'foo';
                            export {foo};
                            var test = 1;",
                        environment => {
                            ecma_version => 6,
                            source_type => "module"
                        },
                        errors => [error]
                    },
                    {
                        code =>
                            "export {foo} from 'foo';
                            var test = 1;",
                        environment => {
                            ecma_version => 6,
                            source_type => "module"
                        },
                        errors => [error]
                    },
                    {
                        code =>
                            "export * from 'foo';
                            var test = 1;",
                        environment => {
                            ecma_version => 6,
                            source_type => "module"
                        },
                        errors => [error]
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    foo();
                                    var x;
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [error]
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    'use strict';
                                    var x;
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [error]
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    var x;
                                    foo();
                                    var y;
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [
                            error_builder
                                .line(5)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code =>
                            "class C {
                                static {
                                    if (foo) {
                                        var x;
                                    }
                                }
                            }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [error]
                    },
                    {
                        code => "class C {
                            static {
                                if (foo)
                                    var x;
                            }
                        }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [error]
                    }
                ]
            },
        )
    }
}
