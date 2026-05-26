# Add this to ~/.zshrc, then run `source ~/.zshrc`

alias swarm='CODEX_SWARM_BUILD_MODEL=opencode-go/deepseek-v4-pro \
  CODEX_SWARM_FRONTEND_MODEL=opencode-go/deepseek-v4-flash \
  CODEX_SWARM_FRONTEND_ADVANCED_MODEL=composer-2.5-fast \
  /Users/ttomar/Documents/Codex/research/codex-source/codex-rs/target/debug/codex'

# Usage:
#   swarm                    → interactive TUI
#   swarm exec "do X"        → one-shot
#   codex                    → stock codex (unchanged)
