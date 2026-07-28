# TFS Check-in Examples

Placeholders: `$tf`, `$collection`, `<LocalPath>`, `<ServerPath>`, `<changesetId>`. Resolve them per [SKILL.md](SKILL.md) — do not hardcode environment values.

## Solution-scoped check-in

```powershell
Set-Location "<LocalPath>"

& $tf status . /recursive

& $tf get "MyProject.sln"

& $tf checkin "MyProject.sln" /comment:"Add MyProject.sln solution file for Visual Studio" /noprompt
```

Verification:

```powershell
& $tf status . /recursive
& $tf changeset <changesetId> /noprompt
```

## Folder-scoped check-in

```powershell
Set-Location "<LocalPath>"

& $tf status . /recursive

& $tf get . /recursive

& $tf checkin . /recursive /comment:"MyArea: API updates, build scripts, and config refinements" /noprompt
```

Verification:

```powershell
& $tf status . /recursive
& $tf changeset <changesetId> /noprompt
```

## Discovery before first check-in

```powershell
& $tf workspaces /collection:$collection /owner:*

& $tf workfold

& $tf dir "<ServerPath>" /recursive
```

## Comment patterns

| Style | Example |
|-------|---------|
| New artifact | `Add MyProject.sln solution file for Visual Studio` |
| Area prefix + summary | `MyArea: API updates, build scripts, and config refinements` |
