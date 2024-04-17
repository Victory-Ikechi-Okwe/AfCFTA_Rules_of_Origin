A Rust implementation of the rule-taker (RT) and rule-reserve (RR) components of DWDS.

# Design

The programs in this project are intended to be the simplest possible
implementation of a processing "backend" for the DWD design. This is referred to
as the "triad". This includes the RT (rule-taker) and RR (rule-reserve)
components of the design. The RM (rule-maker) component is intended to be
implemented as an "interactive frontend" and, therefore, is not part of this
"backend". Part of this "backend" does serve as a receiver of rule definitions
from any RM implementation.

This is an implementation designed to work in a particular way that implements
the abstract design. You won't find programs named "RR" and "RT" that directly
fulfill those roles. Portions of each of those roles are divided amongst a few
programs implemented in this repository.

This implementation follows a "decomposed" design. Rather than containing a
monolithic single application, it's made up of several small binaries that each
perform a specific role.

# Moving parts
## `api`

The sole purpose of the API is to receive requests from integrated programs and
to provide access to the results of processing. It implements a message-based
protocol (described below) over websockets.

This program is designed to integrate with as many network-capable programs as
possible. Ideally, it would use a more-raw network protocol (rather than
websockets) but, since it anticipates single-page applications (SPA) as the
primary integration, it uses websockets due to that technology's popularity as a
basic transport for message-based protocols in Javascript SPAs.

## `sieve`

_This functionality was prototyped in the `api` program and needs to be
extracted into an independent program_.

This program is designed to implement the "sieve" of the DWD algorithm. It
accepts a document as input and outputs a list of matching rule ids. It is
invoked as `sieve <path/to/document>` and writes the list of matching rule ids
to `STDOUT`.

To support the submissions aspect of the storage protocol (see below), this
program is also capable of running persistently as a child process (intended to
be spawned by the `api` program). When run in this mode, the program accepts
document ids via `STDIN` and writes corresponding rule ids to `STDOUT`.

This program forms part of the RT component of the DWD "triad" design.

## `ingest`

_This functionality was prototyped in the `api` program and needs to be
extracted into an independent program_.

The purpose of the `ingest` program is to accept rules and retain them on-disk
(see below for the storage protocol). It is the sole writer of the "rule index"
implemented as an SQLite DB.

This program can be invoked as `ingest <path/to/rule>`. This reads the contents
of the referenced rule file, adds the rule to the index and retains the rule
file.

This program is also capable of running persistently as a child process
(intended to be spawned by the `api` program). When run in this mode, the
program accepts rules via `STDIN`.

## `sync`

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

The `api` program implements a simplistic "message-based" protocol. This is
currently only implemented using websockets.

Messages for the API are written as JSON Arrays. Each array has 3 parts:

```
[action: String, args: Array, doc: Object]
```

- `command`: The action to execute
- `args`: A JSON Array of arguments specific to the `command`
- `doc`: A JSON Object that may be optional, depending on the `commands`

The commands are:

- `[GET, [rule_id, version], {}]`: Gets the rule content from internal storage
  and writes it to the web socket. The `doc` part of the message is
  ignored. There are two args: `rule_id`, `version`. The `version` argument is
  optional.
- `[GET, [rule_id, version], {}]`: Publishes the referenced rule, making it the
  active version of the rule when documents are sumitted. The `doc` part of the
  message is ignored. There are two args: `rule_id`, `version`. The `version`
  argument is optional. If not specified, the highest version is used.
- `[STORE, [rule_id], {}]`: Stores rule content specified in the `doc`
  parameter. The `rule_id` is optional. If specified, a new version of the
  referenced rule is created, otherwise a new `rule_id` is generated and written
  as a result with a starting version of `1`.
- `[SUBMIT, [], {}]`: Submits a document for processing using the submission
  aspect of the storage protocol. The `args` are currently empty.

## Storage

# Current state

- `api`: Mostly implemented; will require improvement as `sync` and `eval` are implemented
- `sync`: Unimplemented
- `eval`: Unimplemented
- `ingest`: Requires extraction from `api`
- `sieve`: Requires extraction from `api`

