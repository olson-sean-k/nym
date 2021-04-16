<div align="center">
    <img alt="Nym" src="https://raw.githubusercontent.com/olson-sean-k/nym/master/doc/nym.svg?sanitize=true" width="320"/>
</div>
<br/>

**Nym** is a cross-platform library and command line interface for manipulating
files using patterns. It is inspired by and very loosely based upon `mmv`.

[![GitHub](https://img.shields.io/badge/GitHub-olson--sean--k/nym-8da0cb?logo=github&style=for-the-badge)](https://github.com/olson-sean-k/nym)
[![docs.rs](https://img.shields.io/badge/docs.rs-nym-66c2a5?logo=rust&style=for-the-badge)](https://docs.rs/nym)
[![crates.io](https://img.shields.io/crates/v/nym.svg?logo=rust&style=for-the-badge)](https://crates.io/crates/nym)

## Usage

Nym commands are formed from flags, options, and patterns. Most commands are
transforms composed of both a from-pattern to match source files and a
to-pattern to resolve destination paths. Transforms include the `append`,
`copy`, `link` and `move` commands. Some commands, such as `find`, use only a
from-pattern.

Nym operates exclusively on files (with the exception of the `--parent`/`-p`
flag, which creates parent directories in destination paths derived from
to-patterns). Commands never apply to directories. It is **not** possible to
copy, link, or move directories.

The following command copies all files in the working directory tree to a
neighboring file with an appended `.bak` extension.

```shell
nym copy '**' '{#0}.bak'
```

Here, `copy` is the sub-command (also known as an actuator), `**` is the
from-pattern, and `{#0}.bak` is the to-pattern. Note that in most shells
patterns must be escaped to avoid interacting with features like expansion.
Quoting patterns usually prevents these unwanted interactions.

The following command finds all files beneath a `src` directory with either the
`.go` or `.rs` extension.

```shell
nym find '**/src/**/*.{go,rs}'
```

## From-Patterns

From-patterns match source files to actuate using Unix-like globs. These globs
**always** use forward slash `/` as a path separator and completely disallow
back slash `\` literals. Separators are normalized across platforms; glob
patterns can match paths on Windows, for example.

Note that on Windows complex UNC paths or paths with other prefixes can be used
via the `--tree`/`-C` option. For example, the following command copies all
files from the UNC share path `\\server\share\src`.

```shell
nym copy --tree=\\server\share 'src/**' 'C:\\backup\\{#1}'
```

Globs resemble literal paths, but additionally support wildcards, character
classes, and alternatives.

### Wildcards

Globs support wildcards that match different fragments of paths and provide
capture text that can be used in to-patterns.

The tree wildcard `**` matches zero or more sub-directories. **This is the only
way to match against arbitrary directories**; all other wildcards do **not**
match across directory boundaries. When a tree wildcard participates in a match
and does not terminate the pattern, its capture includes a trailing path
separator.  If a tree wildcard does not participate in a match, its capture is
an empty string with no path separator. Tree wildcards must be delimited by path
separators or nothing (such as the beginning and/or end of a pattern). If a glob
consists solely of a tree wildcard, then it matches all files in the working
directory tree.

The zero-or-more wildcards `*` and `$` match zero or more of any character
**except path separators**. Zero-or-more wildcards cannot be adjacent to other
zero-or-more wildcards. The `*` wildcard is eager and will match the longest
possible text while the `$` wildcard is lazy and will match the shortest
possible text. When followed by a literal, `*` stops at the last occurrence of
that literal while `$` stops at the first occurence.

The exactly-one wildcard `?` matches any single character **except path
separators**. Exactly-one wildcards do not group, so a pattern of contiguous
wildcards such as `???` form distinct captures for each `?` wildcard.

### Character Classes

Character classes match any single character from a group of literals and ranges
**except path separators**. Classes are delimited by square brackets `[...]`.
Individual character literals are specified as is, such as `[ab]` to match
either `a` or `b`. Character ranges are formed from two characters seperated by
a hyphen, such as `[x-z]` to match `x`, `y`, or `z`.

Any number of character literals and ranges can be used within a single
character class. For example, `[qa-cX-Z]` matches any of `q`, `a`, `b`, `c`,
`X`, `Y`, or `Z`.

Character classes may be negated by including an exclamation mark `!` at the
beginning of the class pattern. For example, `[!a]` matches any character except
for `a`.

Note that character classes can also be used to escape metacharacters like `*`,
`$`, etc., though globs also support escaping via a backslash `\`. To match the
control characters `[`, `]`, and `-` within a character class, they must be
escaped via a backslash, such as `[a\-]` to match `a` or `-`.

### Alternatives

Alternatives match an arbitrary sequence of comma separated sub-globs delimited
by curly braces `{...,...}`. For example, `{a?c,x?z,foo}` matches any of the
sub-globs `a?c`, `x?z`, or `foo` in order.

Alternatives form a single capture group regardless of the contents of their
sub-globs. This capture is formed from the complete match of the sub-glob, so if
the sub-glob `a?c` matches `abc` in the above example, then the capture text
will be `abc` (**not** `b` as it would be outside of an alternative sequence).

Note that alternatives may be nested, though this has no semantic effect and
matches the same way as a flattened sequence of sub-globs.

## To-Patterns

To-patterns resolve destination paths. These patterns consist of literals and
substitutions. A substitution is either a capture from a corresponding
from-pattern or file metadata. Substitutions are delimited by curly braces
`{...}`.

### Captures

Captures are typically indexed from a from-pattern using a hash followed by an
index, like `{#1}`. These indices count from one; the zero index is used for the
full text of a match. Empty braces also respresent the full text of a match, so
`{#0}` and `{}` are equivalent.

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

### Properties

Properties include source file metadata in the destination path and are
specified following an exclamation `!`. Properties are case insensitive.
Supported properties are described in the following table.

| Pattern    | Metadata                               |
|------------|----------------------------------------|
| `{!b3sum}` | [BLAKE3] hash of the source file.      |
| `{!ts}`    | Modified timestamp of the source file. |

For example, `{!b3sum}` is replaced by the [BLAKE3] hash of the matched file.

### Formatters

Substitutions support optional formatters. Formatters must appear last in a
substitution following a vertical bar `|`. Formatters are separated by commas
`,`. Any number of formatters may be used and are applied in the order in which
they appear.

The pad formatter pads substitution text to a specified width and alignment
using the given character shim. For example, `{#1|>4[0]}` pads the substition
text into four columns using right alignment and the character `0` for padding.
If the original substitution text is `13`, then it becomes `0013` after
formatting.

## Crates

Nym's core functionality is exposed as an independent library and front ends are
developed atop this library. The following table describes the official Rust
crates maintained in the [Nym repository][repository].

| Crate       | Description                                        |
|-------------|----------------------------------------------------|
| [`nym`]     | Library implementing Nym's core functionality.     |
| [`nym-bin`] | Binary for the `nym` command line interface (CLI). |

The major and minor versions of these crates are upgraded together.

## Installation

[Install Rust][rustup] and use `cargo` to install from a clone of the
repository.

```shell
git clone https://github.com/olson-sean-k/nym.git
cd nym/nym-bin
cargo install --locked --path=. --force
```

## Disclaimer

Nym is provided as is with no warranty. At the time of writing, Nym is highly
experimental and likely has many bugs. Data loss may occur. **Use at your own
risk.**

[repository]: https://github.com/olson-sean-k/nym

[BLAKE3]: https://github.com/BLAKE3-team/BLAKE3
[rustup]: https://rustup.rs/

[`nym`]: https://crates.io/crates/nym
[`nym-bin`]: https://crates.io/crates/nym-bin
