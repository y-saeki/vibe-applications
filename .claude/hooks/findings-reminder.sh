#!/bin/sh
# UserPromptSubmit hook: remind the session, on every message from the owner, to
# write what the owner points out into the pull request body. The lessons
# workflow turns that section into documentation once the pull request merges.
# See "Write the owner's findings into the pull request" in the root CLAUDE.md.
cat >/dev/null
cat <<'JSON'
{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"If this message points out something about your work or settles a judgement call that should hold next time too, add it to the \"## オーナーからの指摘\" section of the pull request body, as the root CLAUDE.md describes under \"Write the owner's findings into the pull request\"."}}
JSON
