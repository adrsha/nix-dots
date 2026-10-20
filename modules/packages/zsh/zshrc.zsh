# ================= Environment =================
export RUST_BACKTRACE=full
export FZF_DEFAULT_OPTS="--height=80% --layout=reverse --info=inline --border --margin=1 --padding=1 --wrap --gap=1 --no-separator --pointer=✦ --color='gutter:-1,fg+:2,fg:7'"
(( $+commands[bat] )) && export MANPAGER="bat --color always --tabs 4 --theme ansi -l md --wrap auto"

FLAKE_DIR="${FLAKE_DIR:-$HOME/flakes}"
FLAKE_HOST="${FLAKE_HOST:-chilly}"

typeset -U path PATH
path=( $HOME/Scripts(N-/) $HOME/.local/bin(N-/) $path )

# ================= Shell options =================
HISTFILE="${XDG_STATE_HOME:-$HOME/.local/state}/zsh/history"
[[ -d ${HISTFILE:h} ]] || mkdir -p -- "${HISTFILE:h}"
HISTSIZE=50000
SAVEHIST=50000

unsetopt BEEP
setopt HIST_IGNORE_DUPS HIST_IGNORE_SPACE SHARE_HISTORY INTERACTIVE_COMMENTS AUTO_CD

# ================= Vi mode =================
bindkey -v
KEYTIMEOUT=20
bindkey -M viins 'jk' vi-cmd-mode

_set_cursor() {
    case $KEYMAP in
        vicmd|visual) print -n '\e[2 q' ;;
        *)            print -n '\e[6 q' ;;
    esac
}
zle-keymap-select() { _set_cursor }
zle-line-init()     { zle -K viins; _set_cursor }
zle -N zle-keymap-select
zle -N zle-line-init

# ================= Prompt =================
PROMPT='%F{blue}%~%f %(?.%F{green}.%F{red})❯%f '

# ================= Plugins =================
plugins=(
    zsh-users/zsh-completions
)

ZINIT_HOME="${XDG_DATA_HOME:-$HOME/.local/share}/zinit/zinit.git"
if [[ ! -r $ZINIT_HOME/zinit.zsh ]] && (( $+commands[git] )); then
    mkdir -p -- "${ZINIT_HOME:h}" \
        && git clone --depth 1 https://github.com/zdharma-continuum/zinit.git "$ZINIT_HOME"
fi

if [[ -r $ZINIT_HOME/zinit.zsh ]]; then
    source "$ZINIT_HOME/zinit.zsh"

    zinit wait lucid blockf for "${plugins[@]}"

    # Order matters: syntax highlighting goes last and runs compinit.
    zinit wait lucid for \
        atload"
            bindkey -M vicmd 'k' history-substring-search-up
            bindkey -M vicmd 'j' history-substring-search-down
            bindkey -M viins '^[[A' history-substring-search-up
            bindkey -M viins '^[[B' history-substring-search-down
        " \
            zsh-users/zsh-history-substring-search \
        atinit"ZINIT[COMPINIT_OPTS]=-C; zicompinit; zicdreplay" \
            zdharma-continuum/fast-syntax-highlighting
else
    print -u2 "zsh: zinit unavailable, plugins skipped"
    autoload -Uz compinit && compinit
fi

# ================= Tool integrations =================
export DEJA_CYCLE_KEY='^L'
if (( $+commands[deja] )); then
    _deja_init="${XDG_DATA_HOME:-$HOME/.local/share}/deja/init.zsh"
    if [[ -r $_deja_init ]]; then
        source "$_deja_init"
    else
        eval "$(deja init zsh)"
    fi
    unset _deja_init
fi

(( $+commands[fzf] ))    && source <(fzf --zsh)
(( $+commands[zoxide] )) && eval "$(zoxide init zsh --cmd c)"

# ================= Aliases =================
if (( $+commands[lsd] )); then
    alias ls='lsd' l='lsd -L' la='lsd -A' lr='lsd -R'
    alias ll='lsd -lA --date relative --sort git'
    alias lf='lsd --tree --depth=1' lt='lsd --tree --depth=2'
else
    alias la='ls -A' ll='ls -lA'
fi

alias cp='cp -ir' mv='mv -i' mkdir='mkdir -p'
alias bat='bat --theme=ansi'
alias n='nvim'
alias t='nvim ~/Notes/index.md'
alias o="$HOME/Scripts/launch"
alias calc="$HOME/Codes/rumat/target/release/rustCalc"
alias mi='matugen --contrast -1 image'
alias pyv='source ./bin/activate'
alias pubip='curl -4 ifconfig.me | wl-copy'
alias fs='sudo du -h -d 2 | sort -rh'
alias dfs='df -h -x squashfs -x tmpfs -x devtmpfs'

