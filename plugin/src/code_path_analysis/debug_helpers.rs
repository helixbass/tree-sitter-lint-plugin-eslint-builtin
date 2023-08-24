use std::{collections::HashSet, env};

use id_arena::{Arena, Id};
use once_cell::sync::Lazy;
use squalid::{continue_if_none, NonEmpty, OptionExt};
use tree_sitter_lint::{tree_sitter::Node, NodeExt, SourceTextProvider};

use crate::kind::{self, Identifier, PropertyIdentifier};

use super::{
    code_path::CodePath,
    code_path_segment::{CodePathSegment, EnterOrExit},
    code_path_state::CodePathState,
};

static ENABLED: Lazy<bool> = Lazy::new(|| env::var("DEBUG_CODE_PATH").ok().is_non_empty());

fn enabled() -> bool {
    *ENABLED
}

fn get_id(segment: &CodePathSegment) -> String {
    format!("{}{}", segment.id, if segment.reachable { "" } else { "!" })
}

fn node_to_string<'a>(
    node: Node,
    label: Option<&str>,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> String {
    let suffix = label.map_or_default(|label| format!(":{label}"));

    let base = format!("{}{suffix}", node.kind());
    match node.kind() {
        Identifier | PropertyIdentifier => format!("{base} ({})", node.text(source_text_provider)),
        kind::String => format!("{base} ({})", node.text(source_text_provider)),
        _ => base,
    }
}

pub fn dump(message: &str) {
    if !enabled() {
        return;
    }

    eprintln!("{}", message);
}

pub fn dump_state<'a>(
    arena: &mut Arena<CodePathSegment<'a>>,
    node: Node<'a>,
    state: &CodePathState<'a>,
    leaving: bool,
) {
    for current_segment in state
        .current_segments
        .as_ref()
        .map_or_default(|current_segments| current_segments.segments())
    {
        let current_segment = &mut arena[current_segment];

        let nodes = &mut current_segment.nodes;
        if leaving {
            nodes.push((EnterOrExit::Exit, node));
            // #[allow(clippy::unnecessary_lazy_evaluations)]
            // if let Some(last) = (!nodes.is_empty()).then(|| nodes.len() - 1).filter(|last| {
            //     nodes[*last] == node_to_string(node, Some("enter"), source_text_provider)
            // }) {
            //     nodes[last] = node_to_string(node, None, source_text_provider);
            // } else {
            //     nodes.push(node_to_string(node, Some("exit"), source_text_provider));
            // }
        } else {
            nodes.push((EnterOrExit::Enter, node));
            // nodes.push(node_to_string(node, Some("enter"), source_text_provider));
        }
    }

    if !enabled() {
        return;
    }

    dump(&format!(
        "{} {}{}",
        state
            .current_segments
            .as_ref()
            .map_or_default(|current_segments| current_segments.segments())
            .into_iter()
            .map(|segment| { get_id(&arena[segment]) })
            .collect::<Vec<_>>()
            .join(","),
        node.kind(),
        if leaving { ":exit" } else { "" }
    ));
}

pub fn dump_dot<'a>(
    code_path_segment_arena: &Arena<CodePathSegment>,
    code_path: &CodePath,
    source_text_provider: &impl SourceTextProvider<'a>,
) {
    if !enabled() {
        return;
    }

    let mut text = r#"
digraph {
node[shape=box,style="rounded,filled",fillcolor=white];
initial[label="",shape=circle,style=filled,fillcolor=black,width=0.25,height=0.25];
"#
    .to_owned();

    if !code_path.returned_segments().is_empty() {
        text.push_str("final[label=\"\",shape=doublecircle,style=filled,fillcolor=black,width=0.25,height=0.25];\n");
    }
    if !code_path.thrown_segments().is_empty() {
        text.push_str("thrown[label=\"✘\",shape=circle,width=0.3,height=0.3,fixedsize=true];\n");
    }

    let mut trace_map: HashSet<Id<CodePathSegment>> = Default::default();
    let arrows = make_dot_arrows(code_path_segment_arena, code_path, Some(&mut trace_map));

    for id in trace_map {
        let segment = &code_path_segment_arena[id];
        let id = &segment.id;

        text.push_str(&format!("{id}["));

        if segment.reachable {
            text.push_str(r#"label=""#);
        } else {
            text.push_str(
                "style=\"rounded,dashed,filled\",fillcolor=\"#FF9800\",label=\"<<unreachable>>\n",
            );
        }

        if !segment.nodes.is_empty() {
            text.push_str(
                &segment
                    .nodes
                    .iter()
                    .map(|(enter_or_exit, node)| {
                        node_to_string(
                            *node,
                            Some(match enter_or_exit {
                                EnterOrExit::Enter => "enter",
                                EnterOrExit::Exit => "exit",
                            }),
                            source_text_provider,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\\n"),
            );
        } else {
            text.push_str("????");
        }

        text.push_str("\"];\n");
    }

    text.push_str(&format!("{arrows}\n"));
    text.push('}');

    eprintln!("{}", text);
}

pub fn make_dot_arrows<'a>(
    code_path_segment_arena: &Arena<CodePathSegment<'a>>,
    code_path: &CodePath<'a>, /*, traceMap*/
    trace_map: Option<&mut HashSet<Id<CodePathSegment<'a>>>>,
) -> String {
    let mut stack = vec![(code_path.initial_segment(), 0)];
    let mut default_done: HashSet<Id<CodePathSegment<'a>>> = Default::default();
    let done: &mut HashSet<Id<CodePathSegment<'a>>> = trace_map.unwrap_or(&mut default_done);
    let mut last_id = Some(
        code_path_segment_arena[code_path.initial_segment()]
            .id
            .clone(),
    );
    let mut text = format!(
        "initial->{}",
        code_path_segment_arena[code_path.initial_segment()].id
    );

    while !stack.is_empty() {
        let (segment, index) = stack.pop().unwrap();

        if done.contains(&segment) && index == 0 {
            continue;
        }
        done.insert(segment);

        let next_segment = *continue_if_none!(code_path_segment_arena[segment]
            .all_next_segments
            .get(index));

        if last_id.unwrap() == code_path_segment_arena[segment].id {
            text.push_str(&format!("->{}", code_path_segment_arena[next_segment].id));
        } else {
            text.push_str(&format!(
                ";\n{}->{}",
                code_path_segment_arena[segment].id, code_path_segment_arena[next_segment].id,
            ));
        }
        last_id = Some(code_path_segment_arena[next_segment].id.clone());

        stack.insert(0, (segment, 1 + index));
        stack.push((next_segment, 0));
    }

    code_path
        .returned_segments()
        .into_iter()
        .for_each(|&final_segment| {
            if last_id.as_ref() == Some(&code_path_segment_arena[final_segment].id) {
                text.push_str("->final");
            } else {
                text.push_str(&format!(
                    ";\n{}->final",
                    code_path_segment_arena[final_segment].id
                ));
            }
            last_id = None;
        });

    code_path
        .thrown_segments()
        .into_iter()
        .for_each(|&final_segment| {
            if last_id.as_ref() == Some(&code_path_segment_arena[final_segment].id) {
                text.push_str("->thrown");
            } else {
                text.push_str(&format!(
                    ";\n{}->thrown",
                    code_path_segment_arena[final_segment].id
                ));
            }
            last_id = None;
        });

    text.push(';');
    text
}
