# Responsibility detector review — 2026-09-15

## Scope and method

Reviewed the AST producer, `counters::extract_functions` integration, public
`FunctionMetrics` provenance, and detector tests against the Fix #9 requirements.
The original review was independent. After reporting defects, the root agent
authorized this reviewer to implement the bounded lexical/static resolution
fixes; the final whole-branch review therefore needs a separate reviewer.

Executable probes used the repository's pinned tree-sitter libraries with
`rustc`, independently of Cargo's compilation lock. The harness included an exact
copy of the detector, replacing only its snapshot types and test-context lookup
with minimal harness definitions. Thus the standalone probes establish AST and
production resolution behavior; test-callee classification is established by the
repository tests rather than the standalone harness.

Initial AST/output: `/tmp/detector-review-probes.log`,
`/tmp/detector-review-more.log`, and `/tmp/detector-static-p0.log`.
Post-fix probe output: `/tmp/detector-review-probes-after.log` and
`/tmp/detector-review-more-after.log`. Relevant original grammar evidence was
also incorporated into `2026-09-15-responsibility-p0.md` by the producer.

## Findings and dispositions

| Finding | Executable example / initial behavior | Disposition |
|---|---|---|
| Go/C# callback body evidence leaked into enclosing functions | Go `func_literal`, C# `anonymous_method_expression`: both outer methods received callback-only `ready` and `helper` dependencies | Producer added boundaries and regression cases; standalone repros now have empty dependencies |
| Go receiver ownership used a nested type declaration | A method on missing package-level `A` acquired the declaration from `func outer() { type A struct ... }` | Producer restricted resolution to package-level declarations; repro now has unknown owner |
| Shadow bindings omitted | Go range receiver, Python `with ... as self`, JS catch parameter, Java enhanced-for variable each supplied false evidence | Producer added binding grammar and tests; all four original repros now have empty dependencies |
| Static lookup ignored a nearer empty class | JS/Python inner empty `class A` still resolved outer `A.helper` | Resolve the receiver declaration before resolving its methods; empty declarations participate in shadowing |
| Block-local function leaked out of scope | Rust/JS outer `helper()` resolved the function declared inside a finished inner block | Resolve from the call site's lexical containers; tests assert the actual target byte identity, including the positive inner-block case |
| Static type qualification accepted ordinary instance methods | JS/Java/C#/PHP `A.helper()` or `A::helper()` accepted non-static `helper` | Static modifier required for class-qualified calls in these languages; paired positive and negative tests |
| JS static/instance method and field identities conflated | Instance `this.helper()` found constructor-static helper; static and instance `this.ready` had identical field IDs | Match receiver method mode and distinguish static field IDs; regression tests cover both |
| Kotlin companion and extension ownership | Companion methods shared class owner; extension `String.is_a` credited class field `length` | Producer added companion owner and conservative unknown extension provenance |
| Typed Python receiver omitted | `def is_a(self: A): return self.ready` produced no field | Producer added typed receiver support |
| Anonymous Java class methods had enclosing-function owner and scope | Anonymous class methods were attached to the enclosing function | Producer established anonymous class owner; resolution treats its body as a distinct type scope |
| Python bare calls could resolve class members after block handling | Class-body `block` became an ordinary lexical container | Normalize the class body to its class scope; regression asserts module helper is selected |
| Local class binding did not shadow outer free function | `class helper` inside a function followed by `helper()` could select an outer function | Non-function declaration bindings block that lookup; JS/Python regression cases |
| Static type lookup crossed semicolon namespaces | PHP namespace Second could resolve `A` declared only in First | Require matching semicolon namespace for declaration lookup |
| Bodyless signatures could supply local callee evidence | Extern/abstract methods and functions were selected without local implementation | Producer added language cases; resolution retains signatures in ambiguity counts but requires a body for selected evidence |
| Go receiver call mode was overly restrictive | Pointer receiver could not resolve a same-declaration value-receiver helper, or vice versa | Calls resolve across both modes for the same local type declaration; owner IDs remain distinct |

## P1 invariant sweep

- **Measurement/extraction preservation:** `extract_functions` still iterates the
  existing language function query and uses the same name, LOC, complexity,
  nesting, and `TestContext::contains` calculations. The provenance index only
  supplies the new optional field. Unknown ownership does not remove functions.
- **Owner separation:** declaration byte identity separates same-name class,
  trait, interface, object, Rust impl, module, namespace, and nested-function
  scopes. Go uses declaration identity plus pointer/value mode. Producer tests
  cover the original supported extensions and added owner cases.
- **Directness:** `own_nodes` stops at nested function/type boundaries. The
  detector adds each directly observed dependency once, sorts/deduplicates it,
  and never expands dependencies of a callee. Comments and strings cannot match
  the structural node checks.
- **Callee eligibility:** explicit receiver, class/module qualification,
  unqualified lexical lookup, and semicolon-namespace fallback all pass through
  uniqueness and local-body checks. Test declarations participate in ambiguity
  but cannot be selected as evidence. Parameter, alias, reassignment, import,
  modified-member, and dynamic-name guards remain in the producer.
- **Consumers:** producer output feeds `FunctionMetrics.responsibility` only;
  public serialization, cache-version policy, grouping thresholds, numeric
  baselines and report ranking are reviewed/tested by the root and grouping
  agents. They are outside this bounded review's completion claim.

## Verification

The original standalone failure matrix was rerun after the combined callback,
binding, ownership, and resolution fixes. The recorded false dependencies were
removed; block-local lookup selected the visible root helper, while Kotlin's
ordinary property initializer case still retained its direct field dependency.

Repository focused-suite results are recorded below when supplied by the
producer. Full suite, corpus, mutation testing and final independent review are
owned by the root agent; this document does not claim those checks passed.

### Final syntax follow-up

The producer identified Rust's lower-case `self::helper()` as a module-qualified
call whose receiver text matched the instance receiver. Resolution now checks
`scoped_identifier` before the instance branch. Paired tests assert no evidence
when only the impl has `helper`, and the exact root-function identity when the
module declares it. Ordinary `self.helper()` continues to use receiver lookup.

### Available focused result

The producer reports **34 detector tests passed** in the last integrated run
before the final resolution amendments. That run does not verify the subsequent
resolution tests. The latest combined detector was independently recompiled
successfully through the standalone harness with no warnings; the root's final
repository suite must cover the final source and all added tests.

## Performance follow-up: declaration indexes

The corpus run exposed repeated whole-file scans in `resolve_call` at every
ancestor of every unqualified call, plus another whole-file scan for each
qualified receiver. This repeated work was removed without changing detection
rules. The original root traversal now supplies both producer construction and
`ResolutionIndex`, which stores declarations and functions by scope/name,
functions by name for unique-callee filtering, and semicolon namespace positions.

Differential verification used binaries from immediately before and after the
indexing change, with identical tree-sitter dependencies and harness settings:

- 26 saved grammar/regression source probes: **zero output differences**.
- Synthetic 60-function, 1,200-call source: **35.30 s → 0.126 s**, identical output.
- Real Mautic `LeadBundle/Model/LeadModel.php`: **29.14 s → 0.528 s**, identical output.
- Literal-heavy Mautibot helper: about **0.010 s** in both versions, identical output.

Times are standalone harness wall-clock measurements, not predictions for the
full collector/corpus pipeline. Both binaries emitted the full AST and direct
provenance; byte equality covers selected function identities as well as labels.
No new detection rules or measurement changes were included in this follow-up.
The root agent owns the final repository tests and restarted corpus run.
