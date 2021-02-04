<div align="center">
    <img alt="Nym" src="https://raw.githubusercontent.com/olson-sean-k/nym/master/doc/nym.svg?sanitize=true" width="320"/>
</div>
<br/>

**Nym** is a library and command line tool for manipulating files using
patterns. It is inspired by and very loosely based upon `mmv`.

[![GitHub](https://img.shields.io/badge/GitHub-olson--sean--k/nym-8da0cb?logo=github&style=for-the-badge)](https://github.com/olson-sean-k/nym)
[![docs.rs](https://img.shields.io/badge/docs.rs-nym-66c2a5?logo=rust&style=for-the-badge)](https://docs.rs/nym)
[![crates.io](https://img.shields.io/crates/v/nym.svg?logo=rust&style=for-the-badge)](https://crates.io/crates/nym)

## Usage

Nym commands are formed from flags, options, an actuator, and a transform
comprised of a from-pattern and to-pattern. An _actuator_ is a file operation
like append, copy, link, or move. A _transform_ is a from-pattern used to match
source files and a to-pattern used to resolve destination paths.

The following command copies all files in the working directory tree to a file
with an appended `.bak` extension:

```shell
nym copy '**' '{#1}.bak'
```

Here, `copy` is the actuator, `**` is the from-pattern, and `{#1}.bak` is the
to-pattern.

## From-Patterns

From-patterns match source files to actuate using Unix-like globs. Globs must
use `/` as a path separator. Separators are normalized across platforms; glob
patterns can match paths on Windows, for example.

Globs resemble literal paths, but support three special tokens: the tree token
`**`, the zero-or-more token `*`, and the exactly-one token `?`.

The tree token `**` matches zero or more sub-directories. This is the only way
to match against directories; all other tokens do **not** match across directory
boundaries. When a tree token participates in a match and does not terminate the
pattern, its capture includes a trailing path separator. If a tree token does
not participate in a match, its capture is an empty string with no path
separator. Tree tokens cannot be adjacent to any other tokens.

The zero-or-more token `*` matches zero or more of any character **except path
separators**. Zero-or-more tokens cannot be adjacent to other zero-or-more
tokens.

The exactly-one token `?` matches any single character **except path
separators**. Exactly-one tokens do not group, so a pattern of contiguous tokens
like `???` form distinct captures for each `?` token.

## To-Patterns

To-patterns resolve destination paths. These patterns consist of literals,
captures from a corresponding from-pattern, and file properties. Non-literals
occur within curly braces `{...}`.

Captures are typically indexed from a from-pattern using a hash followed by an
index, like `{#1}`. These indices count from one and the index zero is used for
the full text of a match. Empty braces also respresent the full text of a match,
so `{#0}` and `{}` are equivalent.

Captures can also be named when the from-pattern is a raw binary regular
expression. Captures are referenced by name using `@` followed by the name of
the desired capture, such as `{@extension}`. Note that named captures also have
a numerical index.

Captures may include a _substitution_. Substitutions specify a prefix and
postfix that are inserted around the matching text when the capture is
non-empty.  Substitutions also specify alternative text, which is used if the
capture is empty. This is useful when a pattern may not be present and an
explicit replacement or optional delimeter is desired.

Substitutions follow capture identifiers beginning with a question mark `?` and
followed by a prefix, a postfix, and an alternative separated by colons `:`. For
example, `{#1?:-:}` is replaced by the matching text of the first capture and a
postfixed character "-" if the capture is non-empty and is replaced with an
empty string otherwise. `{#1?::unknown}` is replaced by the text "unkown" if the
capture is empty.

Properties include source file metadata in the destination path and are
specified following an exclamation `!`. Properties are case insensitive.
Supported properties are described in the following table.

| Pattern        | Description                            |
|----------------|----------------------------------------|
| `{!b3sum}`     | Blake3 hash of the source file.        |
| `{!timestamp}` | Modified timestamp of the source file. |

## Installation

Use `cargo` to install from a clone of the repository.

```shell
git clone https://github.com/olson-sean-k/nym.git
cd nym
cargo install --locked --path=. --force
```

## Disclaimer

Nym is offered as is with no warranty. Data loss may occur. **Use at your own
risk.**
