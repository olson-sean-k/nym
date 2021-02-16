<div align="center">
    <img alt="Nym" src="https://raw.githubusercontent.com/olson-sean-k/nym/master/doc/nym.svg?sanitize=true" width="320"/>
</div>
<br/>

**Nym** is a cross-platform library and command line tool for manipulating files
using patterns. It is inspired by and very loosely based upon `mmv`.

[![GitHub](https://img.shields.io/badge/GitHub-olson--sean--k/nym-8da0cb?logo=github&style=for-the-badge)](https://github.com/olson-sean-k/nym)
[![docs.rs](https://img.shields.io/badge/docs.rs-nym-66c2a5?logo=rust&style=for-the-badge)](https://docs.rs/nym)
[![crates.io](https://img.shields.io/crates/v/nym.svg?logo=rust&style=for-the-badge)](https://crates.io/crates/nym)

## Usage

Nym commands are formed from flags, options, and an actuator followed by a
transform. An actuator is a file operation like `append`, `copy`, `link`, or
`move`. A transform is a from-pattern used to match source files and a
to-pattern used to resolve destination paths.

The following command copies all files in the working directory tree to a
neighboring file with an appended `.bak` extension:

```shell
nym copy '**' '{#1}.bak'
```

Here, `copy` is the actuator, `**` is the from-pattern, and `{#1}.bak` is the
to-pattern. In most shells, patterns must be escaped to avoid interacting with
features like expansion. Quoting patterns usually prevents these unwanted
interactions.

## From-Patterns

From-patterns match source files to actuate using Unix-like globs. Globs must
use `/` as a path separator. Separators are normalized across platforms; glob
patterns can match paths on Windows, for example.

Globs resemble literal paths, but support three wildcards: the tree wildcard
`**`, the zero-or-more wildcard `*`, and the exactly-one wildcard `?`.

The tree wildcard `**` matches zero or more sub-directories. This is the only
way to match against arbitrary directories; all other wildcards do **not** match
across directory boundaries. When a tree wildcard participates in a match and
does not terminate the pattern, its capture includes a trailing path separator.
If a tree wildcard does not participate in a match, its capture is an empty
string with no path separator. Tree wildcards must be delimited by path
separators (or nothing, such as the beginning and/or end of a pattern).

The zero-or-more wildcard `*` matches zero or more of any character **except
path separators**. Zero-or-more wildcards cannot be adjacent to other
zero-or-more wildcards.

The exactly-one wildcard `?` matches any single character **except path
separators**. Exactly-one wildcards do not group, so a pattern of contiguous
wildcards such as `???` form distinct captures for each `?` wildcard.

## To-Patterns

To-patterns resolve destination paths. These patterns consist of literals and
substitutions. A substitution is either a capture from a corresponding
from-pattern or file metadata. Substitutions are delimited by curly braces
`{...}`.

Captures are typically indexed from a from-pattern using a hash followed by an
index, like `{#1}`. These indices count from one; the zero index is used for the
full text of a match. Empty braces also respresent the full text of a match, so
`{#0}` and `{}` are equivalent.

Captures can also be named when supported by the from-pattern (e.g., raw binary
regular expressions). Captures are referenced by name using `@` followed by the
name of the desired capture delimited by square brackets, such as
`{@[extension]}`. Note that named captures also have a numerical index.

Captures may include a condition. Conditions specify substitution text based on
whether or not the match text is empty. Conditions follow capture identifiers
using a ternary-like syntax: they begin with a question mark `?` followed by the
non-empty case, a colon `:`, and finally the empty case. Each case supports
literals, which specify alternative text delimited by square brackets `[...]`.
In the non-empty case, a surrounding prefix and postfix can be used instead
using two comma separated literals `[...],[...]`. Condition cases and
substitution text may be empty.

For example, `{#1?[],[-]:}` is replaced by the matching text of the first
capture and, when that text is **non-empty**, is followed by the postfix `-`.
`{#1?:[unknown]}` is replaced by the matching text of the first capture and,
when that text is **empty**, is replaced by the literal `unknown`.

Properties include source file metadata in the destination path and are
specified following an exclamation `!`. Properties are case insensitive.
Supported properties are described in the following table.

| Pattern    | Metadata                               |
|------------|----------------------------------------|
| `{!b3sum}` | [Blake3] hash of the source file.      |
| `{!ts}`    | Modified timestamp of the source file. |

For example, `{!b3sum}` is replaced by the Blake3 hash of the matched file.

Substitutions support optional formatters. Formatters must appear last in a
substitution following a vertical bar `|`. Formatters are separated by commas
`,`. Any number of formatters may be used and are applied in the order in which
they appear.

The pad formatter pads substitution text to a specified width and alignment
using the given character shim. For example, `{#1|>4[0]}` pads the substition
text into four columns using right alignment and the character `0` for padding.
If the original substitution text is `13`, then it becomes `0013` after
formatting.

## Development

Nym is loosely based upon `mmv`. Below are some initial ideas that have not yet
been implemented (in no particular order).

- Support for various from-patterns, including raw binary regular expressions
  and metadata filters (e.g., files modified after some timestamp).
- Custom to-pattern properties read from the shell.
- Formatters in to-patterns (e.g., width, alignment, capitalization, etc.).
- An interactive TUI.

## Installation

[Install Rust][rustup] and use `cargo` to install from a clone of the
repository.

```shell
git clone https://github.com/olson-sean-k/nym.git
cd nym
cargo install --locked --path=. --force
```

## Disclaimer

Nym is provided as is with no warranty. At the time of writing, Nym is highly
experimental and likely has many bugs. Data loss may occur. **Use at your own
risk.**

[Blake3]: https://github.com/BLAKE3-team/BLAKE3
[rustup]: https://rustup.rs/
