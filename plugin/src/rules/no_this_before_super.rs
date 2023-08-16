use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_this_before_super_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-this-before-super",
        languages => [Javascript],
        messages => [
            no_before_super => "'{{kind}}' is not allowed before 'super()'.",
        ],
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::{Super, This};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_this_before_super_rule() {
        RuleTester::run(
            no_this_before_super_rule(),
            rule_tests! {
                valid => [
                    /*
                     * if the class has no extends or `extends null`, just ignore.
                     * those classes cannot call `super()`.
                     */
                    "class A { }",
                    "class A { constructor() { } }",
                    "class A { constructor() { this.b = 0; } }",
                    "class A { constructor() { this.b(); } }",
                    "class A extends null { }",
                    "class A extends null { constructor() { } }",

                    // allows `this`/`super` after `super()`.
                    "class A extends B { }",
                    "class A extends B { constructor() { super(); } }",
                    "class A extends B { constructor() { super(); this.c = this.d; } }",
                    "class A extends B { constructor() { super(); this.c(); } }",
                    "class A extends B { constructor() { super(); super.c(); } }",
                    "class A extends B { constructor() { if (true) { super(); } else { super(); } this.c(); } }",
                    "class A extends B { constructor() { foo = super(); this.c(); } }",
                    "class A extends B { constructor() { foo += super().a; this.c(); } }",
                    "class A extends B { constructor() { foo |= super().a; this.c(); } }",
                    "class A extends B { constructor() { foo &= super().a; this.c(); } }",

                    // allows `this`/`super` in nested executable scopes, even if before `super()`.
                    "class A extends B { constructor() { class B extends C { constructor() { super(); this.d = 0; } } super(); } }",
                    "class A extends B { constructor() { var B = class extends C { constructor() { super(); this.d = 0; } }; super(); } }",
                    "class A extends B { constructor() { function c() { this.d(); } super(); } }",
                    "class A extends B { constructor() { var c = function c() { this.d(); }; super(); } }",
                    "class A extends B { constructor() { var c = () => this.d(); super(); } }",

                    // ignores out of constructors.
                    "class A { b() { this.c = 0; } }",
                    "class A extends B { c() { this.d = 0; } }",
                    "function a() { this.b = 0; }",

                    // multi code path.
                    "class A extends B { constructor() { if (a) { super(); this.a(); } else { super(); this.b(); } } }",
                    "class A extends B { constructor() { if (a) super(); else super(); this.a(); } }",
                    "class A extends B { constructor() { try { super(); } finally {} this.a(); } }",

                    // https://github.com/eslint/eslint/issues/5261
                    "class A extends B { constructor(a) { super(); for (const b of a) { this.a(); } } }",
                    "class A extends B { constructor(a) { for (const b of a) { foo(b); } super(); } }",

                    // https://github.com/eslint/eslint/issues/5319
                    "class A extends B { constructor(a) { super(); this.a = a && function(){} && this.foo; } }",

                    // https://github.com/eslint/eslint/issues/5394
                    "class A extends Object {
                        constructor() {
                            super();
                            for (let i = 0; i < 0; i++);
                            this;
                        }
                    }",

                    // https://github.com/eslint/eslint/issues/5894
                    "class A { constructor() { return; this; } }",
                    "class A extends B { constructor() { return; this; } }",

                    // https://github.com/eslint/eslint/issues/8848
                    "
                        class A extends B {
                            constructor(props) {
                                super(props);

                                try {
                                    let arr = [];
                                    for (let a of arr) {
                                    }
                                } catch (err) {
                                }
                            }
                        }
                    ",

                    // Class field initializers are always evaluated after `super()`.
                    "class C { field = this.toString(); }",
                    "class C extends B { field = this.foo(); }",
                    "class C extends B { field = this.foo(); constructor() { super(); } }",
                    "class C extends B { field = this.foo(); constructor() { } }" // < in this case, initializers are never evaluated.
                ],
                invalid => [
                    // disallows all `this`/`super` if `super()` is missing.
                    {
                        code => "class A extends B { constructor() { this.c = 0; } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { this.c(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { super.c(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "super" }, type => Super }]
                    },

                    // disallows `this`/`super` before `super()`.
                    {
                        code => "class A extends B { constructor() { this.c = 0; super(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { this.c(); super(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { super.c(); super(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "super" }, type => Super }]
                    },

                    // disallows `this`/`super` in arguments of `super()`.
                    {
                        code => "class A extends B { constructor() { super(this.c); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { super(this.c()); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { super(super.c()); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "super" }, type => Super }]
                    },

                    // even if is nested, reports correctly.
                    {
                        code => "class A extends B { constructor() { class C extends D { constructor() { super(); this.e(); } } this.f(); super(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This, column => 96 }]
                    },
                    {
                        code => "class A extends B { constructor() { class C extends D { constructor() { this.e(); super(); } } super(); this.f(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This, column => 73 }]
                    },

                    // multi code path.
                    {
                        code => "class A extends B { constructor() { if (a) super(); this.a(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { try { super(); } finally { this.a; } } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { try { super(); } catch (err) { } this.a; } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { foo &&= super().a; this.c(); } }",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { foo ||= super().a; this.c(); } }",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { foo ??= super().a; this.c(); } }",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    }
                ]
            },
        )
    }
}
