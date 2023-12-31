use std::collections::HashMap;

use tree_sitter_lint::{
    Configuration, ConfigurationBuilder, ErrorLevel, RuleConfigurationValueBuilder,
};

pub fn all() -> Configuration {
    ConfigurationBuilder::default()
        .rules(
            [
                "for-direction",
                "no-async-promise-executor",
                "no-await-in-loop",
                "no-compare-neg-zero",
                "no-cond-assign",
                "no-debugger",
                "no-dupe-class-members",
                "max-params",
                "max-nested-callbacks",
                "no-dupe-else-if",
                "no-dupe-keys",
                "no-duplicate-case",
                "no-unneeded-ternary",
                "no-array-constructor",
                "no-eq-null",
                "no-extra-bind",
                "no-extra-label",
                "no-labels",
                "no-lonely-if",
                "no-multi-assign",
                "no-negated-condition",
                "no-nested-ternary",
                "no-new",
                "no-new-wrappers",
                "no-octal",
                "no-octal-escape",
                "no-plusplus",
                "no-proto",
                "no-restricted-properties",
                "no-return-assign",
                "no-script-url",
                "no-sequences",
                "no-ternary",
                "no-throw-literal",
                "no-unused-labels",
                "no-useless-call",
                "no-useless-catch",
                "sort-keys",
                "default-case",
                "default-case-last",
                "require-yield",
                "no-multi-str",
                "no-mixed-operators",
                "no-empty-pattern",
                "no-constructor-return",
                "complexity",
                "consistent-return",
                "getter-return",
                "no-unreachable",
                "no-fallthrough",
                "no-useless-return",
                "no-self-assign",
                "constructor-super",
                "no-unreachable-loop",
                "array-callback-return",
                "no-this-before-super",
                "no-unsafe-finally",
                "no-unsafe-negation",
                "no-unsafe-optional-chaining",
                "yield-star-spacing",
                "array-bracket-newline",
                "space-unary-ops",
                "no-const-assign",
                "no-class-assign",
                "no-ex-assign",
                "no-func-assign",
                "no-import-assign",
                "no-new-object",
                "no-param-reassign",
                "wrap-regex",
                "dot-location",
                "symbol-description",
                "no-constant-binary-expression",
                "no-constant-condition",
                "no-dupe-args",
                "yoda",
                "vars-on-top",
                "max-statements",
                "prefer-object-has-own",
                "line-comment-position",
                "guard-for-in",
                "no-inner-declarations",
                "no-undef",
                "accessor-pairs",
                "no-unused-vars",
                "no-duplicate-imports",
                "no-new-native-nonconstructor",
                "no-new-symbol",
                "no-empty-character-class",
                "no-control-regex",
                "no-regex-spaces",
                "no-invalid-regexp",
                "no-useless-escape",
                "class-methods-use-this",
                "default-param-last",
            ]
            .into_iter()
            .map(|rule_name| {
                (
                    format!("eslint-builtin/{rule_name}"),
                    RuleConfigurationValueBuilder::default()
                        .level(ErrorLevel::Error)
                        .build()
                        .unwrap(),
                )
            })
            .collect::<HashMap<_, _>>(),
        )
        .build()
        .unwrap()
}
