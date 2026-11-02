# Candidate-bound Markdown references

This native v1 adapter implements a bounded documentation slice of
[CAP-KNW-001](../product/capability-catalog.md#cap-knw-001) in
[003-B](../product/roadmap.md#003-temporal-knowledge-and-memory).
It uses the existing [source index](knowledge-references.md),
[candidate binding](candidate-binding.md) and Evidence validity vocabulary.
It does not complete those capabilities or establish documentation truth.

## Native v1 coverage

The profile is a lexical contract, not full CommonMark or GitHub rendering.
Input is nonempty UTF-8 with LF or CRLF. Non-ASCII prose and inline link text
retain their exact bytes. Controls other than tab/newline are refused.

ATX headings start with up to three spaces and one through six hashes followed
by whitespace. Their content is nonempty plain ASCII text, with optional closing
hashes separated by whitespace. Backslash escapes for ASCII punctuation are
decoded. Asterisks, inline code, entities, HTML and explicit anchor annotations
in headings are refused. Underscores are literal under this lexical rule. Anchors
lowercase ASCII letters, preserve digits,
hyphens and underscores, replace each space/tab with a hyphen, and drop other
ASCII punctuation. Empty anchors are refused. In source order, a collision tries
`-1`, `-2`, etc. until globally unique, including collisions with literal suffixed
headings. This is the adapter's declared rule; no renderer is consulted.

Inline links use same-line `[text](destination)` or `[text](<destination>)`.
Bare destinations may have balanced parentheses to depth 32 and backslash-escaped
ASCII punctuation. Raw whitespace, titles and nested brackets in link
syntax are refused. Link text may contain same-line inline code. Images are
outside this profile and refused. Ordinary escaped brackets and ordinary bracket
text with no definition remain non-links.

Reference definitions use a single line `[label]: destination`, optionally an
angle-delimited destination, with up to three leading spaces. Full `[text][label]`,
collapsed `[label][]` and defined shortcut `[label]` uses are supported. Labels
are ASCII, case-insensitive and normalize consecutive spaces/tabs to one space;
backslash-escaped punctuation is decoded. Duplicate normalized definitions are
refused, rather than silently selecting one. Explicit uses without a definition
remain unresolved outside coverage. Multiline definitions/links are refused,
including a following-line destination or reference label that could continue
preceding bracket text. Blank lines separate
ordinary bracket text. Angle destinations treat parentheses literally. Entities
are never decoded: local fragments containing them remain unsupported.

Top-level backtick/tilde fences of at least three characters, indented by up to
three spaces, suppress all contents until a matching same-character closing run
of at least the opening length with only trailing whitespace. Unterminated fences
are malformed. Lines starting with four spaces or a tab are excluded as indented
code. Flat list prose may contain links; indented continuations after a list
that could hide nested code are refused. Same-line inline code uses matching
exact-length backtick runs; an unmatched
run is malformed. Container interpretation (block quotes/list continuations) is
outside the profile: container-prefixed code/heading forms are refused. HTML,
autolinks, Setext heading forms and explicit anchors are refused so they cannot
hide an unsupported target behind an observed missing-target claim.

Only destinations beginning with `#` are considered for same-document resolution.
The fragment must contain only lowercase ASCII letters, digits, hyphens and
underscores. A supported existing anchor resolves locally; a supported absent
anchor is `MissingTarget`. Empty, non-ASCII, percent/entity-bearing or other
unsupported fragments are `UnsupportedFragment`. Every other destination,
including relative documents, absolute paths, network URLs and special schemes,
is `OutsideCoverage`. Nothing is fetched, opened or synthesized as a target.
An unsupported anchor mechanism refuses the source; it never becomes a claim
that the target was observed missing.

## Locations and relationships

The root locator is empty and spans the whole source. Heading locators are `#`
plus the unique anchor, spanning the entire ATX line except its line ending.
Link locators are `link@BYTE_OFFSET`; definition locators are
`definition@BYTE_OFFSET`, where the offset is the opening bracket. Link spans
cover their complete markup; definition spans cover the complete definition line
excluding indentation and line ending. Repeated occurrences retain distinct
locators and exact half-open UTF-8 byte spans. Core orders values by locator.

Inline and definition relationships retain the original destination spelling
(without angle delimiters). Reference uses retain their whole original markup
and point to the captured definition occurrence. Definitions in turn point to
local headings or an unresolved target. Incoming queries can therefore inspect
both steps without inventing a target. These indexes must never be passed to the
JSON-pointer-specific `ReferenceSet` resolver.

## Producer and validity

The `knowledge::markdown::extract_markdown_references` function accepts the
repository, binding contract, first observation ID, one explicit `SourcePath` and
recording instant, following the existing producer signature. It reads a regular committed blob at the actual before-capture commit,
parses, obtains a second real capture, requires Core binding and constructs one
immutable `ReferenceIndex`. Failure publishes no partial index. Sequences n/n+1
identify captures; n+2 identifies extraction, with checked arithmetic. The caller
supplies IDs and recording time; these are unauthenticated assertions.

`qualify` accepts a supplied later observation; `reinspect` performs actual Git
inspection. Matching candidate/profile and fully resolved relationships permit
scoped `Current`; known mismatch is `Stale`; absent observations or unresolved
coverage are unestablished. These functions do not mutate historical facts.
Syntax, a local edge and Current validity imply no authenticity, semantic
validation, permission or evidence PASS. Snapshot races and all existing Git
coverage exclusions remain; there is no atomic snapshot or dirty-source identity.

## Bounds and failures

Existing bounds apply: 256 KiB source, 4096 nodes, 1024 relationships, 4096 bytes
per locator/spelling/path, and 256 KiB aggregate locator bytes. Parenthesis depth
is 32. Source acquisition retains the existing bounded read-only Git helper,
process/capture budgets, literal byte paths and no lazy fetching or filters.
Invalid UTF-8, unsupported syntax/anchor mechanisms, malformed/truncated input,
ambiguous definitions, limits, acquisition and binding failures are typed errors.
Errors and Debug omit source snippets. Only explicit index getters expose bytes.
There is no new dependency, command, index class, retrieval, persistence, transport,
prompt generation, method selection or remote resolution.

## Acceptance mechanisms

The adapter's private parser tests and
[real-Git integration tests](../../crates/cortex-cli/tests/knowledge_markdown.rs)
exercise exact spans, local/missing/unsupported targets, repeated headings/links,
reference relationships, code exclusions, UTF-8 and finite bounds. Temporary
committed fixtures include representative Cortex documentation and a canonical
architecture source. Candidate/profile qualification, read-only content/metadata
comparison, detached/mismatched/dirty/missing/symlink cases and a controlled capture
race preserve the acquisition boundary. Test source records a mechanism; actual
commands, counts, candidate and limitations belong in the private worker handoff.
