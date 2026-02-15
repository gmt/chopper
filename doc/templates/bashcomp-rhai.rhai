// Rhai completion script for a chopper alias.
// Place this file alongside your alias config and reference it via:
//   [bashcomp]
//   rhai_script = "completions/myalias.rhai"
//   rhai_function = "complete"    // optional, default "complete"
//
// The function receives a context map and returns an array of candidates.

fn complete(ctx) {
    // ctx.words      -- Array of strings (COMP_WORDS from bash)
    // ctx.cword      -- Integer (0-based index of word being completed)
    // ctx.current    -- String (the partial word being completed)
    // ctx.exec       -- String (resolved exec path for the alias)
    // ctx.alias_args -- Array of strings (alias's configured args)
    // ctx.alias_env  -- Map of string->string (alias's configured env)

    // Example: complete subcommands at position 1
    if ctx.cword == 1 {
        return ["start", "stop", "status", "restart"];
    }

    // Default: no completions
    []
}
