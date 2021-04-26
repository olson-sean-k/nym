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
copy, link, or move directories, for example.

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
resemble literal paths, but additionally support wildcards, character classes,
and alternatives that can be matched against paths on the file system. Matches
provide capture text that can be used in to-patterns.

Forward slash `/` is **always** the path separator in globs and back slashes `\`
are forbidden (back slash is used for escape sequences, but the literal sequence
`\\` is not supported). Separators are normalized across platforms; glob
patterns can match paths on Windows, for example.

On Windows, UNC paths or paths with other prefixes can be used via the
`--tree`/`-C` option. For example, the following command copies all files from
the UNC share path `\\server\share\src`.

```shell
nym copy --tree=\\server\share 'src/**' 'C:\\backup\\{#1}'
```

### Wildcards

Wildcards match some amount of arbitrary text in paths and are the most
fundamental tool provided by globs.

The tree wildcard `**` matches zero or more sub-directories. **This is the only
way to match against arbitrary directories**; all other wildcards do **not**
match across directory boundaries. When a tree wildcard participates in a match
and does not terminate the pattern, its capture includes a trailing path
separator. If a tree wildcard does not participate in a match, its capture is an
empty string with no path separator. Tree wildcards must be delimited by path
separators or nothing (such as the beginning and/or end of a glob or sub-glob).
If a glob consists solely of a tree wildcard, then it matches all files in the
working directory tree.

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
sub-globs `a?c`, `x?z`, or `foo` in order. Alternatives may be arbitrarily
nested, such as in `a{b*,c{x?z,foo},d}`.

Alternatives form a single capture group regardless of the contents of their
sub-globs. This capture is formed from the complete match of the sub-glob, so if
the sub-glob `a?c` matches `abc` in the above example, then the capture text
will be `abc` (**not** `b` as it would be outside of an alternative sequence).

Sub-globs, in particular those containing wildcards, must consider neighboring
patterns. For example, it is not possible to introduce a tree wildcard that is
adjacent to anything but a path separator or termination, so `foo{bar,baz/**}`
is allowed but `foo{bar,**/baz}` is not. Because it matches anything, singular
tree wildcards are **never** allowed in sub-globs, so `foo/{bar,**}` is
disallowed.

## To-Patterns

To-patterns resolve destination paths. These patterns consist of literals and
substitutions. A substitution is either a capture from a corresponding
from-pattern or a property that reads file metadata. Substitutions are delimited
by curly braces `{...}`.

### Captures

Captures index a from-pattern using a hash followed by the index, like `{#1}`.
These indices count from one; the zero index is used for the full text of a
match. Empty braces also respresent the full text of a match, so `{#0}` and `{}`
are equivalent.

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
specified by name following an exclamation mark `!`. Property names are case
insensitive. Supported properties are described in the following table.

| Pattern    | Metadata                               | Cargo Feature              |
|------------|----------------------------------------|----------------------------|
| `{!b3sum}` | [BLAKE3] hash of the source file.      | `property-b3sum` (default) |
| `{!ts}`    | Modified timestamp of the source file. | `property-ts` (default)    |

For example, `{!b3sum}` is replaced by the [BLAKE3] hash of the matched file.

Properties may require additional dependencies and some can be toggled in a
build using [Cargo features][features].

### Formatters

Substitutions (both captures and properties) support optional formatters.
Formatters must appear last in a substitution following a vertical bar `|`.
Formatters are separated by commas `,`. Any number of formatters may be used and
are applied in the order in which they appear.

The pad formatter pads substitution text to a specified width and alignment
using the given character shim. For example, `{#1|>4[0]}` pads the substition
text into four columns using right alignment and the character `0` for padding.
If the original substitution text is `13`, then it becomes `0013` after
formatting in this example.

## Crates

Nym's core functionality is exposed as an independent library and front ends are
developed atop this library. The following table describes the official Rust
crates maintained in the [Nym repository][repository].

| Crate       | Description                                        |
|-------------|----------------------------------------------------|
| [`nym`]     | Library implementing Nym's core functionality.     |
| [`nym-cli`] | Binary for the `nym` command line interface (CLI). |

The major and minor versions of these crates are upgraded together.

## Installation

The `nym` binary can be installed in various ways described below.

### Repository

To install `nym` from a clone of the repository, [install Rust][rustup] and then
build and install `nym` using `cargo`.

```shell
git clone https://github.com/olson-sean-k/nym.git
cd nym/nym-cli
cargo install --locked --path=. --force
```

### Registry

To install `nym` from the [crates.io] Rust package registry, [install
Rust][rustup] and then build and install `nym` using `cargo`.

```shell
cargo install nym-cli --locked --force
```

### Package

The following table describes packages that can be used to install `nym`.

| Platform          | Package                      |
|-------------------|------------------------------|
| [Arch (AUR)][AUR] | [`nym-git`][pkg-aur-nym-git] |
| [Scoop][Scoop]    | [`nym`][pkg-scoop-nym]       |

## Disclaimer

Nym is provided as is and with **no** warranty. At the time of writing, Nym is
experimental and likely has bugs. Data loss may occur. **Use at your own risk.**

[repository]: https://github.com/olson-sean-k/nym

[AUR]: https://aur.archlinux.org
[BLAKE3]: https://github.com/BLAKE3-team/BLAKE3
[crates.io]: https://crates.io
[features]: https://doc.rust-lang.org/cargo/reference/features.html
[rustup]: https://rustup.rs
[Scoop]: https://scoop.sh

[`nym`]: https://crates.io/crates/nym
[`nym-cli`]: https://crates.io/crates/nym-cli

[pkg-aur-nym-git]: https://aur.archlinux.org/packages/nym-git/
[pkg-scoop-nym]: https://github.com/ScoopInstaller/Main/blob/master/bucket/nym.json