# Config shortcuts
alias mc='nvim "$FLAKE_DIR/modules/packages/mango/mango.conf"'
alias zc='nvim "$FLAKE_DIR/modules/packages/zsh/zshrc.zsh"'
alias pc='nvim "$FLAKE_DIR/modules/system/packages.nix"'
alias kc='nvim "$FLAKE_DIR/modules/packages/kitty/kitty.conf"'
alias xc='nvim "$FLAKE_DIR/"'
alias ns='nix-shell --command zsh'

# ================= Functions: navigation =================
f() {
    local file
    file=$( (fd --type f 2>/dev/null || find . -type f) | fzf ) || return
    [[ -n $file ]] && nvim -- "$file"
}

gt() {
    local dir
    dir=$(fd --type d --hidden --follow 2>/dev/null | fzf) || return
    [[ -n $dir ]] && cd -- "$dir"
}

tk() {
    [[ -n $1 ]] || { print -u2 "usage: tk <dir>"; return 1 }
    mkdir -p -- "$1" && cd -- "$1"
}

# ================= Functions: files =================
# Delete everything in the current directory.
#   -p / --preview      list what will be deleted and ask first
#   -x / --exclude PAT  keep entries matching PAT (repeatable)
clrdir() {
    local preview=0 item pattern skip reply
    local -a excludes items

    while (( $# )); do
        case $1 in
            -p|--preview) preview=1 ;;
            -x|--exclude)
                [[ -n $2 ]] || { print -u2 "clrdir: $1 needs a pattern"; return 1 }
                excludes+=( "$2" ); shift ;;
            *) print -u2 "clrdir: unknown option: $1"; return 1 ;;
        esac
        shift
    done

    if [[ $PWD == "/" || $PWD == "$HOME" ]]; then
        print -u2 "clrdir: refusing to run in $PWD"
        return 1
    fi

    for item in *(DN); do
        skip=0
        for pattern in $excludes; do
            [[ $item == ${~pattern} ]] && { skip=1; break }
        done
        (( skip )) || items+=( "$item" )
    done

    (( ${#items} )) || { print "📁 Nothing to delete"; return 0 }

    if (( preview )); then
        print "📋 Items to be deleted:"
        printf '  %s\n' "${items[@]}"
        read -q "reply?Delete? [y/N] " || { print; print "Aborted"; return 1 }
        print
    fi

    if rm -rf -- "${items[@]}"; then
        print "✅ Deleted ${#items} item(s)"
    else
        print -u2 "❌ Some items could not be deleted"; return 1
    fi
}

# ================= Functions: git =================
_push_with_token() {
    local token_file=$1 branch
    [[ -r $token_file ]] || { print -u2 "token file not readable: $token_file"; return 1 }
    branch=$(git branch --show-current) && [[ -n $branch ]] \
        || { print -u2 "not on a branch"; return 1 }
    wl-copy < "$token_file" && git push origin "$branch"
}
glpush() { _push_with_token "$HOME/Documents/gitlabtoken" }
ghpush() { _push_with_token "$HOME/Documents/githubtoken" }

# ================= Functions: theme =================
theme() {
    [[ -n $1 ]] || { print -u2 "usage: theme <name>"; return 1 }
    command theme-set "$1" || return 1
    [[ -r $HOME/.config/zsh/theme_vars.zsh ]] && source "$HOME/.config/zsh/theme_vars.zsh"
}

# ================= NixOS =================
# Flakes only see git-tracked files, so stage everything first.
_flake_stage() {
    [[ -d $FLAKE_DIR/.git ]] || { print -u2 "not a git repo: $FLAKE_DIR"; return 1 }
    git -C "$FLAKE_DIR" add -A
}

nixr() {
    _flake_stage || return 1
    sudo nixos-rebuild switch --flake "$FLAKE_DIR#$FLAKE_HOST"
}

# Quick reload: no network, no lock update.
qnixr() {
    _flake_stage || return 1
    print "Changed since last commit:"
    git -C "$FLAKE_DIR" --no-pager diff --cached --name-only HEAD || return 1
    sudo nixos-rebuild switch \
        --flake "$FLAKE_DIR#$FLAKE_HOST" \
        --no-update-lock-file
}

nix-cleanup() {
    emulate -L zsh
    setopt pipefail

    print "==> Removing old generations"
    sudo nix-env --profile /nix/var/nix/profiles/system --delete-generations old || return 1

    local hm_profile="$HOME/.local/state/nix/profiles/home-manager"
    [[ -e $hm_profile ]] && nix-env --profile "$hm_profile" --delete-generations old

    print "==> Garbage collecting"
    sudo nix-collect-garbage --delete-old || return 1
    nix-collect-garbage --delete-old

    print "==> Cleaning disposable caches (older than 30 days)"
    local dir
    for dir in "$HOME/.cache/thumbnails" "$HOME/.cache/fontconfig"; do
        [[ -d $dir ]] && find "$dir" -type f -atime +30 -delete 2>/dev/null
    done

    print "==> Done"
}

eval "$(starship init zsh)"
