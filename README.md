# git-todos

A simple tool that scans your git repo and generates a list of action items that you did not forget at all...

Executing it inside the `git-todos` repo will generate the following file (which you can find [here](TODOS.md)).

> # TODOS
> 
> ## ABC
> 
>  - [src/main.rs#L13](src/main.rs#L13) :  
> 
> ## TEST
> 
>  - [src/main.rs#L10](src/main.rs#L10) @mhs:  minimum on 3 characters to be parsed
> 

## Installing

Using `cargo install`, installing `git-todos` is super easy. 

```fish
git clone {git_todos_url}
cargo install --path .
```

## Usage 

To use `git-todos` just navigate to your desired git repo and execute it. A file named `TODOS.md` will be created,
or modified if it already exists, and filled with a list of pending comments found in the repo. 

For now the functionality is very limited. I use Regex expressions to search and match for a desired comment. Take a 
look at [here](src/main.rs#L11) and [here](src/main.rs#L35) for the regex and the `TodoItem` respectively.

## Contributing 

Feel free to open PRs with the desired changes. I will try to respond to all of them.

## License

git-todos is licensed under the MIT license. See [LICENSE.txt](LICENSE.txt) for more details.

