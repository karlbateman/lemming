use clap::Parser;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicIsize;
use std::{error::Error, sync::Mutex};
use tree_sitter::{Language, Query, QueryCapture, QueryCursor};
use walkdir::WalkDir;

extern "C" {
    fn tree_sitter_go() -> Language;
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    path: PathBuf,
    pattern: String,
}

// A counter to track total matches found in the searched files.
static TOTAL_COUNT: AtomicIsize = AtomicIsize::new(0);

// Parse a single Go file with Tree Sitter and execute the provided query.
fn parse_file(
    path: &PathBuf,
    query: &Query,
    out: &Mutex<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Unsafe: Tree Sitter uses a C foreign function interface, so we need unsafe to access the generated C library.
    let language = unsafe { tree_sitter_go() };

    // todo(perf): We could significantly improve perf by reusing this parser object in a thread local.
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(language).unwrap();

    // Parse the file.
    let source_code = fs::read_to_string(path)?;
    let tree = parser.parse(&source_code, None).unwrap();

    let mut cursor = QueryCursor::new();
    let root_node = tree.root_node();

    let source_bytes = &*source_code.as_bytes();

    let mut seen_nodes = HashSet::new();

    for m in cursor.matches(query, root_node, source_bytes) {
        let captures: HashMap<_, _> = m
            .captures
            .iter()
            .map(|c: &QueryCapture| (query.capture_names()[c.index as usize].clone(), c))
            .collect();

        // This is the capture we added above, so we can access the root node of the query match.
        let full_capture = captures["full_pattern_cli_capture"];

        if seen_nodes.contains(&full_capture.node.id()) {
            continue; // Sometimes the same node can match multiple times, so we ignore duplicate matches.
        }
        seen_nodes.insert(full_capture.node.id());

        TOTAL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let found_text = full_capture.node.utf8_text(source_bytes).unwrap();
        let mut output = out.lock().unwrap();

        output.push(format!(
            "=================================================================\nFound [{}]\n{}",
            path.display(),
            found_text,
        ));
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // Buffer the lines written to stdout to avoid lock contention with println!().
    // todo(perf): Switch to a parallel append-only data structure for a slight perf improvement.
    let output_text = Mutex::new(Vec::new());

    let language = unsafe { tree_sitter_go() };

    let args = Args::parse();

    // Add an extra root pattern to force capturing the root pattern so it can be displayed.
    let full_pattern = format!("{} @full_pattern_cli_capture", args.pattern);

    // Query string built from the CLI input.
    let query =
        Query::new(language, &full_pattern).expect("Error building query from given string.");

    // Scan the entire directory first, so that we can more easily split the work among worker threads later.
    let mut paths = Vec::new();
    for entry in WalkDir::new(args.path).into_iter() {
        let path = entry.unwrap().into_path();
        let path_str = path.to_str().expect("filename was not valid utf-8");
        if path_str.ends_with(".go") {
            paths.push(path);
        }
    }

    println!("Searching {} files.\n", paths.len());

    // Divide the files to be searched into equal portions and send each to a worker thread, via Rayons par_iter.
    paths.par_iter().for_each(|path| {
        if let Err(e) = parse_file(&path, &query, &output_text) {
            output_text
                .lock()
                .unwrap()
                .push(format!("==> Skipping [{}] [{}]", path.display(), e));
        }
    });

    for o in output_text.lock().unwrap().iter() {
        println!("{}", o);
    }

    println!(
        "\nFound {} total results.",
        TOTAL_COUNT.load(std::sync::atomic::Ordering::SeqCst)
    );

    Ok(())
}
