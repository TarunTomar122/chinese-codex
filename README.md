<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you want the desktop app experience, run <code>codex app</code> or visit <a href="https://chatgpt.com/codex?app-landing-page=true">the Codex App page</a>.
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---

> **This is a fork of Codex CLI with native AI subagent dispatch.**  
> GPT orchestrates — DeepSeek (opencode) and Cursor do the actual code work.  
> Each subscription pays for its own work, not your codex quota.

### What's changed

Three new native tools registered in codex's tool system:

| Tool | Backend | Model | For |
|------|---------|-------|-----|
| `build` | opencode | deepseek-v4-pro | Backend: API, DB, logic, refactoring |
| `build_frontend` | opencode | deepseek-v4-flash | Simple frontend: CSS fixes, small components |
| `build_frontend_advanced` | cursor | composer-2.5-fast | Complex frontend: full pages, redesigns |

Features: session reuse across calls, live cost tracking from `opencode db`, configurable models via env vars, MCP timeout bumped to 600s.

### Build from source

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # install Rust
cd codex-rs
cargo build -p codex-cli        # debug (~7 min)
cargo build --release -p codex-cli  # release (~25 min)
```

### Usage

```bash
./target/debug/codex                           # TUI mode
./target/debug/codex exec "build a login API"  # one-shot

# Custom models per tool
CODEX_SWARM_BUILD_MODEL=opencode-go/some-model \
CODEX_SWARM_FRONTEND_ADVANCED_MODEL=composer-2.5-fast \
./target/debug/codex
```

### When to delegate

The orchestrator auto-decides: **≤2 files → DIY**, **3+ files → delegate to swarm**.  
Benchmarked: subagents win at ~200k tokens / 4+ files (30% faster, 12% cheaper).

### Subscriptions used

| Subscription | Model | Role |
|-------------|-------|------|
| Codex | GPT-5.x | Orchestrator |
| Opencode Go | deepseek-v4-pro/flash | Backend + simple frontend |
| Cursor | composer-2.5-fast | Complex frontend |

### Source changes

| File | Change |
|------|--------|
| `codex-rs/codex-mcp/src/rmcp_client.rs:75` | Timeout 120s → 600s |
| `codex-rs/core/src/tools/handlers/swarm_subagents.rs` | New: 3 subagent tools |
| `codex-rs/core/src/tools/handlers/mod.rs` | Module declaration |
| `codex-rs/core/src/tools/spec_plan.rs` | Tool registration |

---

## Quickstart

### Installing and running Codex CLI

Run the following on Mac or Linux to install Codex CLI:

```shell
curl -fsSL https://chatgpt.com/codex/install.sh | sh
```

Run the following on Windows to install Codex CLI:

```
powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 | iex"
```

Codex CLI can also be installed via the following package managers:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Business, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
