---
name: variable-names
description: >
  Governs how agents name variables, functions, classes, files, modules, hooks, types, constants,
  and all other code identifiers when writing new code or suggesting names. Apply whenever writing
  any new code — TypeScript, Python, Go, Rust, Java, Swift, Kotlin, or any other language — or when
  the user asks for variable name suggestions, naming conventions, or identifier naming decisions.
  Names must feel like they belong to a world-class product: cutting-edge, globally professional,
  and polished. Trigger on code generation, scaffolding, refactoring of new code, API design, schema
  definition, or naming decisions. Do NOT rename existing identifiers unless the user explicitly asks.
---

# Variable Naming Conventions

## Core Principle

Every identifier you write is a surface of the product. Names should feel like they were coined by
a senior engineer at Stripe, Vercel, Linear, or Anthropic — precise, intentional, and effortlessly
modern. Vague, generic, or throwaway names degrade code quality. Invest in names.

---

## The Golden Rules

1. **Names are for new code only.** Never rename existing variables, files, classes, or functions
   unless the user explicitly requests it. Treat existing names as immutable.

2. **Clarity over brevity, but never verbose.** `userSessionToken` beats `tok` and beats
   `theCurrentUserAuthenticationSessionToken`.

3. **Domain-first.** Names should encode *what* a thing is in its domain, not *how* it works.
   `invoiceDraftQueue` > `arrayOfPendingThings`.

4. **Globally neutral.** Avoid slang, cultural idioms, or locale-specific references. Names must
   read cleanly to engineers worldwide.

5. **No placeholders.** Never use `foo`, `bar`, `baz`, `temp`, `test123`, `thing`, `data2`, `obj`,
   or similar throwaway names — even in examples. Always use a domain-meaningful name.

---

## Naming Patterns by Identifier Type

### Variables & Parameters
- Use **camelCase** (JS/TS/Java/Swift/Kotlin) or **snake_case** (Python/Rust/Go) per language convention.
- Name after the domain concept, not the data type.

| ❌ Avoid | ✅ Prefer |
|----------|-----------|
| `data` | `invoicePayload` |
| `res` | `checkoutResponse` |
| `arr` | `pendingTransactions` |
| `tmp` | `resolvedIdentity` |
| `flag` | `isEmailVerified` |
| `val` | `discountMultiplier` |

### Booleans
- Always prefix: `is`, `has`, `can`, `should`, `was`, `did`, `needs`.
- State what is true, never what is absent.

| ❌ Avoid | ✅ Prefer |
|----------|-----------|
| `notAdmin` | `isGuestUser` |
| `noCache` | `shouldBypassCache` |
| `loaded` | `isResourceReady` |

### Functions & Methods
- **Verb + noun** structure. The verb describes the action; the noun describes the subject.
- Async functions may add `fetch`, `resolve`, `load`, `stream` as prefixes.

| ❌ Avoid | ✅ Prefer |
|----------|-----------|
| `process()` | `normalizeInvoiceLineItems()` |
| `doStuff()` | `reconcilePaymentLedger()` |
| `getData()` | `fetchUserSubscriptionTier()` |
| `handleIt()` | `dispatchWebhookEvent()` |
| `run()` | `executeMigrationPipeline()` |

### Classes & Types
- **PascalCase** always.
- Name after the domain entity or capability, not the implementation.

| ❌ Avoid | ✅ Prefer |
|----------|-----------|
| `Manager` | `SessionCoordinator` |
| `Handler` | `WebhookEventRouter` |
| `Helper` | `CurrencyFormatter` |
| `Processor` | `BatchTransactionReconciler` |
| `Data` | `UserIdentityProfile` |

### Interfaces & Abstract Types (TypeScript / Java / Go)
- For TypeScript interfaces describing a *shape*: no `I` prefix. Use the domain noun directly.
  - `UserProfile`, `PaymentIntent`, `SubscriptionTier`
- For interfaces describing a *capability/contract*: use `-able` or `-er` suffixes.
  - `Serializable`, `EventEmitter`, `CacheProvider`

### Constants & Enums
- **SCREAMING_SNAKE_CASE** for primitive constants.
- **PascalCase** for enum names; **PascalCase** members in TS/Python, **SCREAMING_SNAKE_CASE** in Java/Go.

```ts
// ✅
const MAX_RETRY_ATTEMPTS = 3;
const DEFAULT_PAGINATION_LIMIT = 20;

enum SubscriptionStatus {
  Active = "active",
  PastDue = "past_due",
  Canceled = "canceled",
}
```

### React / Vue Hooks & Composables
- Prefix with `use`. Name after the domain capability, not the internal mechanism.

| ❌ Avoid | ✅ Prefer |
|----------|-----------|
| `useData()` | `useInvoiceTimeline()` |
| `useQuery()` | `useOrganizationMembers()` |
| `useHelper()` | `useCheckoutSession()` |

