use std::{cell::RefCell, env, fs, sync::Arc};

use clap::{ArgGroup, Parser, Subcommand};

use tree_sitter_lint::{rule, ConfigBuilder, ErrorLevel, Rule, RuleConfiguration};
use tree_sitter_lint_plugin_eslint_builtin::{
    CodePathAnalyzer, CodePathAnalyzerInstanceProviderFactory,
};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    DumpDotFile(DumpDotFileArgs),
}

#[derive(clap::Args)]
#[clap(group(
    ArgGroup::new("source")
        .multiple(false)
        .required(true)
        .args(&["source_text", "path"])
))]
struct DumpDotFileArgs {
    source_text: Option<String>,
    #[arg(long)]
    path: Option<String>,
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::DumpDotFile(args) => {
            dump_dot_file(&match (args.source_text, args.path) {
                (Some(source_text), None) => source_text,
                (None, Some(path)) => fs::read_to_string(path).unwrap(),
                _ => unreachable!(),
            });
        }
    }
}

fn dump_dot_file(source_text: &str) {
    env::set_var("DEBUG_CODE_PATH", "1");

    thread_local! {
        static ACTUAL: RefCell<Vec<String>> = Default::default();
    }

    let rule: Arc<dyn Rule> = rule! {
        name => "testing-code-path-analyzer-paths",
        languages => [Javascript],
        listeners => [
            r#"
              (program) @c
            "# => |node, context| {
                context.retrieve::<CodePathAnalyzer<'a>>();
            },
        ],
    };

    tree_sitter_lint::run_for_slice(
        source_text.as_bytes(),
        None,
        "tmp.js",
        ConfigBuilder::default()
            .rule(rule.meta().name.clone())
            .all_standalone_rules([rule.clone()])
            .rule_configurations([RuleConfiguration {
                name: rule.meta().name.clone(),
                level: ErrorLevel::Error,
                options: None,
            }])
            .build()
            .unwrap(),
        tree_sitter_lint::tree_sitter_grep::SupportedLanguage::Javascript,
        &CodePathAnalyzerInstanceProviderFactory,
    );
}
