use std::sync::Arc;

use itertools::Itertools;
use tree_sitter_lint::{rule, violation, Rule};

use crate::{scope::ScopeManager, utils::ast_utils};

pub fn no_ex_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-ex-assign",
        languages => [Javascript],
        messages => [
            unexpected => "Do not assign to the exception parameter.",
        ],
        listeners => [
            r#"
              (catch_clause) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();

                scope_manager.get_declared_variables(node).unwrap_or_default().into_iter().for_each(|variable| {
                    ast_utils::get_modifying_references(&variable.references().collect_vec())
                        .into_iter()
                        .for_each(|reference| {
                            context.report(violation! {
                                node => reference.identifier(),
                                message_id => "unexpected",
                            });
                        });
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{kind::Identifier, get_instance_provider_factory};

    #[test]
    fn test_no_ex_assign_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
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
            get_instance_provider_factory(),
        )
    }
}