### Event Handlers
- Prefix with `handle` + domain subject + action.

```ts
// ✅
handleInvoiceSubmit()
handleUserAvatarUpload()
handleStripeWebhookVerification()
```

---

## File & Directory Naming

| Type | Convention | Example |
|------|-----------|---------|
| Components (React/Vue/Svelte) | PascalCase | `InvoiceSummaryCard.tsx` |
| Pages / Routes | kebab-case | `subscription-upgrade.tsx` |
| Utilities / Helpers | kebab-case | `currency-formatter.ts` |
| Hooks | camelCase with `use` prefix | `usePaymentSession.ts` |
| Config files | kebab-case | `stripe-client.config.ts` |
| Store / State modules | kebab-case | `user-identity.store.ts` |
| Test files | mirror source name + `.test` or `.spec` | `InvoiceSummaryCard.test.tsx` |
| Python modules | snake_case | `payment_reconciler.py` |
| Go packages | single lowercase word | `ledger`, `gateway`, `identity` |

**Directory names** are always `kebab-case` in web projects, `snake_case` in Python, lowercase in Go.

---

## Language-Specific Quick Reference

### TypeScript / JavaScript
- Variables/params: `camelCase`
- Classes/types/interfaces: `PascalCase`
- Files: `PascalCase` for components, `kebab-case` for everything else
- Constants: `SCREAMING_SNAKE_CASE`

### Python
- Variables/functions: `snake_case`
- Classes: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules/packages: `snake_case`
- Private: `_leading_underscore`

### Go
- Exported: `PascalCase`
- Unexported: `camelCase`
- Packages: single lowercase word, no underscores
- Acronyms stay uppercase: `userID`, `parseHTTPRequest`

### Rust
- Variables/functions: `snake_case`
- Types/traits/enums: `PascalCase`
- Constants/statics: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`

---

## Naming Anti-Patterns to Actively Avoid

| Pattern | Why | Fix |
|---------|-----|-----|
| `Manager`, `Handler`, `Helper`, `Util` as standalone class names | Meaningless — describes nothing | Name the domain role: `SessionCoordinator`, `PaymentRouter` |
| `data`, `info`, `details` as variable names | No semantic content | Name the entity: `orderMetadata`, `userProfile` |
| Numeric suffixes: `component2`, `handler3` | Signals confusion, not versioning | Create distinct, domain-driven names |
| Abbreviations beyond `id`, `url`, `ctx`, `err` | Reduces readability globally | Spell it out: `configuration` not `cfg` |
| Single-letter names outside loop counters (`i`, `j`) | Unreadable at a glance | `userId`, `productIndex` |
| Redundant type suffixes: `userObject`, `dataArray` | The type is implicit | `user`, `transactions` |

---

## Precision Vocabulary Reference

Reach for these words when they precisely match the domain concept:

| Instead of... | Consider... |
|--------------|-------------|
| `get` | `fetch`, `resolve`, `derive`, `extract`, `retrieve`, `hydrate` |
| `set` | `assign`, `configure`, `persist`, `commit`, `apply` |
| `update` | `patch`, `revise`, `amend`, `reconcile`, `sync` |
| `delete` | `revoke`, `purge`, `archive`, `invalidate`, `dismiss` |
| `check` | `validate`, `verify`, `assert`, `inspect`, `audit` |
| `send` | `dispatch`, `emit`, `publish`, `broadcast`, `relay` |
| `run` | `execute`, `invoke`, `trigger`, `orchestrate`, `initiate` |
| `make` | `construct`, `compose`, `instantiate`, `scaffold`, `forge` |

---

## Suggesting Names (When Asked)

When the user asks for name suggestions rather than new code:

1. **Anchor in domain context** — infer or ask what the identifier represents before suggesting.
2. **Offer 2–3 options** with a one-line rationale each, ranked by fit.
3. **Match language conventions** — camelCase vs snake_case per the target language.
4. **Prefer the precision vocabulary** above over generic verbs.
5. **Never suggest placeholders** — every option must be production-ready.

Example response format:

```
For a variable holding the user's pending invoice total before tax:

1. `pendingInvoiceSubtotal` — clearest; separates pre-tax amount from final total
2. `draftInvoiceAmount` — good if the invoice is not yet finalized
3. `preTaxInvoiceTotal` — explicit about tax exclusion
```

---

## Do Not Rename Rule

This is critical: **never rename existing identifiers**, even if they violate the conventions above.
The user's existing codebase is out of scope. Apply these conventions **only** to:
- New files being created
- New variables, functions, classes written from scratch
- New parameters in new function signatures
- Name suggestions the user explicitly requests

If the user explicitly says "rename this", "clean up these names", or "refactor the naming", then
and only then apply the conventions to existing code — and confirm scope before doing so.
