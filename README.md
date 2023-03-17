# space-cli

Latest release can be found under [releases](https://github.com/space-operator/space-cli/releases/).
Otherwise install with cargo: `cargo install --git https://github.com/space-operator/space-cli`.

```
$ space
Usage: space <COMMAND>

Commands:
  login     Login by store token locally
  new       Create a new WASM project
  upload    Upload WASM project to Space Operator
  generate  Generate JSON from dialogue
  manual    Manually upload WASM, source code and json to Space Operator
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

Basic usage looks like following:

- `space login` to login
- `space new <project>` to create a new WASM project
- `space upload` to upload the WASM project to Space Operator
