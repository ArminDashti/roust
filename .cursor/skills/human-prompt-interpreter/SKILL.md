---
name: human-prompt-interpreter
description: Use this skill whenever a human gives a vague, ambiguous, poorly structured, or overly conversational prompt that needs to be interpreted, clarified, or enhanced into clear, agent-friendly instructions before acting on it. Triggers include: requests that mix multiple goals together, prompts with implicit assumptions or missing context, casual natural language that would confuse an agent ("can you kinda like fix the thing"), multi-step tasks described as a single sentence, contradictory instructions, or any time the human's intent is unclear. Also trigger when a user asks to "enhance", "improve", "clean up", "simplify", or "rewrite a prompt for an agent", or when they paste a long rambling request and expect something structured out of it. When in doubt, use this skill — it's better to interpret than to execute blindly on a bad prompt. Also apply during and after any work (add, edit, delete code, or execute commands) to keep the project's ./growth-log/ documentation up to date as the project grows.
---

# Prompt Interpreter & Enhancer

This skill has two phases:

1. **Interpret & Enhance** — Turn a raw human prompt into clear, agent-ready instructions.
2. **Grow** — Maintain `./growth-log/` documentation as the project evolves.

---

## Phase 1 — Interpret & Enhance

Humans don't always communicate efficiently with agents. They use vague language, mix
multiple goals, assume shared context, and leave out critical details. This phase bridges
the gap: it takes a raw human prompt, interprets what the human actually wants, enhances
clarity and structure where needed, and produces a clean, unambiguous, agent-ready
instruction set — asking the human for clarification when needed.

### Core Philosophy

- **The human's words ≠ the human's intent.** Your job is to find the intent.
- **Agents need precision.** Ambiguous input produces bad output downstream — resolve
  ambiguity here, not later.
- **Ask before you assume.** One focused question beats ten wrong assumptions. But don't
  ask for things you can reasonably infer.
- **One task at a time.** If the prompt contains multiple goals, decompose them into
  separate, ordered sub-tasks.
- **Enhance, don't rewrite blindly.** Preserve the human's goals; improve structure,
  specificity, and actionability.

### Workflow

#### Step 1 — Ingest and Parse

Read the human's prompt and identify:

- **Primary intent**: What is the single most important thing they want done?
- **Secondary goals**: Are there implicit wants or side goals?
- **Scope**: How big is this? One action, a workflow, a project?
- **Missing information**: What does an agent *need* that wasn't provided?
- **Ambiguities**: Where could this be reasonably interpreted in 2+ different ways?
- **Constraints**: Any deadlines, format requirements, or things to avoid?

#### Step 2 — Clarify (if needed)

If there are blocking ambiguities — things an agent *cannot* proceed without — ask
the human before producing output. Follow these rules:

- Ask **one question at a time**. If multiple things are unclear, prioritize the most
  blocking one.
- Make the question **specific and easy to answer**. Offer options where possible.
- **Never ask about things you can reasonably infer** from context.
- If the ambiguity is minor and you can state a reasonable assumption, do so and proceed —
  flag the assumption in your output instead of asking.

Example of a good clarifying question:
> "Just to confirm — when you say 'update the file', do you mean (a) overwrite it in place,
> or (b) save a new version alongside the original?"

Example of a bad clarifying question (too vague, puts burden on human):
> "Can you tell me more about what you want?"

#### Step 3 — Decompose

Break the interpreted intent into a numbered list of discrete, atomic sub-tasks. Each task should:

- Start with an **action verb** (Create, Read, Update, Delete, Search, Validate, Send, etc.)
- Contain **exactly one action** — no "and" tasks
- Specify the **target** (which file, API, service, field, etc.)
- Include **any relevant constraints or conditions** inline
- Be **sequentially ordered** where order matters; flag parallel tasks explicitly

#### Step 4 — Produce the Agent Prompt

Output a clean, structured prompt using the format below. This is what gets handed to the
agent.

### Output Format

Always produce output in this structure:

```
## Interpreted Task

[1-2 sentence summary of what the human actually wants, in plain English]

## Assumptions

- [Any assumption you made where the prompt was unclear — be explicit]
- [If no assumptions were needed, omit this section]

## Sub-tasks

1. [Action verb] [target] — [detail/constraint if needed]
2. [Action verb] [target] — [detail/constraint if needed]
...

## Context for Agent

[Optional. Include here any background, file paths, environment info, or constraints the
agent will need but that weren't captured in the sub-tasks. Omit if not needed.]

## Open Questions

[Optional. If anything remains unresolved that the agent may hit, flag it here so the
agent knows to pause and ask rather than guess. Omit if everything is clear.]
```

### Examples

#### Example 1 — Vague single request

**Human input:**
> "can you fix the broken stuff in my project"

**Clarifying question asked:**
> "What project are we working with, and do you know which part is broken — for example,
> is it failing tests, a runtime error, a specific file?"

**After human responds:** "it's the Python backend, I keep getting a 500 error on the /users endpoint"

