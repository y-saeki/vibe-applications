# CLAUDE.md

This file provides guidance to Claude Code when working in this repository.

This repository is a place to build applications. Each application lives in its own
directory and may add its own `CLAUDE.md` with project-specific conventions
(e.g. `todo-app/CLAUDE.md`). Those apply in addition to what is written here.

## One directory per application

Every application gets a single directory at the repository root, named after the
application itself. Applications do not share a build system, a dependency tree or a
lockfile: each directory is self-contained and can be built, run and thrown away on its
own. Nothing but shared repository-level configuration belongs at the root.

## Confirm the stack before starting a new application

Which language, framework and package manager an application uses is a decision to make
with the user, not to infer from what other directories here happen to use. Ask before
scaffolding when the request does not say.

## Check the official documentation

Anything that depends on how an external framework, library or API actually behaves —
configuration fields, what a call returns, which APIs exist — is settled by reading the
current official documentation, not by recalling it. Versions move, and a habit that was
correct a few releases ago may no longer be. When a specific version is pinned, check that
version's documentation.

## Each application carries a README

Every application directory has a `README.md` covering what the application does, how to
set it up, and how to run it. Write it as the entry point for someone who has just opened
the directory and knows nothing about it.

## Write issues and pull requests in Japanese

GitHub issues and pull requests — titles and bodies alike — are written in Japanese.
This applies to issues you file, pull requests you open, and edits to existing ones.

Inside the codebase: code, identifiers, code comments and commit messages are English,
while user-facing documentation (`README.md`, `docs/`) and strings shown to the user are
Japanese.

## Link pull requests to the issue they close

A pull request that comes from an issue opens its body with a closing reference to that
issue, on its own line, before anything else:

```
Closes #11
```

Prefer `Closes`; `Fixes` and `Resolves` behave the same way. Write the keyword in English
even though the rest of the body is Japanese — GitHub only recognises the English form, so
this is the one exception to the rule above.

The keyword only works for issues in the same repository, and it closes the issue when the
pull request is merged. Reference an issue that should stay open — or one in another
repository — without a keyword instead (`Refs #12`). A pull request with no originating
issue gets no such line.

## Keep the pull request body in step with the branch

A pull request body describes the branch as it stands, not as it stood when the pull
request was opened. After pushing further commits to a branch that already has one, re-read
the body and update it — the title too, when the scope of the change moved. Renames,
reversed decisions and work added on review feedback all belong there; a typo fix or a
formatting-only commit usually changes nothing worth writing down.

This holds however the pull request was created, including ones opened from the Claude Code
UI rather than by you.

## Do not commit secrets

API keys, tokens and credentials never enter the repository. Read them from environment
variables, and commit an `.env.example` listing the required names with empty or dummy
values instead of the real `.env`.
