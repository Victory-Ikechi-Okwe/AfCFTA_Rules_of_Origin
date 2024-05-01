A Rust implementation of the rule-taker (RT) and rule-reserve (RR) design
components of DWDS.

# Design

The programs in this project are intended to be the simplest possible
implementation of a processing "backend" for the DWD design. This includes the
RT (rule-taker) and RR (rule-reserve) components of the design. Portions of
each of those roles are divided amongst a few programs implemented in this
repository.


The RM (rule-maker) component is intended to be implemented as an "interactive
frontend" and, therefore, is not part of this "backend". Part of this "backend"
does serve as a storage sink for rule definitions that could be sent from any RM
implementation.

This implementation follows a "decomposed" design. Rather than containing a
monolithic single application, it's made up of several small binaries that each
perform a specific role. These binaries can be composed in different ways,
possibly with additional binaries, to create a complete system. This delineates
the anticipated "moving parts" of a RR and RT implementation so that they can be
studied individually by programmers working on other implementations.

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

## `select`

_This functionality was prototyped in the `api` program and needs to be
extracted into an independent program_.

This program is designed to implement the "select" of the DWD algorithm. It
accepts a document as input and outputs a list of matching rule ids. It is
invoked as `select <path/to/document>` and writes the list of matching rule ids
to `STDOUT`.

The operating idea for this program is that figuring out which rules are "in
effect" and "applicable" should be no more complicated than some basic SQL
queries. We take that literally and record everything we need when `ingest`
accepts a rule. All this program does is run those queries. It's an independent
binary so we can make sure that it remains a "reader" of the DB.

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

We assume that rules are far more often read than written. To make `select`
simple, this program takes the time to parse submitted rules and transform the
metadata into entries in a couple of DB tables. This simplifies `select`,
allowing it to do less and operate faster.

This program can be invoked as `ingest <path/to/rule> [<id>]`. This reads the
contents of the referenced rule file, adds the rule to the index and retains the
rule file. The rule id argument is optional. The `ingest` program assumes that
the calling context (`api`, `sync`, or an interactive invocation), if it
provides the `id` argument, "knows better". It does not verify whether the
provided rule id matches the `id` field of the rule metadata. If the argument is
not provided, `ingest` parses it from the `id` field in the rule file.

## `sync`

The `sync` program downloads rules stored on the network and integrates them
with the local rule storage. This program only downloads the rules from the
remote network location. It uses `ingest` to add them to the rule storage.

This program accepts a URL or an IPFS content id as an argument (`sync
<url|content_id>`). It expects to find an [RSS
2.0](https://www.rssboard.org/rss-specification) file at the remote
location. That file (or "feed") should contain references to _all_ of the rules
retained at the remote location. The feed should contain rule ids, versions, and
references to the full rule content to allow `sync` to determine whether the
full content should be downloaded and stored.

## `invoke`

Eventually, you'll want to generate an "ought" document from an "input"
document, based on a rule. The `invoke` program implements that core
functionality. It's designed to be used as part of this larger collection of
programs as well as on its own (typically for testing purposes).

This program accepts two arguments: `invoke <rule-path> <document-path>`. This
applies the rule found in `<rule-path>` to the document in
`<document-path>`. The resulting "ought" document is written to `STDOUT`.

The `api` program, after applying the "select" protocol defined in the DWD
specification, will invoke this program to build a set of "oughts". To
facilitate this process, the `invoke` program can also accept: `invoke <rules-dir>
<documents-dir> <oughts-dir>`. For each of the documents discovered in
`<documents-dir>`, `invoke` will apply all of the rules `<rules-dir>`. The
resulting "ought" documents will be organized into the `<oughts-dir>` in
subdirectories named after the corresponding document.

This program forms part of the RT component of the DWD "triad" design.

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

## Processing

# Current state

- `api`: Mostly implemented; will require improvement as `sync` and `invoke` are implemented
- `sync`: Unimplemented
- `invoke`: Unimplemented
- `ingest`: Requires extraction from `api`
- `select`: Requires extraction from `api`

