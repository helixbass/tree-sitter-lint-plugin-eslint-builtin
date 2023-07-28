use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn for_direction_rule() -> Arc<dyn Rule> {
    rule! {
        name => "for-direction",
        languages => [Javascript],
        listeners => [
            r#"(
              (for_statement
                condition: (expression_statement
                  (binary_expression
                    left: (identifier) @counter
                    operator: [
                      "<"
                      "<="
                      ">"
                      ">="
                    ] @operator
                  )
                )
                increment: [
                  (update_expression
                    argument: (identifier) @update_name (#eq? @counter @update_name)
                    operator: [
                      "++"
                      "--"
                    ] @update_operator
                  )
                  (augmented_assignment_expression
                    left: (identifier) @update_name (#eq? @counter @update_name)
                    operator: [
                      "+="
                      "-="
                    ] @update_operator
                    right: [
                      (number)
                      (unary_expression
                        argument: (number)
                        operator: "-"
                      ) @update_right_reversed
                    ]
                  )
                ]
              ) @for_statement
            )"# => |captures, context| {
                #[derive(Copy, Clone, Debug, PartialEq, Eq)]
                enum Direction {
                    Decreasing,
                    Increasing
                }

                impl Direction {
                    pub fn reversed(self) -> Self {
                        match self {
                            Self::Decreasing => Self::Increasing,
                            Self::Increasing => Self::Decreasing,
                        }
                    }
                }

                let wrong_direction = match context.get_node_text(captures["operator"]) {
                    "<" | "<=" => Direction::Decreasing,
                    _ => Direction::Increasing,
                };

                let reverse_if_negated = |direction: Direction| {
                    if captures.get("update_right_reversed").is_some() {
                        direction
                    } else {
                        direction.reversed()
                    }
                };

                if match context.get_node_text(captures["update_operator"]) {
                    "++" => Direction::Increasing,
                    "--" => Direction::Decreasing,
                    "+=" => reverse_if_negated(Direction::Increasing),
                    "-=" => reverse_if_negated(Direction::Decreasing),
                    _ => unreachable!(),
                } == wrong_direction {
                    context.report(violation! {
                        node => captures["for_statement"],
                        message => "The update clause in this loop moves the variable in the wrong direction."
                    });
                }
            },
        ]
    }
}
