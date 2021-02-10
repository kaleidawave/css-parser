use css_parser::{raise_rules, ASTNode, StyleSheet, ToStringSettings};
use std::{env, path::PathBuf, fs};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        println!(
            r#"Info:
{} {}
Usage:
./css-parser index.css out.css
Flags:
--source-map Builds a source map
--minify     Minifies output"#,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    let minify = args.contains(&"--minify".to_owned());
    let generate_source_map = args.contains(&"--source-map".to_owned());
    let current_dir: PathBuf = env::current_dir().unwrap().into();
    let source_file = current_dir.join(args.get(1).unwrap());
    let output_file = current_dir.join(args.get(2).unwrap());

    let mut style_sheet = StyleSheet::from_path(&source_file, args.get(1).unwrap()).unwrap();
    raise_rules(&mut style_sheet);
    let stringify_settings = ToStringSettings {
        minify, generate_source_map, indent_with: "    ".to_owned()
    };
    let (mut output, source_map) = style_sheet.to_string(&stringify_settings);

    if let Some(source_map) = source_map {
        // Add the source map comment:
        output.push_str(&format!("/*# sourceMappingURL={}.map*/", args.get(2).unwrap()));
        fs::write(output_file.with_extension("css.map"), source_map.to_string()).unwrap();
    }
    fs::write(output_file, output).unwrap();
    println!("Compiled out {} to {}", args.get(1).unwrap(), args.get(2).unwrap());
}
