# Shell completions

Generated from the CLI itself — **do not edit by hand**. To regenerate:

```sh
cargo run --bin trot -- completions bash       > completions/trot.bash
cargo run --bin trot -- completions zsh        > completions/_trot
cargo run --bin trot -- completions fish       > completions/trot.fish
cargo run --bin trot -- completions powershell > completions/_trot.ps1
cargo run --bin trot -- completions elvish     > completions/trot.elv
```

CI regenerates these and fails if they differ from what's committed, so they
can't drift from the command tree.

They ship inside every release archive so packagers can install them to system
paths. Anyone else can just run `trot completions --install`.
