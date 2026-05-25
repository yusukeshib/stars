---
name: semble-search
description: Semantic code search for this repository using MinishLab Semble. Use when finding code by intent, locating implementations, understanding how something works, or discovering related code. Prefer over grep/read for semantic or exploratory repo questions.
allowed-tools:
  - Bash
  - Read
---

# Semble Search

Use [MinishLab Semble](https://github.com/MinishLab/semble) for fast semantic code search in this repo.

## Command

Use the GitHub version so pi gets the current CLI documented upstream (`index`, `--index`, and `--content` are not in PyPI 0.2.0 yet):

```bash
uvx --from 'semble @ git+https://github.com/MinishLab/semble.git' semble <command>
```

If a future PyPI release contains the same commands, plain `semble` or `uvx --from semble semble` is also fine.

## Workflow for this repo

1. For repeated searches, create/recreate the local ignored index:

```bash
uvx --from 'semble @ git+https://github.com/MinishLab/semble.git' semble index . -o .semble-index --include-text-files
```

2. Search by natural language or symbol name:

```bash
uvx --from 'semble @ git+https://github.com/MinishLab/semble.git' semble search "star catalog loading" --index .semble-index --top-k 8
```

3. Search without a prebuilt index for one-off queries:

```bash
uvx --from 'semble @ git+https://github.com/MinishLab/semble.git' semble search "camera controls" . --top-k 5 --content all
```

4. Find related code from a previous result:

```bash
uvx --from 'semble @ git+https://github.com/MinishLab/semble.git' semble find-related <file_path> <line> --index .semble-index --top-k 8
```

## Rules

- Use Semble before `grep`/`rg` for exploratory or semantic code questions.
- Use `rg` when an exhaustive literal match is required.
- Rebuild `.semble-index` after substantial code changes or if results look stale.
- Inspect full files with `read` only when the returned snippet is insufficient.
