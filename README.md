# space-cli

```
$ cargo install --git https://github.com/space-operator/space-cli
$ space
Usage: space <COMMAND>

Commands:
  init      Authenticate and store locally
  new       Create a new WASM project
  upload    Upload WASM project to Space Operator
  generate  Generate JSON from dialogue
  deploy    Manually deploy WASM and source code to Space Operator
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help information
```

Basic usage looks like following:

- `space init` to login
- `space new <project>` to create a new WASM project
- `space upload` to upload the WASM project to Space Operator
