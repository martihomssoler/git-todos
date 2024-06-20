use crossbeam_channel::{unbounded, Receiver, Sender};
use rayon::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Result, Write};
use std::path::{Path, PathBuf};

// TEST(mhs): minimum on 3 characters to be parsed
// A:
// AB:
// ABC:
const REGEX: &str = r"(?m)(\/\/|\#)(\s)*[A-Z][A-Z][A-Z]+(\([a-zA-Z0-9]+\))?:(\s)*";
const REGEX_SPLIT_CHARS: [char; 3] = ['(', ')', ':'];
const COMMENT_SPACE_CHARS: [char; 3] = ['/', '#', ' '];

const SOURCE_EXT: [&str; 1] = ["rs"];

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
    pub name: Option<Name>,
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
        let mut line = String::new();
        println!("git-todos in being called outside a git repo. Do you want to proceed? (y/n)");
        if std::io::stdin().read_line(&mut line).is_err()
            || line.to_lowercase().trim_end().eq("n")
            || line.to_lowercase().trim_end().eq("no")
        {
            return Ok(());
        }
    }
    git_folder.pop();

    todos_path.push("TODOS");
    todos_path = todos_path.with_extension("md");

    let todos_file = std::fs::File::create(todos_path)?;

    let regex = Regex::new(REGEX).unwrap();

    let (tx, rx) = unbounded();

    let _ = run_path(&git_folder, tx, &regex);
    collect_todos(&git_folder, todos_file, rx)?;
    Ok(())
}

fn run_path(path: &Path, tx: Sender<TodoItem>, regex: &Regex) -> Result<()> {
    if path.is_dir() {
        for result in ignore::Walk::new(path) {
            let Ok(entry) = result else {
                continue;
            };

            if entry.path().is_dir() {
                continue;
            }

            rayon::scope(|_| {
                let _ = search_file(entry.path(), tx.clone(), regex);
            });
        }
    } else {
        rayon::scope(|_| {
            let _ = search_file(path, tx, regex);
        });
    }
    Ok(())
}

fn search_file(path: &Path, tx: Sender<TodoItem>, regex: &Regex) -> Result<()> {
    if let Some(path_ext) = path.extension() {
        if !SOURCE_EXT.contains(&path_ext.to_str().unwrap_or_default()) {
            return Ok(());
        }

        let file_content = std::fs::read_to_string(path)?;

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

                    let filtered_items = split_item[0]
                        .rsplit(COMMENT_SPACE_CHARS)
                        .filter(|i| !i.is_empty())
                        .collect::<Vec<_>>();

                    let name = if split_item.len() > 1 && !split_item[1].trim_end().is_empty() {
                        Some(Name::new(split_item[1]))
                    } else {
                        None
                    };

                    let todo_item = TodoItem {
                        keyword: Keyword::new(filtered_items[0]),
                        name,
                        file_path: path.to_owned(),
                        line: line_number + 1,
                        message: line_content[rgx.end()..].to_string(),
                    };

                    let _ = tx.send(todo_item);
                }
            });
    }

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
    let _ = writeln!(todos_file, "# TODOS");
    let _ = writeln!(todos_file);

    for (todo_keyword, todos_list) in todos {
        let _ = writeln!(todos_file, "## {}", todo_keyword.0);
        let _ = writeln!(todos_file);

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
                .to_string()
                .replace('\\', "/");

            let _ = writeln!(
                todos_file,
                " - [{rel_path}#L{line}]({rel_path}#L{line}) {}:  {message}",
                if let Some(name) = name {
                    format!("@{}", name.0)
                } else {
                    "".to_string()
                }
            );
        }

        let _ = writeln!(todos_file);
    }

    Ok(())
}
