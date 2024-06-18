use crossbeam_channel::{unbounded, Receiver, Sender};
use rayon::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Result, Write};
use std::path::{Path, PathBuf};

// TEST(mhs):
const REGEX: &str = r"(?m)(\/\/|\#)(\s)*[A-Z]+(\([a-zA-Z0-9]+\))?:(\s)*";
const REGEX_SPLIT_CHARS: [char; 3] = ['(', ')', ':'];
const COMMENT_SPACE_CHARS: [char; 3] = ['/', '#', ' '];

const SOURCE_EXT: [Option<&str>; 1] = [
    Some("rs"),
    //
];

fn main() -> Result<()> {
    let count = env::args().len();
    match count {
        1 => run()?,
        _ => {
            println!("Error: Wrong number of arguments provided.");
            println!("Usage: git-todos");
        }
    }
    Ok(())
}

/// A TodoItem contains the following infomation of a todo comment
/// "KEYWORD(name):"
#[derive(Debug)]
struct TodoItem {
    pub keyword: Keyword,
    pub name: Name,
    pub file_path: PathBuf,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Keyword(String);
impl Keyword {
    pub fn new(k: &str) -> Self {
        Self(k.to_uppercase())
    }
}

#[derive(Debug)]
struct Name(String);
impl Name {
    pub fn new(n: &str) -> Self {
        Self(n.to_lowercase())
    }
}

fn run() -> Result<()> {
    let mut todos_path = env::current_dir()?;

    if !todos_path.is_dir() {
        todos_path.pop();
    }

    // check in we are in a .git folder
    let mut git_folder = todos_path.clone();
    git_folder.push(".git");

    if !git_folder.exists() {
        println!("git-todos must be called inside a git repo.");
        return Ok(());
    }
    git_folder.pop();

    todos_path.push("TODOS");
    todos_path = todos_path.with_extension("md");

    let todos_file = std::fs::File::create(todos_path)?;

    let regex = Regex::new(REGEX).unwrap();

    let (tx, rx) = unbounded();

    run_path(&git_folder, tx, &regex)?;
    collect_todos(&git_folder, todos_file, rx)?;
    Ok(())
}

fn run_path(path: &Path, tx: Sender<TodoItem>, regex: &Regex) -> Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry_path = entry?.path();
            rayon::scope(|_| -> Result<()> { run_path(&entry_path, tx.clone(), regex) })?;
        }
    } else if let Some(path_ext) = path.extension() {
        if !SOURCE_EXT.contains(&path_ext.to_str()) {
            return Ok(());
        }

        let file_content = std::fs::read_to_string(path)?;

        rayon::scope(|_| -> Result<()> { search_file(&path, &file_content, tx, regex) })?;
    }
    Ok(())
}

fn search_file(
    file_path: &Path,
    file_content: &str,
    tx: Sender<TodoItem>,
    regex: &Regex,
) -> Result<()> {
    file_content
        .lines()
        .enumerate()
        .par_bridge()
        .for_each(|(line_number, line_content)| {
            if let Some(rgx) = regex.find(line_content) {
                let rgx_str = rgx.as_str();

                let split_item: Vec<&str> = rgx_str
                    .split(REGEX_SPLIT_CHARS)
                    .filter(|i| !i.is_empty())
                    .collect();

                let collect = split_item[0]
                    .rsplit(COMMENT_SPACE_CHARS)
                    .filter(|i| !i.is_empty())
                    .collect::<Vec<_>>();

                let todo_item = TodoItem {
                    keyword: Keyword::new(collect[0]),
                    name: Name::new(split_item[1]),
                    file_path: file_path.to_owned(),
                    line: line_number,
                    message: line_content[rgx.end()..].to_string(),
                };

                let _ = tx.send(todo_item);
            }
        });

    Ok(())
}

fn collect_todos(git_folder: &Path, file: File, rx: Receiver<TodoItem>) -> Result<()> {
    let mut todos: HashMap<Keyword, Vec<TodoItem>> = HashMap::new();

    while let Ok(todo) = rx.recv() {
        if let Some(entry) = todos.get_mut(&todo.keyword) {
            entry.push(todo);
        } else {
            todos.insert(todo.keyword.clone(), [todo].into());
        }
    }

    write_todos(git_folder, todos, file)?;

    Ok(())
}

fn write_todos(
    git_folder: &Path,
    todos: HashMap<Keyword, Vec<TodoItem>>,
    mut todos_file: File,
) -> Result<()> {
    writeln!(todos_file, "# TODOS")?;
    writeln!(todos_file)?;

    for (todo_keyword, todos_list) in todos {
        writeln!(todos_file, "## {}", todo_keyword.0)?;
        writeln!(todos_file)?;

        for item in todos_list {
            let TodoItem {
                name,
                file_path,
                line,
                message,
                ..
            } = item;

            let rel_path = file_path
                .strip_prefix(git_folder)
                .expect("The provided file should be inside the git repo.")
                .to_str()
                .unwrap()
                .to_owned();

            // [here](src/main.rs#L13)
            writeln!(
                todos_file,
                " - [{rel_path}#L{line}]({rel_path}#L{line}) @{}:  {message}",
                name.0
            )?;
        }

        writeln!(todos_file)?;
    }

    Ok(())
}
