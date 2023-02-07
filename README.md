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

## Examples

```
$ space new double
Created new project `double`
$ cd double/
$ nano src/lib.rs
$ cat src/lib.rs
#[no_mangle]
fn main(input: f32) -> f32 {
    input * 2.0
}
$ space upload
   Compiling double v0.1.0 (/tmp/tmp.8HxDL3jeMl/double)
    Finished release [optimized] target(s) in 0.21s
Name: Double
Version: 0.1
Description: Takes f32 and doubles
Input: input -> f32
Output: output -> f32
Finished uploading Double@0.1!
```
