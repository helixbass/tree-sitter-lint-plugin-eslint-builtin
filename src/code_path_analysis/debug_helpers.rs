use std::collections::HashSet;

use id_arena::{Arena, Id};
use squalid::continue_if_none;

use super::{code_path::CodePath, code_path_segment::CodePathSegment};

// pub fn dump_state(arena: &Arena<CodePathSegment>, node: Node, state: &CodePathState, leaving: bool) {
//     for &current_segment in &state.current_segments {

//     }
// }

pub fn make_dot_arrows(
    code_path_segment_arena: &Arena<CodePathSegment>,
    code_path: &CodePath, /*, traceMap*/
) -> String {
    let mut stack = vec![(code_path.initial_segment(), 0)];
    let mut done: HashSet<Id<CodePathSegment>> = /*traceMap ||*/ Default::default();
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
