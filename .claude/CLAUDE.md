@../AGENTS.md

<!--
Why this file exists, and why it is only an import.

Claude Code reads CLAUDE.md and does not read AGENTS.md. This repository's
instructions live in the root AGENTS.md, which every other agent reads directly.
Importing it here means both read the same file, so the two cannot disagree and
this repository keeps one source of instructions rather than two.

The import path is relative to THIS file, not to the repository root, so it has
to be ../AGENTS.md. Writing @AGENTS.md would resolve to .claude/AGENTS.md, which
does not exist.

Do not paste AGENTS.md content into this file. A second copy is the thing the
shared instruction set forbids, and for the same reason: the copy goes stale and
then quietly overrides the original.

Claude Code strips block level HTML comments before injecting the file, so this
note costs no context and stays here for whoever reads the file directly.
-->
