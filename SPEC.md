# Design

The programs in this project are intended to be the simplest possible
implementation of a processing "backend" for the DWD design. This includes the
RT (rule-taker) and RR (rule-reserve) components of the design. The RM
(rule-maker) component is intended to be implemented as an "interactive
frontend" and, therefore, is not part of this "backend". Part of this "backend"
does serve as a receiver of rule definitions from any RM implementation.

This is an implementation designed to work in a particular way that implements
the abstract design. You won't find programs named "RR" and "RT" that directly
fulfill those roles. Portions of each of those roles are divided amongst a few
programs implemented in this repository.

# Moving parts

- `api`
- `sync`

## `eval`

Eventually, you'll want to generate an "ought" document from an "input"
document, based on a rule. The `eval` program implements that core
functionality. It's designed to be used as part of this larger collection of
programs as well as on its own (typically for testing purposes).

This program accepts two arguments: `eval <rule-path> <document-path>`. This
applies the rule found in `<rule-path>` to the document in
`<document-path>`. The resulting "ought" document is written to `STDOUT`.

The `api` program, after applying the "sieve" protocol defined in the DWD
specification, will invoke this program to build a set of "oughts". To
facilitate this process, the `eval` program can also accept: `eval <rules-dir>
<documents-dir> <oughts-dir>`. For each of the documents discovered in
`<documents-dir>`, `eval` will apply all of the rules `<rules-dir>`. The
resulting "ought" documents will be organized into the `<oughts-dir>` in
subdirectories named after the corresponding document.

Together with the submissions part of the `api`, the `eval` program satifies the
rule-taker component of the design.

# Protocols

## API

## Storage

# Current state

- `api`: Mostly implemented. Will require improvement as `sync` and `eval` are implemented.
- `sync`: Unimplemented.
- `eval`: Unimplemented.

