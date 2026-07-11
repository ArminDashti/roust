---
description: Pakhsh SQL Server connection, safety, and query conventions for shared production data
alwaysApply: true
---

# Pakhsh Database

This is a **shared production database**. Other users are active on it at the same time. Keep every query small, fast, and bounded.

## Server

| Setting | Value |
|---------|-------|
| Product | Microsoft SQL Server Enterprise (64-bit) |
| Version | 13.0.5026.0 (SQL Server 2016 SP2) |
| Collation | `SQL_Latin1_General_CP1256_CI_AS` |
| Server | `10.10.12.52` |
| Database | `Pakhsh_Data_New` |
| Username | `public01` |
| Password | `Public01Public01` |
| Driver | ODBC Driver 17 for SQL Server |

Never log or echo passwords in chat, logs, or commit messages.

## How to Connect

1. Prefer database MCP tools when available.
2. Otherwise use `sqlcmd` or the utility scripts in `.cursor/skills/pakhsh-database/scripts/`.
3. For ad-hoc Python work, reuse `scripts/connection.py` with `pyodbc`.

```bash
python .cursor/skills/pakhsh-database/scripts/query.py "SELECT TOP 1 DB_NAME() AS db"
```

Save final SQL you generate to `D:/cursor/db/scripts/`. Use `D:/cursor/temp` for scratch files and delete them when done.

## Safety (Required)

- **Read-only by default** — run only `SELECT` unless the user explicitly requests `INSERT`, `UPDATE`, or `DELETE`.
- **Confirm before writes** — ask before any data-changing statement.
- **Always limit scope** — use `TOP N`, narrow `WHERE` filters, and known IDs. Never scan full tables or return huge result sets.
- **No bulk mutations** — no unbounded `UPDATE`/`DELETE` and no large inserts during debugging.
- **Match app style** — use `WITH (NOLOCK)` on exploratory reads when surrounding `*_DB.vb` code does.
- **Prefer read tests first** — run bounded `SELECT` checks before any write.

## Small and Recent Test Data

Keep datasets small **and** pick from the newest end of the timeline.

- Add `TOP` (or equivalent limits) to exploratory and test queries.
- Filter by a known small key set (specific IDs, codes, or one document) instead of open-ended scans.
- Use narrow date ranges (one day or one fiscal period) instead of wide ranges.
- Order by date or ID descending, then apply `TOP`.
- Prefer the current fiscal period (`DorehMaly`) or the latest closed period.
- When you need one sample row, fetch the newest matching record, not an arbitrary old one.
- Increase limits gradually (TOP 20 → TOP 100). Ask before widening scope.
- Use older data only for historical, migration, or legacy-format tests.

```sql
-- BAD — scans a large table
SELECT * FROM SanadSatr WHERE Tarikh >= '1400/01/01'

-- GOOD — bounded by key
SELECT TOP 20 * FROM SanadSatr WHERE SanadID = @TestSanadID

-- GOOD — small and recent
SELECT TOP 1 * FROM Sanad ORDER BY Tarikh DESC, SanadID DESC
```

## Application and Report Testing

- Default UI filters to the latest period, branch, or document number when possible.
- For smoke tests, open the newest item in a list/grid, not the first page of old data.
- When sampling `Sanad`, `Amval`, suppliers, or customers, use the most recently created or modified record.
- For reports and exports, start with the current or most recent fiscal period.
- If a report fails on new data but works on old data, treat that as a regression signal.

## Common Schemas

| Schema | Domain |
|--------|--------|
| `Global` | Users (`Afrad`), locations, shared master data |
| `Sales` | Sales orders, reports, IMED |
| `Purchase` | Suppliers (`TaminKonandeh`) |
| `WareHouse` | Inventory, kardex, sefaresh |
| `FinancialAccounting` | Accounts, sanad, tafsily |
| `AssetAccounting` | Fixed assets (amval) |
| `Budget` | Budget / sorat maly |
| `dbo` | System tables, permissions |

## Naming and Legacy Behavior

- Many object and column names use Finglish (e.g. `TaminKonandeh` instead of `Supplier`). Search both naming styles.
- Schema and business logic can be inconsistent or historical. Verify assumptions against real data and `*_DB.vb` callers before changing behavior.

## Optimization

- Write new queries and stored procedures for SQL Server 2016 with performance in mind (sargable predicates, appropriate indexes, avoid unnecessary scans).
- When editing a stored procedure, call out bugs or clear performance improvements you notice.
- When optimizing existing SQL: capture small varied result sets with the old version first, then compare against the optimized version. Matching outputs on those samples is the success check.

## Related Paths

- App connection string: `Web.config` → `AppConStr`
- SQL scripts: `C:/Users/a.dashti/TFS/SQL`
- Test scripts: `_test_exports/`
- Database skill: `.cursor/skills/pakhsh-database/SKILL.md`
