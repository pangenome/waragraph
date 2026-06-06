# WG GitHub remote

WG worktrees in this checkout share the main repository git config at:

```text
/home/erik/waragraph/.git/config
```

The remote configuration for WG branch pushes is:

```text
origin fetch: https://github.com/chfi/waragraph
origin push:  https://github.com/pangenome/waragraph.git
```

The existing `origin` fetch URL is preserved so normal fetch and pull behavior
continues to use the upstream repository. The `origin` push URL points to the
requested fork, `pangenome/waragraph`, so `git push origin <branch>:<branch>`
targets that repository from every WG worktree.

On 2026-06-06, the local GitHub credentials authenticated as `ekg`. Both SSH and
HTTPS access to `pangenome/waragraph` failed with GitHub `Repository not found`.
The attempted non-force pushes were:

```text
git push origin wg/agent-7/add-zstd-support:wg/agent-7/add-zstd-support
git push origin wg/agent-12/fix-waragraph-sparse:wg/agent-12/fix-waragraph-sparse
git push origin wg/agent-15/validate-c4-k311:wg/agent-15/validate-c4-k311
git push origin wg/agent-18/fix-wayland-hidpi:wg/agent-18/fix-wayland-hidpi
git push origin wg/agent-21/default-waragraph-to:wg/agent-21/default-waragraph-to
git push origin wg/agent-1/quality-pass-zstd:wg/agent-1/quality-pass-zstd
```

Each HTTPS push failed with:

```text
remote: Repository not found.
fatal: repository 'https://github.com/pangenome/waragraph.git/' not found
```

The SSH probe used:

```text
git ls-remote git@github.com:pangenome/waragraph.git HEAD
```

It failed with:

```text
ERROR: Repository not found.
fatal: Could not read from remote repository.
```

No force push, history rewrite, or branch merge was performed.