**Output:**
```
## Interpreted Task

Debug the Python backend to resolve a 500 error occurring on the /users API endpoint.

## Assumptions

- "Fix" means identify and resolve the root cause, not just suppress the error.
- The backend is running locally or the agent has access to the environment.

## Sub-tasks

1. Read the server logs around the /users endpoint to identify the error trace
2. Locate the route handler for /users in the codebase
3. Identify the root cause of the 500 response (unhandled exception, bad DB query, etc.)
4. Implement a fix for the identified cause
5. Validate the fix by hitting the /users endpoint and confirming a 200 response
6. Add or update a test case covering this endpoint to prevent regression

## Open Questions

- If a database issue is found, does the agent have permission to modify schema or
  only application-layer code?
```

#### Example 2 — Multi-goal rambling prompt

**Human input:**
> "I want to set up a weekly digest email thing that pulls our top Slack messages and also
> maybe tweets and then summarises them and sends it Monday morning, oh and it should be
> pretty not just plain text"

**No clarifying question needed — enough context to proceed with assumptions flagged.**

**Output:**
```
## Interpreted Task

Build an automated weekly digest system that aggregates top Slack messages (and optionally
tweets), summarises them, and sends a formatted HTML email every Monday morning.

## Assumptions

- "Top" Slack messages = most-reacted or most-replied messages from the past 7 days.
- Twitter/X integration is optional (v2) — Slack is the primary source for now.
- "Pretty" means HTML email, not plain text.
- Send time is Monday 9:00 AM in the user's local timezone (to be confirmed).
- Delivery is via an existing email provider (SMTP or API like SendGrid/Resend).

## Sub-tasks

1. Connect to the Slack API and fetch the top 10 messages from the past 7 days (by reaction count)
2. Pass the fetched messages to a summarisation step to produce a short digest blurb per message
3. Design an HTML email template with clear sections, sender names, and reaction counts
4. Populate the template with the summarised content
5. Configure a Monday 9:00 AM cron job (or scheduled task) to trigger the pipeline
6. Send the rendered HTML email via the configured email provider
7. Log each send with timestamp and recipient for debugging

## Open Questions

- Which Slack workspace and channel(s) should be monitored?
- Which email provider should be used for sending (SendGrid, Resend, SMTP, other)?
- Should tweets be included now or deferred to a later version?
```

### Anti-patterns to Avoid

| Anti-pattern | Instead |
|---|---|
| Executing on the raw prompt without interpreting | Always interpret first |
| Asking 3+ clarifying questions at once | Ask one, the most critical one |
| Writing vague sub-tasks like "handle errors" | Write "Catch and log exceptions from the /users DB query" |
| Lumping two actions into one task | Split at every "and" |
| Assuming away a genuinely blocking unknown | Ask |
| Producing output before a blocking question is answered | Pause; ask first |
| Over-editing and changing the human's intent | Enhance clarity while preserving goals |

### Handling Edge Cases

**The prompt is already clear and well-structured:**
Skip interpretation, note that the prompt is clear, and pass it through with minimal
reformatting. Don't add unnecessary complexity.

**The human is clearly mid-thought or typing incrementally:**
Wait for them to finish or ask "Want to add anything before I break this down?"

**The request involves a tool or system you don't know:**
Flag it in `## Open Questions` and proceed with what you do know. Don't block on it.

**The human pushes back on your interpretation:**
Accept the correction, update your understanding, and re-output. Don't argue — your
interpretation is a hypothesis, not a verdict.

**The human asks only to enhance an existing prompt:**
Apply the same workflow but focus on structure, specificity, and agent-readiness.
Do not change the underlying goal unless ambiguity requires clarification.

---

## Phase 2 — Growth Log

Every time you add, edit, or delete code, or execute something that changes the project,
update the `./growth-log/` files below. Maintain these files as the project grows and
always keep them up to date. Create the folder and any missing files if they do not exist.

### Growth log files

```
./growth-log/
├── architecture-schematic.md
├── architecture-technical.md
├── architecture-non-technical.md
├── goals.md
├── directory-tree.md
├── features.md
├── modules.md
├── suggestion.md
└── bugs.md
```

### What to write in each file

| File | Purpose |
|---|---|
| `features.md` | All the functions this app can currently perform |
| `directory-tree.md` | Every file in the project with a very short description |
| `modules.md` | Break the app into modules and list them |
| `suggestion.md` | Suggestions from the agent; the user will review later |
| `bugs.md` | Potential bugs or confirmed bugs |
| `goals.md` | Goals listed by the user; note whether each has been achieved |
| `architecture-schematic.md` | A schematic of how the app works |
| `architecture-technical.md` | Explain the architecture in technical terms |
| `architecture-non-technical.md` | Explain the architecture in non-technical terms |

### When to update

- After completing any sub-task from Phase 1 that changes the codebase or project structure
- When new features, modules, bugs, or goals are discovered or resolved
- Before marking a task as done — the growth log must reflect the current state
