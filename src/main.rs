use std::error::Error;
use std::fs;
use tree_sitter::Language;

extern "C" {
    fn tree_sitter_go() -> Language;
}

fn main() -> Result<(), Box<dyn Error>> {
    // Invoke the top-level C function from the Tree-sitter Go library.
    // Unsafe, because the C FFI is unsafe.
    let language = unsafe { tree_sitter_go() };

    // Create a tree-sitter parser.
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(language).unwrap();

    // Parse a file.
    let source_code = fs::read_to_string("example.go")?;
    let _tree = parser.parse(&source_code, None).unwrap();

    Ok(())
}
