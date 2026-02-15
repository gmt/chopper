# Custom bash completion for a chopper alias.
# Place this file alongside your alias config and reference it via:
#   [bashcomp]
#   script = "completions/myalias.bash"
#
# The function name must be _chopper_bashcomp_<alias> with non-alphanumeric
# characters replaced by underscores.

_chopper_bashcomp_myalias() {
    local cur="${COMP_WORDS[$COMP_CWORD]}"

    # Example: complete with a fixed set of subcommands
    local commands="start stop status restart"
    COMPREPLY=($(compgen -W "$commands" -- "$cur"))
}
