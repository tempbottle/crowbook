use clap::{App, Arg, AppSettings, Format, ArgMatches};
use std::io::{self, Write};
use std::process::exit;
use std::fs;
use crowbook::Book;

/// Prints an error on stderr and exit the program
pub fn print_error(s: &str) -> ! {
    writeln!(&mut io::stderr(), "{} {}", Format::Error("error:"), s).unwrap();
    exit(0);
}

/// sets the book options according to command line arguments
/// Also print these options to a string, so it can be used at
/// the creation of a book to check that parameters are OK and t
/// then print them to file
pub fn set_book_options(book: &mut Book, matches: &ArgMatches) -> String {
    let mut output = String::new();
    if let Some(iter) = matches.values_of("set") {
        let v:Vec<_> = iter.collect();
        if v.len() %2 != 0 {
            print_error("An odd number of arguments was passed to --set, but it takes a list of key value pairs.");
        }

        for i in 0..v.len()/2 {
            let key = v[i * 2];
            let value = v[i * 2 + 1];
            let res = book.set_option(key, value);
            if let Err(err) = res {
                print_error(&format!("Error in setting key {}: {}", key, err));
            }
            output.push_str(&format!("{}: {}\n", key, value));
        }
    }
    output
}

/// create a book file with the command line arguments
/// and exit the process at the end
pub fn create_book(matches: &ArgMatches) -> ! {
    if let Some(values) = matches.values_of("FILES") {
        let s = matches.value_of("BOOK").unwrap();
        if fs::metadata(s).is_ok() {
            print_error(&format!("Could not create file {}: it already exists!", s));
        }

        let mut f = fs::File::create(s).unwrap();

        if matches.is_present("set") {
            let mut book = Book::new();
            let s = set_book_options(&mut book, matches);
            f.write_all(&s.as_bytes()).unwrap();
        } else {
            f.write_all(b"author: Your name
title: Your title
lang: en

# Uncomment and fill to generate files
# output.html: some_file.html
# output.epub: some_file.epub
# output.pdf: some_file.pdf

# Uncomment and fill to set cover image (for Epub)
# cover: some_cover.png\n").unwrap();
        }
        f.write_all(b"\n# List of chapters\n").unwrap();
        for file in values {
            f.write_all(&format!("+ {}\n", file).as_bytes()).unwrap();
        }
        println!("Created {}, now you'll have to complete it!", s);
        exit(0);
    } else {
        print_error("--create must be used with a list of additonal files");
    }
}

pub fn create_matches<'a>() -> ArgMatches<'a> {
    let app = App::new("crowbook")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::UnifiedHelpMessage)
        .version(env!("CARGO_PKG_VERSION"))
        .about("Render a markdown book in Epub, PDF or HTML.")
        .after_help("Command line options allow to override options defined in <BOOK> configuration file. 
E.g., even if this file specifies 'verbose: false', calling 'crowbook --verbose <BOOK>' 
will activate verbose mode.

Note that Crowbook generates output files relatively to the directory where <BOOK> is:
$ crowbook foo/bar.book --to pdf --output baz.pdf
will thus generate baz.pdf in directory foo and not in current directory.")
        .arg_from_usage("-v, --verbose 'Activate verbose mode'")
        .arg_from_usage("--create 'Creates a new book with existing markdown files.'")
        .arg(Arg::from_usage("-o, --output [FILE] 'Specifies output file.'")
             .requires("to"))
        .arg(Arg::from_usage("[FILES]... 'Files to list in book when using --create'")
             .index(2))
        .arg(Arg::from_usage("-t, --to [FORMAT] 'Generate specific format'")
             .possible_values(&["epub", "pdf", "html", "tex", "odt"]))
        .arg(Arg::from_usage("-s, --set [KEY_VALUES] 'Sets a list of book options'")
             .min_values(2))
        .arg(Arg::with_name("BOOK")
             .index(1)
             .required(true)
             .help("File containing the book configuration."));

    let matches = app.get_matches();

    pre_check(&matches);
    
    matches
}


/// Pre-check the matches to see if there isn't illegal options not detected by clap
fn pre_check(matches: &ArgMatches) {
    if matches.is_present("files") && !matches.is_present("create") {
        print_error("A list of additional files is only valid with the --create option.");
    }
}