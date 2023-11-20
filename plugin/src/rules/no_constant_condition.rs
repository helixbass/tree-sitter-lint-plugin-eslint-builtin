use std::{collections::HashSet, sync::Arc};

use serde::Deserialize;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule,
};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{ExpressionStatement, ParenthesizedExpression},
    scope::ScopeManager,
    utils::ast_utils::is_constant,
};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    check_loops: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { check_loops: true }
    }
}

fn check_loop<'a>(
    node: Node<'a>,
    check_loops: bool,
    loop_set_stack: &mut [HashSet<Node<'a>>],
    context: &QueryMatchContext<'a, '_>,
) {
    if !check_loops {
        return;
    }

    let scope_manager = context.retrieve::<ScopeManager<'a>>();
    if
    /* node.test && */
    is_constant(
        &scope_manager.get_scope(node),
        node.field("condition")
            .skip_nodes_of_type(ExpressionStatement, SupportedLanguage::Javascript),
        true,
        context,
    ) {
        loop_set_stack.last_mut().unwrap().insert(node);
    }
}

pub fn no_constant_condition_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-constant-condition",
        languages => [Javascript],
        messages => [
            unexpected => "Unexpected constant condition.",
        ],
        options_type => Options,
        state => {
            [per-config]
            check_loops: bool = options.check_loops,

            [per-file-run]
            loop_set_stack: Vec<HashSet<Node<'a>>> = vec![Default::default()],
        },
        listeners => [
            r#"
              (ternary_expression) @c
              (if_statement) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                if /*node.test &&*/ is_constant(&scope_manager.get_scope(node), node.field("condition"), true, context) {
                    context.report(violation! {
                        node => node.field("condition").skip_parentheses(),
                        message_id => "unexpected",
                    });
                }
            },
            r#"
              (while_statement) @c
              (do_statement) @c
              (for_statement) @c
            "# => |node, context| {
                check_loop(node, self.check_loops, &mut self.loop_set_stack, context);
            },
            r#"
              (for_statement
                condition: (_) @c
              )
            "# => |node, context| {
                check_loop(node.parent().unwrap(), self.check_loops, &mut self.loop_set_stack, context);
            },
            "
              while_statement:exit,
              do_statement:exit,
              for_statement:exit
            " => |node, context| {
                if self.loop_set_stack.last().unwrap().contains(&node) {
                    self.loop_set_stack.last_mut().unwrap().remove(&node);
                    context.report(violation! {
                        node => node.field("condition").skip_nodes_of_types(&[ExpressionStatement, ParenthesizedExpression], SupportedLanguage::Javascript),
                        message_id => "unexpected",
                    });
                }
            },
            "
              (function) @c
              (function_declaration) @c
              (generator_function) @c
              (generator_function_declaration) @c
            " => |node, context| {
                self.loop_set_stack.push(Default::default());
            },
            "
              function:exit,
              function_declaration:exit,
              generator_function:exit,
              generator_function_declaration:exit
            " => |node, context| {
                self.loop_set_stack.pop().unwrap();
            },
            "
              (yield_expression) @c
            " => |node, context| {
                self.loop_set_stack.last_mut().unwrap().clear();
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use squalid::json_object;
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{
        get_instance_provider_factory,
        kind::{
            self, Array, ArrowFunction, AssignmentExpression, AugmentedAssignmentExpression,
            BinaryExpression, Function, Object, SequenceExpression, TemplateString, True,
            UnaryExpression,
        },
    };

    #[test]
    fn test_no_constant_condition_rule() {
        RuleTester::run_with_instance_provider_and_environment(
            no_constant_condition_rule(),
            rule_tests! {
                valid => [
                    "if(a);",
                    "if(a == 0);",
                    "if(a = f());",
                    "if(a += 1);",
                    "if(a |= 1);",
                    "if(a |= true);",
                    "if(a |= false);",
                    "if(a &= 1);",
                    "if(a &= true);",
                    "if(a &= false);",
                    "if(a >>= 1);",
                    "if(a >>= true);",
                    "if(a >>= false);",
                    "if(a >>>= 1);",
                    "if(a ??= 1);",
                    "if(a ??= true);",
                    "if(a ??= false);",
                    "if(a ||= b);",
                    "if(a ||= false);",
                    "if(a ||= 0);",
                    "if(a ||= void 0);",
                    "if(+(a ||= 1));",
                    "if(f(a ||= true));",
                    "if((a ||= 1) + 2);",
                    "if(1 + (a ||= true));",
                    "if(a ||= '' || false);",
                    "if(a ||= void 0 || null);",
                    "if((a ||= false) || b);",
                    "if(a || (b ||= false));",
                    "if((a ||= true) && b);",
                    "if(a && (b ||= true));",
                    "if(a &&= b);",
                    "if(a &&= true);",
                    "if(a &&= 1);",
                    "if(a &&= 'foo');",
                    "if((a &&= '') + false);",
                    "if('' + (a &&= null));",
                    "if(a &&= 1 && 2);",
                    "if((a &&= true) && b);",
                    "if(a && (b &&= true));",
                    "if((a &&= false) || b);",
                    "if(a || (b &&= false));",
                    "if(a ||= b ||= false);",
                    "if(a &&= b &&= true);",
                    "if(a ||= b &&= false);",
                    "if(a ||= b &&= true);",
                    "if(a &&= b ||= false);",
                    "if(a &&= b ||= true);",
                    "if(1, a);",
                    "if ('every' in []);",
                    "if (`\\\n${a}`) {}",
                    "if (`${a}`);",
                    "if (`${foo()}`);",
                    "if (`${a === 'b' && b==='a'}`);",
                    "if (`foo${a}` === 'fooa');",
                    "if (tag`a`);",
                    "if (tag`${a}`);",
                    "if (+(a || true));",
                    "if (-(a || true));",
                    "if (~(a || 1));",
                    "if (+(a && 0) === +(b && 0));",
                    "while(~!a);",
                    "while(a = b);",
                    "while(`${a}`);",
                    "for(;x < 10;);",
                    "for(;;);",
                    "for(;`${a}`;);",
                    "do{ }while(x)",
                    "q > 0 ? 1 : 2;",
                    "`${a}` === a ? 1 : 2",
                    "`foo${a}` === a ? 1 : 2",
                    "tag`a` === a ? 1 : 2",
                    "tag`${a}` === a ? 1 : 2",
                    "while(x += 3) {}",
                    "while(tag`a`) {}",
                    "while(tag`${a}`) {}",
                    "while(`\\\n${a}`) {}",

                    // #5228, typeof conditions
                    "if(typeof x === 'undefined'){}",
                    "if(`${typeof x}` === 'undefined'){}",
                    "if(a === 'str' && typeof b){}",
                    "typeof a == typeof b",
                    "typeof 'a' === 'string'|| typeof b === 'string'",
                    "`${typeof 'a'}` === 'string'|| `${typeof b}` === 'string'",

                    // #5726, void conditions
                    "if (void a || a);",
                    "if (a || void a);",

                    // #5693
                    "if(xyz === 'str1' && abc==='str2'){}",
                    "if(xyz === 'str1' || abc==='str2'){}",
                    "if(xyz === 'str1' || abc==='str2' && pqr === 5){}",
                    "if(typeof abc === 'string' && abc==='str2'){}",
                    "if(false || abc==='str'){}",
                    "if(true && abc==='str'){}",
                    "if(typeof 'str' && abc==='str'){}",
                    "if(abc==='str' || false || def ==='str'){}",
                    "if(true && abc==='str' || def ==='str'){}",
                    "if(true && typeof abc==='string'){}",

                    // #11181, string literals
                    "if('str1' && a){}",
                    "if(a && 'str'){}",

                    // #11306
                    "if ((foo || true) === 'baz') {}",
                    "if ((foo || 'bar') === 'baz') {}",
                    "if ((foo || 'bar') !== 'baz') {}",
                    "if ((foo || 'bar') == 'baz') {}",
                    "if ((foo || 'bar') != 'baz') {}",
                    "if ((foo || 233) > 666) {}",
                    "if ((foo || 233) < 666) {}",
                    "if ((foo || 233) >= 666) {}",
                    "if ((foo || 233) <= 666) {}",
                    "if ((key || 'k') in obj) {}",
                    "if ((foo || {}) instanceof obj) {}",
                    "if ((foo || 'bar' || 'bar') === 'bar');",
                    {
                        code => "if ((foo || 1n) === 'baz') {}",
                        environment => { ecma_version => 11 }
                    },
                    {
                        code => "if (a && 0n || b);",
                        environment => { ecma_version => 11 }
                    },
                    {
                        code => "if(1n && a){};",
                        environment => { ecma_version => 11 }
                    },

                    // #12225
                    "if ('' + [y] === '' + [ty]) {}",
                    "if ('a' === '' + [ty]) {}",
                    "if ('' + [y, m, d] === 'a') {}",
                    "if ('' + [y, 'm'] === '' + [ty, 'tm']) {}",
                    "if ('' + [y, 'm'] === '' + ['ty']) {}",
                    "if ([,] in\n\n($2))\n ;\nelse\n ;",
                    "if ([...x]+'' === 'y'){}",

                    // { check_loops => false }
                    { code => "while(true);", options => { check_loops => false } },
                    { code => "for(;true;);", options => { check_loops => false } },
                    { code => "do{}while(true)", options => { check_loops => false } },

                    "function* foo(){while(true){yield 'foo';}}",
                    "function* foo(){for(;true;){yield 'foo';}}",
                    "function* foo(){do{yield 'foo';}while(true)}",
                    "function* foo(){while (true) { while(true) {yield;}}}",
                    "function* foo() {for (; yield; ) {}}",
                    "function* foo() {for (; ; yield) {}}",
                    "function* foo() {while (true) {function* foo() {yield;}yield;}}",
                    "function* foo() { for (let x = yield; x < 10; x++) {yield;}yield;}",
                    "function* foo() { for (let x = yield; ; x++) { yield; }}",
                    "if (new Number(x) + 1 === 2) {}",

                    // #15467
                    "if([a]==[b]) {}",
                    "if (+[...a]) {}",
                    "if (+[...[...a]]) {}",
                    "if (`${[...a]}`) {}",
                    "if (`${[a]}`) {}",
                    "if (+[a]) {}",
                    "if (0 - [a]) {}",
                    "if (1 * [a]) {}",

                    // Boolean function
                    "if (Boolean(a)) {}",
                    "if (Boolean(...args)) {}",
                    "if (foo.Boolean(1)) {}",
                    "function foo(Boolean) { if (Boolean(1)) {} }",
                    "const Boolean = () => {}; if (Boolean(1)) {}",
                    // TODO: are these commented-out ones "supported"?
                    // { code => "if (Boolean()) {}"/*, globals: { Boolean: "off" }*/ },
                    "const undefined = 'lol'; if (undefined) {}",
                    // { code => "if (undefined) {}"/*, globals: { undefined: "off" }*/ }
                ],
                invalid => [
                    { code => "for(;true;);", errors => [{ message_id => "unexpected", type => True }] },
                    { code => "for(;``;);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "for(;`foo`;);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "for(;`foo${bar}`;);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "do{}while(true)", errors => [{ message_id => "unexpected", type => True }] },
                    { code => "do{}while('1')", errors => [{ message_id => "unexpected", type => kind::String }] },
                    { code => "do{}while(0)", errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "do{}while(t = -2)", errors => [{ message_id => "unexpected", type => AssignmentExpression }] },
                    { code => "do{}while(``)", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "do{}while(`foo`)", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "do{}while(`foo${bar}`)", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "true ? 1 : 2;", errors => [{ message_id => "unexpected", type => True }] },
                    { code => "1 ? 1 : 2;", errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "q = 0 ? 1 : 2;", errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "(q = 0) ? 1 : 2;", errors => [{ message_id => "unexpected", type => AssignmentExpression }] },
                    { code => "`` ? 1 : 2;", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "`foo` ? 1 : 2;", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "`foo${bar}` ? 1 : 2;", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(-2);", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(true);", errors => [{ message_id => "unexpected", type => True }] },
                    { code => "if(1);", errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "if({});", errors => [{ message_id => "unexpected", type => Object }] },
                    { code => "if(0 < 1);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(0 || 1);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(a, 1);", errors => [{ message_id => "unexpected", type => SequenceExpression }] },
                    { code => "if(`foo`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(``);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(`\\\n`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(`${'bar'}`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(`${'bar' + `foo`}`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(`foo${false || true}`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(`foo${0 || 1}`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(`foo${bar}`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(`${bar}foo`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "if(!(true || a));", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(!(a && void b && c));", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(0 || !(a && null));", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(1 + !(a || true));", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(!(null && a) > 1);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(+(!(a && 0)));", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(!typeof a === 'string');", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(-('foo' || a));", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(+(void a && b) === ~(1 || c));", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(a ||= true);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a ||= 5);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a ||= 'foo' || b);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a ||= b || /regex/);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a ||= b ||= true);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a ||= b ||= c || 1);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(!(a ||= true));", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(!(a ||= 'foo') === true);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(!(a ||= 'foo') === false);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(a || (b ||= true));", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if((a ||= 1) || b);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if((a ||= true) && true);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(true && (a ||= true));", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(a &&= false);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a &&= null);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a &&= void b);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a &&= 0 && b);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a &&= b && '');", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a &&= b &&= false);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(a &&= b &&= c && false);", errors => [{ message_id => "unexpected", type => AugmentedAssignmentExpression }] },
                    { code => "if(!(a &&= false));", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(!(a &&= 0) + 1);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(a && (b &&= false));", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if((a &&= null) && b);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(false || (a &&= false));", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if((a &&= false) || false);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },

                    { code => "while([]);", errors => [{ message_id => "unexpected", type => Array }] },
                    { code => "while(~!0);", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "while(x = 1);", errors => [{ message_id => "unexpected", type => AssignmentExpression }] },
                    { code => "while(function(){});", errors => [{ message_id => "unexpected", type => Function }] },
                    { code => "while(true);", errors => [{ message_id => "unexpected", type => True }] },
                    { code => "while(1);", errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "while(() => {});", errors => [{ message_id => "unexpected", type => ArrowFunction }] },
                    { code => "while(`foo`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "while(``);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "while(`${'foo'}`);", errors => [{ message_id => "unexpected", type => TemplateString }] },
                    { code => "while(`${'foo' + 'bar'}`);", errors => [{ message_id => "unexpected", type => TemplateString }] },

                    // #5228 , typeof conditions
                    { code => "if(typeof x){}", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(typeof 'abc' === 'string'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(a = typeof b){}", errors => [{ message_id => "unexpected", type => AssignmentExpression }] },
                    { code => "if(a, typeof b){}", errors => [{ message_id => "unexpected", type => SequenceExpression }] },
                    { code => "if(typeof 'a' == 'string' || typeof 'b' == 'string'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "while(typeof x){}", errors => [{ message_id => "unexpected", type => UnaryExpression }] },

                    // #5726, void conditions
                    { code => "if(1 || void x);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(void x);", errors => [{ message_id => "unexpected", type => UnaryExpression }] },
                    { code => "if(y = void x);", errors => [{ message_id => "unexpected", type => AssignmentExpression }] },
                    { code => "if(x, void x);", errors => [{ message_id => "unexpected", type => SequenceExpression }] },
                    { code => "if(void x === void y);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(void x && a);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(a && void x);", errors => [{ message_id => "unexpected", type => BinaryExpression }] },

                    // #5693
                    { code => "if(false && abc==='str'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(true || abc==='str'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(1 || abc==='str'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(abc==='str' || true){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(abc==='str' || true || def ==='str'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(false || true){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(typeof abc==='str' || true){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },

                    // #11181, string literals
                    { code => "if('str' || a){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if('str' || abc==='str'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if('str1' || 'str2'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if('str1' && 'str2'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(abc==='str' || 'str'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },
                    { code => "if(a || 'str'){}", errors => [{ message_id => "unexpected", type => BinaryExpression }] },

                    {
                        code => "function* foo(){while(true){} yield 'foo';}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "function* foo(){while(true){if (true) {yield 'foo';}}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "function* foo(){while(true){yield 'foo';} while(true) {}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "var a = function* foo(){while(true){} yield 'foo';}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "while (true) { function* foo() {yield;}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "function* foo(){if (true) {yield 'foo';}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "function* foo() {for (let foo = yield; true;) {}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "function* foo() {for (foo = yield; true;) {}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "function foo() {while (true) {function* bar() {while (true) {yield;}}}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "function foo() {while (true) {const bar = function*() {while (true) {yield;}}}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },
                    {
                        code => "function* foo() { for (let foo = 1 + 2 + 3 + (yield); true; baz) {}}",
                        errors => [{ message_id => "unexpected", type => True }]
                    },

                    // #12225
                    {
                        code => "if([a]) {}",
                        errors => [{ message_id => "unexpected", type => Array }]
                    },
                    {
                        code => "if([]) {}",
                        errors => [{ message_id => "unexpected", type => Array }]
                    },
                    {
                        code => "if(''+['a']) {}",
                        errors => [{ message_id => "unexpected", type => BinaryExpression }]
                    },
                    {
                        code => "if(''+[]) {}",
                        errors => [{ message_id => "unexpected", type => BinaryExpression }]
                    },
                    {
                        code => "if(+1) {}",
                        errors => [{ message_id => "unexpected", type => UnaryExpression }]
                    },
                    {
                        code => "if ([,] + ''){}",
                        errors => [{ message_id => "unexpected", type => BinaryExpression }]
                    },

                    // #13238
                    { code => "if(/foo/ui);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "if(0n);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "if(0b0n);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "if(0o0n);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "if(0x0n);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "if(0b1n);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "if(0o1n);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "if(0x1n);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => kind::Number }] },
                    { code => "if(0x1n || foo);", environment => { ecma_version => 11 }, errors => [{ message_id => "unexpected", type => BinaryExpression }] },

                    // Classes and instances are always truthy
                    { code => "if(class {}) {}", errors => [{ message_id => "unexpected" }] },
                    { code => "if(new Foo()) {}", errors => [{ message_id => "unexpected" }] },

                    // Boxed primitives are always truthy
                    { code => "if(new Boolean(foo)) {}", errors => [{ message_id => "unexpected" }] },
                    { code => "if(new String(foo)) {}", errors => [{ message_id => "unexpected" }] },
                    { code => "if(new Number(foo)) {}", errors => [{ message_id => "unexpected" }] },

                    // Spreading a constant array
                    { code => "if(`${[...['a']]}`) {}", errors => [{ message_id => "unexpected" }] },

                    /*
                     * undefined is always falsy (except in old browsers that let you
                     * re-assign, but that's an obscure enough edge case to not worry about)
                     */
                    { code => "if (undefined) {}", errors => [{ message_id => "unexpected" }] },

                    // Coercion to boolean via Boolean function
                    { code => "if (Boolean(1)) {}", errors => [{ message_id => "unexpected" }] },
                    { code => "if (Boolean()) {}", errors => [{ message_id => "unexpected" }] },
                    { code => "if (Boolean([a])) {}", errors => [{ message_id => "unexpected" }] },
                    { code => "if (Boolean(1)) { function Boolean() {}}", errors => [{ message_id => "unexpected" }] }
                ]
            },
            get_instance_provider_factory(),
            json_object!({
                "ecma_version": 2021,
            }),
        )
    }
}
