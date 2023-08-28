use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_ex_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-ex-assign",
        languages => [Javascript],
        messages => [
            unexpected => "Do not assign to the exception parameter.",
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
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::Identifier;

    #[test]
    fn test_no_ex_assign_rule() {
        RuleTester::run(
            no_ex_assign_rule(),
            rule_tests! {
                valid => [
                    "try { } catch (e) { three = 2 + 1; }",
                    { code => "try { } catch ({e}) { this.something = 2; }", environment => { ecma_version => 6 } },
                    "function foo() { try { } catch (e) { return false; } }"
                ],
            invalid => [
                { code => "try { } catch (e) { e = 10; }", errors => [{ message_id => "unexpected", type => Identifier }] },
                { code => "try { } catch (ex) { ex = 10; }", errors => [{ message_id => "unexpected", type => Identifier }] },
                { code => "try { } catch (ex) { [ex] = []; }", environment => { ecma_version => 6 }, errors => [{ message_id => "unexpected", type => Identifier }] },
                { code => "try { } catch (ex) { ({x: ex = 0} = {}); }", environment => { ecma_version => 6 }, errors => [{ message_id => "unexpected", type => Identifier }] },
                { code => "try { } catch ({message}) { message = 10; }", environment => { ecma_version => 6 }, errors => [{ message_id => "unexpected", type => Identifier }] }
            ]
            },
        )
    }
}
