# TFS Reference

Environment-agnostic notes for `tf.exe` (TFVC). See [SKILL.md](SKILL.md) for the check-in workflow.

## Resolve tf.exe

1. `Get-Command tf.exe` (Developer PowerShell)
2. Visual Studio install path:

```
<VS Root>\Common7\IDE\CommonExtensions\Microsoft\TeamFoundation\Team Explorer\TF.exe
```

Common VS roots: `2022\Enterprise`, `2022\Professional`, `2022\Community`, `2019\*`.

## Collection URL

Shape: `http(s)://<host>[:port]/tfs/<CollectionName>`

Sources (in order):
1. User request
2. Project docs in the current repo
3. `workfold` / `workspaces` output

The collection URL is **not** the same as a team project path (`$/ProjectName/...`). Use the collection root for `/collection:` switches.

## Workspace and mappings

| Command | Purpose |
|---------|---------|
| `tf workspaces /collection:$collection /owner:*` | List workspaces on a collection |
| `tf workfold` | Show workspace name and local ↔ server mappings for cwd |
| `tf dir "<ServerPath>" /recursive` | List server items under a path |

When cwd is inside a mapped folder, most commands infer workspace automatically.

## Authentication

Default: Windows integrated auth or cached TFS credentials. Avoid `/login` unless the user explicitly needs it.

## Common tf.exe flags

| Flag | Purpose |
|------|---------|
| `/recursive` | Include subfolders |
| `/comment:"..."` | Check-in comment (required for checkin) |
| `/noprompt` | Non-interactive; fail instead of prompting |
| `/collection:$collection` | Target collection (workspaces, some server queries) |

## Server vs local paths

- Server paths start with `$/` (example: `$/MyTeamProject/src`)
- Local paths are normal filesystem paths mapped via the workspace
- `workfold` links the two for the current directory

## Troubleshooting

| Symptom | Action |
|---------|--------|
| "Not mapped" / no workspace | Run `workfold`; cd into a mapped folder or map with `tf workfold` |
| Wrong collection | Confirm collection URL; list workspaces with `/collection:` |
| Check-in blocked by get | Run `get` on the same scope, resolve conflicts, retry |
| tf.exe not found | Use Developer PowerShell or install VS with Team Explorer |
