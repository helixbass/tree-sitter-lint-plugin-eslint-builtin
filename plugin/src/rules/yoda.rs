use std::{borrow::Cow, cmp::Ordering, collections::HashMap, sync::Arc};

use once_cell::sync::Lazy;
use serde::Deserialize;
use squalid::{regex, OptionExt};
use tree_sitter_lint::{
    rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule, SourceTextProvider,
};

use crate::{
    ast_helpers::{get_number_literal_value, is_logical_expression, NodeExtJs, NumberOrBigInt},
    kind::{self, is_literal_kind, BinaryExpression, ParenthesizedExpression, UnaryExpression},
    utils::ast_utils,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum AlwaysNever {
    Always,
    #[default]
    Never,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionsVariants {
    EmptyList(),
    JustAlwaysNever([AlwaysNever; 1]),
    AlwaysNeverAndOptionsObject(AlwaysNever, OptionsObject),
}

impl Default for OptionsVariants {
    fn default() -> Self {
        Self::EmptyList()
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct OptionsObject {
    except_range: bool,
    only_equality: bool,
}

struct Options {
    always_never: AlwaysNever,
    except_range: bool,
    only_equality: bool,
}

impl Options {
    pub fn from_always_never_and_options_object(
        always_never: AlwaysNever,
        options_object: OptionsObject,
    ) -> Self {
        Self {
            always_never,
            except_range: options_object.except_range,
            only_equality: options_object.only_equality,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        OptionsVariants::default().into()
    }
}

impl From<OptionsVariants> for Options {
    fn from(value: OptionsVariants) -> Self {
        match value {
            OptionsVariants::EmptyList() => {
                Self::from_always_never_and_options_object(Default::default(), Default::default())
            }
            OptionsVariants::JustAlwaysNever(always_never) => {
                Self::from_always_never_and_options_object(always_never[0], Default::default())
            }
            OptionsVariants::AlwaysNeverAndOptionsObject(always_never, options_object) => {
                Self::from_always_never_and_options_object(always_never, options_object)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Options {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(OptionsVariants::deserialize(deserializer)?.into())
    }
}

fn is_equality_operator(operator: &str) -> bool {
    regex!(r#"^(==|===)$"#).is_match(operator)
}

fn is_range_test_operator(operator: &str) -> bool {
    ["<", "<="].contains(&operator)
}

fn is_negative_numeric_literal(node: Node) -> bool {
    node.kind() == UnaryExpression
        && node.field("operator").kind() == "-"
        && node.field("argument").kind() == kind::Number
}

fn looks_like_literal(node: Node) -> bool {
    is_negative_numeric_literal(node) || ast_utils::is_static_template_literal(node)
}

fn get_normalized_literal<'a>(
    node: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<StringOrNumber<'a>> {
    if node.kind() == kind::Number {
        return Some(get_number_literal_value(node, context).into());
    }

    if is_negative_numeric_literal(node) {
        return Some((get_number_literal_value(node.field("argument"), context) * -1).into());
    }

    ast_utils::get_static_string_value(node, context).map(Into::into)
}

#[derive(Debug)]
enum StringOrNumber<'a> {
    Number(NumberOrBigInt),
    String(Cow<'a, str>),
}

impl<'a> From<NumberOrBigInt> for StringOrNumber<'a> {
    fn from(value: NumberOrBigInt) -> Self {
        Self::Number(value)
    }
}

impl<'a> From<Cow<'a, str>> for StringOrNumber<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        Self::String(value)
    }
}

impl<'a> PartialEq for StringOrNumber<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            _ => false,
        }
    }
}

impl<'a> PartialOrd for StringOrNumber<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.partial_cmp_value(b),
            (Self::String(a), Self::String(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

fn is_between_test<'a>(
    operator: &str,
    left: Node<'a>,
    right: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> bool {
    if !(operator == "&&"
        && ast_utils::is_same_reference(left.field("right"), right.field("left"), None, context))
    {
        return false;
    }
    match (
        get_normalized_literal(left.field("left"), context),
        get_normalized_literal(right.field("right"), context),
    ) {
        (None, None) => false,
        (None, _) | (_, None) => true,
        (Some(left_literal), Some(right_literal)) => left_literal <= right_literal,
    }
}

fn is_outside_test<'a>(
    operator: &str,
    left: Node<'a>,
    right: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> bool {
    if !(operator == "||"
        && ast_utils::is_same_reference(left.field("left"), right.field("right"), None, context))
    {
        return false;
    }
    match (
        get_normalized_literal(left.field("right"), context),
        get_normalized_literal(right.field("left"), context),
    ) {
        (None, None) => false,
        (None, _) | (_, None) => true,
        (Some(left_literal), Some(right_literal)) => left_literal <= right_literal,
    }
}

fn is_range_test<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
    if !is_logical_expression(node) {
        return false;
    }

    let left = node.field("left");
    let right = node.field("right");
    let operator = node.field("operator").kind();

    left.kind() == BinaryExpression
        && right.kind() == BinaryExpression
        && is_range_test_operator(left.field("operator").kind())
        && is_range_test_operator(right.field("operator").kind())
        && (is_between_test(operator, left, right, context)
            || is_outside_test(operator, left, right, context))
        && node.parent().unwrap().kind() == ParenthesizedExpression
}

static OPERATOR_FLIP_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        ("===", "==="),
        ("!==", "!=="),
        ("==", "=="),
        ("!=", "!="),
        ("<", ">"),
        (">", "<"),
        ("<=", ">="),
        (">=", "<="),
    ]
    .into_iter()
    .collect()
});

fn get_flipped_string<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> String {
    let left = node.field("left");
    let right = node.field("right");
    let operator = node.field("operator").kind();
    let operator_token = context
        .get_first_token_between(
            left,
            right,
            Some(|token: Node| token.text(context) == operator),
        )
        .unwrap();
    let last_left_token =
        context.get_token_before(operator_token, Option::<fn(Node) -> bool>::None);
    let first_right_token =
        context.get_token_after(operator_token, Option::<fn(Node) -> bool>::None);

    let left_text = context.slice(node.range().start_byte..last_left_token.range().end_byte);
    let text_before_operator =
        context.slice(last_left_token.range().end_byte..operator_token.range().start_byte);
    let text_after_operator =
        context.slice(operator_token.range().end_byte..first_right_token.range().start_byte);
    let right_text = context.slice(first_right_token.range().start_byte..node.range().end_byte);

    let token_before = context.maybe_get_token_before(node, Option::<fn(Node) -> bool>::None);
    let token_after = context.maybe_get_token_after(node, Option::<fn(Node) -> bool>::None);
    let mut prefix = "";
    let mut suffix = "";

    if token_before.matches(|token_before| {
        token_before.range().end_byte == node.range().start_byte
            && !ast_utils::can_tokens_be_adjacent(token_before, first_right_token, context)
    }) {
        prefix = " ";
    }

    if token_after.matches(|token_after| {
        node.range().end_byte == token_after.range().start_byte
            && !ast_utils::can_tokens_be_adjacent(last_left_token, token_after, context)
    }) {
        suffix = " ";
    }

    format!(
        "{prefix}{right_text}{text_before_operator}{}{text_after_operator}{left_text}{suffix}",
        OPERATOR_FLIP_MAP[operator_token.kind()]
    )
}

pub fn yoda_rule() -> Arc<dyn Rule> {
    rule! {
        name => "yoda",
        languages => [Javascript],
        messages => [
            expected => "Expected literal to be on the {{expected_side}} side of {{operator}}.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            always: bool = options.always_never == AlwaysNever::Always,
            except_range: bool = options.except_range,
            only_equality: bool = options.only_equality,
        },
        listeners => [
            r#"
              (binary_expression
                operator: [
                  "=="
                  "==="
                  "!="
                  "!=="
                  "<"
                  ">"
                  "<="
                  ">="
                ]
              ) @c
            "# => |node, context| {
                let expected_literal = if self.always {
                    node.field("left").skip_parentheses()
                } else {
                    node.field("right").skip_parentheses()
                };
                let expected_non_literal = if self.always {
                    node.field("right").skip_parentheses()
                } else {
                    node.field("left").skip_parentheses()
                };

                if (is_literal_kind(expected_non_literal.kind()) ||
                    looks_like_literal(expected_non_literal)) &&
                    !(
                        is_literal_kind(expected_literal.kind()) ||
                        looks_like_literal(expected_literal)
                    ) &&
                    !(self.only_equality && !is_equality_operator(node.field("operator").kind())) &&
                    !(self.except_range && is_range_test(node.next_non_parentheses_ancestor(), context)) {
                    context.report(violation! {
                        node => node,
                        message_id => "expected",
                        data => {
                            operator => node.field("operator").kind(),
                            expected_side => if self.always {
                                "left"
                            } else {
                                "right"
                            },
                        },
                        fix => |fixer| {
                            fixer.replace_text(node, get_flipped_string(node, context));
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::BinaryExpression;

    #[test]
    fn test_yoda_rule() {
        RuleTester::run(
            yoda_rule(),
            rule_tests! {
                valid => [
                    // "never" mode
                    { code => r#"if (value === "red") {}"#, options => ["never"] },
                    { code => "if (value === value) {}", options => ["never"] },
                    { code => "if (value != 5) {}", options => ["never"] },
                    { code => "if (5 & foo) {}", options => ["never"] },
                    { code => "if (5 === 4) {}", options => ["never"] },
                    {
                        code => "if (value === `red`) {}",
                        options => ["never"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (`red` === `red`) {}",
                        options => ["never"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (`${foo}` === `red`) {}",
                        options => ["never"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => r#"if (`${""}` === `red`) {}"#,
                        options => ["never"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => r#"if (`${"red"}` === foo) {}"#,
                        options => ["never"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (b > `a` && b > `a`) {}",
                        options => ["never"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => r#"if (`b` > `a` && "b" > "a") {}"#,
                        options => ["never"],
                        environment => { ecma_version => 2015 }
                    },

                    // "always" mode
                    { code => r#"if ("blue" === value) {}"#, options => ["always"] },
                    { code => "if (value === value) {}", options => ["always"] },
                    { code => "if (4 != value) {}", options => ["always"] },
                    { code => "if (foo & 4) {}", options => ["always"] },
                    { code => "if (5 === 4) {}", options => ["always"] },
                    {
                        code => "if (`red` === value) {}",
                        options => ["always"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (`red` === `red`) {}",
                        options => ["always"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (`red` === `${foo}`) {}",
                        options => ["always"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => r#"if (`red` === `${""}`) {}"#,
                        options => ["always"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => r#"if (foo === `${"red"}`) {}"#,
                        options => ["always"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (`a` > b && `a` > b) {}",
                        options => ["always"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => r#"if (`b` > `a` && "b" > "a") {}"#,
                        options => ["always"],
                        environment => { ecma_version => 2015 }
                    },

                    // Range exception
                    {
                        code => r#"if ("a" < x && x < MAX ) {}"#,
                        options => ["never", { except_range => true }],
                    },
                    {
                        code => "if (1 < x && x < MAX ) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if ('a' < x && x < MAX ) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (x < `x` || `x` <= x) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (0 < x && x <= 1) {}",
                        options => ["never", { except_range => true }],
                    },
                    {
                        code => "if (0 <= x && x < 1) {}",
                        options => ["always", { except_range => true }]
                    },
                    {
                        code => "if ('blue' < x.y && x.y < 'green') {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (0 < x[``] && x[``] < 100) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (0 < x[''] && x[``] < 100) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code =>
                            "if (a < 4 || (b[c[0]].d['e'] < 0 || 1 <= b[c[0]].d['e'])) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (0 <= x['y'] && x['y'] <= 100) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (a < 0 && (0 < b && b < 1)) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if ((0 < a && a < 1) && b < 0) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (-1 < x && x < 0) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (0 <= this.prop && this.prop <= 1) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (0 <= index && index < list.length) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (ZERO <= index && index < 100) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (value <= MIN || 10 < value) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (value <= 0 || MAX < value) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => r#"if (0 <= a.b && a["b"] <= 100) {}"#,
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (0 <= a.b && a[`b`] <= 100) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (-1n < x && x <= 1n) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "if (-1n <= x && x < 1n) {}",
                        options => ["always", { except_range => true }],
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "if (x < `1` || `1` < x) {}",
                        options => ["always", { except_range => true }],
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "if (1 <= a['/(?<zero>0)/'] && a[/(?<zero>0)/] <= 100) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2018 }
                    },
                    {
                        code => "if (x <= `bar` || `foo` < x) {}",
                        options => ["always", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if ('a' < x && x < MAX ) {}",
                        options => ["always", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if ('a' < x && x < MAX ) {}",
                        options => ["always"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (MIN < x && x < 'a' ) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (MIN < x && x < 'a' ) {}",
                        options => ["never"],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (`blue` < x.y && x.y < `green`) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (0 <= x[`y`] && x[`y`] <= 100) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => r#"if (0 <= x[`y`] && x["y"] <= 100) {}"#,
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if ('a' <= x && x < 'b') {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (x < -1n || 1n <= x) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "if (x < -1n || 1n <= x) {}",
                        options => ["always", { except_range => true }],
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "if (1 < a && a <= 2) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (x < -1 || 1 < x) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (x <= 'bar' || 'foo' < x) {}",
                        options => ["always", { except_range => true }]
                    },
                    {
                        code => "if (x < 0 || 1 <= x) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if('a' <= x && x < MAX) {}",
                        options => ["never", { except_range => true }]
                    },
                    {
                        code => "if (0 <= obj?.a && obj?.a < 1) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2020 }
                    },

                    // onlyEquality
                    {
                        code => "if (0 < x && x <= 1) {}",
                        options => ["never", { only_equality => true }]
                    },
                    {
                        code => "if (x !== 'foo' && 'foo' !== x) {}",
                        options => ["never", { only_equality => true }]
                    },
                    {
                        code => "if (x < 2 && x !== -3) {}",
                        options => ["always", { only_equality => true }]
                    },
                    {
                        code => "if (x !== `foo` && `foo` !== x) {}",
                        options => ["never", { only_equality => true }],
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (x < `2` && x !== `-3`) {}",
                        options => ["always", { only_equality => true }],
                        environment => { ecma_version => 2015 }
                    }
                ],
                invalid => [
                    {
                        code => "if (x <= 'foo' || 'bar' < x) {}",
                        output => "if ('foo' >= x || 'bar' < x) {}",
                        options => ["always", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => r#"if ("red" == value) {}"#,
                        output => r#"if (value == "red") {}"#,
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "==" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (true === value) {}",
                        output => "if (value === true) {}",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (5 != value) {}",
                        output => "if (value != 5) {}",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "!=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (5n != value) {}",
                        output => "if (value != 5n) {}",
                        options => ["never"],
                        environment => { ecma_version => 2020 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "!=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (null !== value) {}",
                        output => "if (value !== null) {}",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "!==" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => r#"if ("red" <= value) {}"#,
                        output => r#"if (value >= "red") {}"#,
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (`red` <= value) {}",
                        output => "if (value >= `red`) {}",
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (`red` <= `${foo}`) {}",
                        output => "if (`${foo}` >= `red`) {}",
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => r#"if (`red` <= `${"red"}`) {}"#,
                        output => r#"if (`${"red"}` >= `red`) {}"#,
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (true >= value) {}",
                        output => "if (value <= true) {}",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => ">=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = (5 < value) ? true : false",
                        output => "var foo = (value > 5) ? true : false",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function foo() { return (null > value); }",
                        output => "function foo() { return (value < null); }",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => ">" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (-1 < str.indexOf(substr)) {}",
                        output => "if (str.indexOf(substr) > -1) {}",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => r#"if (value == "red") {}"#,
                        output => r#"if ("red" == value) {}"#,
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "==" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (value == `red`) {}",
                        output => "if (`red` == value) {}",
                        options => ["always"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "==" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (value === true) {}",
                        output => "if (true === value) {}",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (value === 5n) {}",
                        output => "if (5n === value) {}",
                        options => ["always"],
                        environment => { ecma_version => 2020 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => r#"if (`${"red"}` <= `red`) {}"#,
                        output => r#"if (`red` >= `${"red"}`) {}"#,
                        options => ["always"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (a < 0 && 0 <= b && b < 1) {}",
                        output => "if (a < 0 && b >= 0 && b < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a && a < 1 && b < 1) {}",
                        output => "if (a >= 0 && a < 1 && b < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (1 < a && a < 0) {}",
                        output => "if (a > 1 && a < 0) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "0 < a && a < 1",
                        output => "a > 0 && a < 1",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "var a = b < 0 || 1 <= b;",
                        output => "var a = b < 0 || b >= 1;",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= x && x < -1) {}",
                        output => "if (x >= 0 && x < -1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "var a = (b < 0 && 0 <= b);",
                        output => "var a = (0 > b && 0 <= b);",
                        options => ["always", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "var a = (b < `0` && `0` <= b);",
                        output => "var a = (`0` > b && `0` <= b);",
                        options => ["always", { except_range => true }],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (`green` < x.y && x.y < `blue`) {}",
                        output => "if (x.y > `green` && x.y < `blue`) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[b] && a['b'] < 1) {}",
                        output => "if (a[b] >= 0 && a['b'] < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[b] && a[`b`] < 1) {}",
                        output => "if (a[b] >= 0 && a[`b`] < 1) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (`0` <= a[b] && a[`b`] < `1`) {}",
                        output => "if (a[b] >= `0` && a[`b`] < `1`) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[b] && a.b < 1) {}",
                        output => "if (a[b] >= 0 && a.b < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[''] && a.b < 1) {}",
                        output => "if (a[''] >= 0 && a.b < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[''] && a[' '] < 1) {}",
                        output => "if (a[''] >= 0 && a[' '] < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[''] && a[null] < 1) {}",
                        output => "if (a[''] >= 0 && a[null] < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[``] && a[null] < 1) {}",
                        output => "if (a[``] >= 0 && a[null] < 1) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[''] && a[b] < 1) {}",
                        output => "if (a[''] >= 0 && a[b] < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[''] && a[b()] < 1) {}",
                        output => "if (a[''] >= 0 && a[b()] < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[``] && a[b()] < 1) {}",
                        output => "if (a[``] >= 0 && a[b()] < 1) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a[b()] && a[b()] < 1) {}",
                        output => "if (a[b()] >= 0 && a[b()] < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= a.null && a[/(?<zero>0)/] <= 1) {}",
                        output => "if (a.null >= 0 && a[/(?<zero>0)/] <= 1) {}",
                        options => ["never", { except_range => true }],
                        environment => { ecma_version => 2018 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (3 == a) {}",
                        output => "if (a == 3) {}",
                        options => ["never", { only_equality => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "==" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "foo(3 === a);",
                        output => "foo(a === 3);",
                        options => ["never", { only_equality => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "foo(a === 3);",
                        output => "foo(3 === a);",
                        options => ["always", { only_equality => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "foo(a === `3`);",
                        output => "foo(`3` === a);",
                        options => ["always", { only_equality => true }],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 <= x && x < 1) {}",
                        output => "if (x >= 0 && x < 1) {}",
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if ( /* a */ 0 /* b */ < /* c */ foo /* d */ ) {}",
                        output => "if ( /* a */ foo /* b */ > /* c */ 0 /* d */ ) {}",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if ( /* a */ foo /* b */ > /* c */ 0 /* d */ ) {}",
                        output => "if ( /* a */ 0 /* b */ < /* c */ foo /* d */ ) {}",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => ">" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (foo()===1) {}",
                        output => "if (1===foo()) {}",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (foo()     === 1) {}",
                        output => "if (1     === foo()) {}",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },

                    // https://github.com/eslint/eslint/issues/7326
                    {
                        code => "while (0 === (a));",
                        output => "while ((a) === 0);",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "while (0 === (a = b));",
                        output => "while ((a = b) === 0);",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "while ((a) === 0);",
                        output => "while (0 === (a));",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "while ((a = b) === 0);",
                        output => "while (0 === (a = b));",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (((((((((((foo)))))))))) === ((((((5)))))));",
                        output => "if (((((((5)))))) === ((((((((((foo)))))))))));",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "===" },
                                type => BinaryExpression
                            }
                        ],
                    },

                    // Adjacent tokens tests
                    {
                        code => "function *foo() { yield(1) < a }",
                        output => "function *foo() { yield a > (1) }",
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield((1)) < a }",
                        output => "function *foo() { yield a > ((1)) }",
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield 1 < a }",
                        output => "function *foo() { yield a > 1 }",
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield/**/1 < a }",
                        output => "function *foo() { yield/**/a > 1 }",
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield(1) < ++a }",
                        output => "function *foo() { yield++a > (1) }",
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield(1) < (a) }",
                        output => "function *foo() { yield(a) > (1) }",
                        options => ["never"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "x=1 < a",
                        output => "x=a > 1",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield++a < 1 }",
                        output => "function *foo() { yield 1 > ++a }",
                        options => ["always"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield(a) < 1 }",
                        output => "function *foo() { yield 1 > (a) }",
                        options => ["always"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield a < 1 }",
                        output => "function *foo() { yield 1 > a }",
                        options => ["always"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield/**/a < 1 }",
                        output => "function *foo() { yield/**/1 > a }",
                        options => ["always"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "function *foo() { yield++a < (1) }",
                        output => "function *foo() { yield(1) > ++a }",
                        options => ["always"],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "x=a < 1",
                        output => "x=1 > a",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "0 < f()in obj",
                        output => "f() > 0 in obj",
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "1 > x++instanceof foo",
                        output => "x++ < 1 instanceof foo",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => ">" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "x < ('foo')in bar",
                        output => "('foo') > x in bar",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "false <= ((x))in foo",
                        output => "((x)) >= false in foo",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "x >= (1)instanceof foo",
                        output => "(1) <= x instanceof foo",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => ">=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "false <= ((x)) in foo",
                        output => "((x)) >= false in foo",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "x >= 1 instanceof foo",
                        output => "1 <= x instanceof foo",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => ">=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "x >= 1/**/instanceof foo",
                        output => "1 <= x/**/instanceof foo",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => ">=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "(x >= 1)instanceof foo",
                        output => "(1 <= x)instanceof foo",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => ">=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "(x) >= (1)instanceof foo",
                        output => "(1) <= (x)instanceof foo",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => ">=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "1 > x===foo",
                        output => "x < 1===foo",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => ">" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "1 > x",
                        output => "x < 1",
                        options => ["never"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => ">" },
                                type => BinaryExpression
                            }
                        ]
                    },

                    {
                        code => "if (`green` < x.y && x.y < `blue`) {}",
                        output => "if (`green` < x.y && `blue` > x.y) {}",
                        options => ["always", { except_range => true }],
                        environment => { ecma_version => 2015 },
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if('a' <= x && x < 'b') {}",
                        output => "if('a' <= x && 'b' > x) {}",
                        options => ["always"],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "left", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if ('b' <= x && x < 'a') {}",
                        output => "if (x >= 'b' && x < 'a') {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if('a' <= x && x < 1) {}",
                        output => "if(x >= 'a' && x < 1) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<=" },
                                type => BinaryExpression
                            }
                        ]
                    },
                    {
                        code => "if (0 < a && b < max) {}",
                        output => "if (a > 0 && b < max) {}",
                        options => ["never", { except_range => true }],
                        errors => [
                            {
                                message_id => "expected",
                                data => { expected_side => "right", operator => "<" },
                                type => BinaryExpression
                            }
                        ]
                    }
                ]
            },
        )
    }
}
