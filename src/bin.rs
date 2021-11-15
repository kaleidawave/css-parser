use std::{fs, path::PathBuf};

use argh::FromArgs;
use codespan_reporting::{
    diagnostic::{Diagnostic, Label},
    files::SimpleFiles,
    term::{
        emit,
        termcolor::{ColorChoice, StandardStream},
    },
};
use css_parser::{raise_nested_rules, ParseError, StyleSheet, ToStringSettings};

#[derive(FromArgs, Debug)]
/// A css parser/compiler
struct TopLevel {
    #[argh(subcommand)]
    nested: CSSParserSubCommand,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
enum CSSParserSubCommand {
    Info(Info),
    Build(BuildArguments),
}

/// Display info
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "info")]
struct Info {}

/// Build arguments
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "build")]
struct BuildArguments {
    /// path to input file
    #[argh(positional)]
    input: PathBuf,
    /// path to output
    #[argh(positional)]
    output: PathBuf,

    /// whether to minify build output
    #[argh(switch, short = 'm')]
    minify: bool,
    /// build source maps
    #[argh(switch)]
    source_maps: bool,
}

fn main() {
    let args: TopLevel = argh::from_env();
    match args.nested {
        CSSParserSubCommand::Info(_) => {
            println!("CSS Parser: CSS and SCSS compiler");
            println!("   Version: {}", env!("CARGO_PKG_VERSION"));
            println!("Repository: {}", env!("CARGO_PKG_REPOSITORY"));
        }
        CSSParserSubCommand::Build(build) => {
            let res = StyleSheet::from_path(build.input);
            match res {
                Ok(mut stylesheet) => {
                    raise_nested_rules(&mut stylesheet);

                    let settings = if build.minify {
                        ToStringSettings::minified()
                    } else {
                        ToStringSettings::default()
                    };

                    let output = if build.source_maps {
                        let (output, source_map) =
                            stylesheet.to_string_with_source_map(Some(settings));
                        let prefix = "sourceMappingURL=data:application/json;base64,";
                        // Append inline comment
                        format!("{}\n/*# {}{}*/", output, prefix, base64::encode(source_map))
                    } else {
                        stylesheet.to_string(Some(settings))
                    };
                    fs::write(build.output.as_path(), output).unwrap();
                    println!("Wrote '{}'", build.output.display());
                }
                Err(err) => {
                    let ParseError { position, reason } = err;

                    let mut files = SimpleFiles::new();
                    let (filename, file_content) = position.source_id.get_file().unwrap();
                    let file_id = files.add(filename.to_str().unwrap().to_owned(), file_content);

                    let diagnostic = Diagnostic::error()
                        .with_labels(vec![Label::primary(file_id, position).with_message(&reason)]);

                    let writer = StandardStream::stderr(ColorChoice::Always);
                    let config = codespan_reporting::term::Config::default();

                    emit(&mut writer.lock(), &config, &files, &diagnostic).unwrap();
                }
            }
        }
    }
}
