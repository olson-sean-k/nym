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

Commands that write to the file system (i.e., transforms like `copy`) are
interactive by default and print a manifest and then prompt to continue before
writing. This behavior can be controlled with the `--interactive` option.

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
patterns must be escaped to avoid interacting with shell features like
expansion. Quoting patterns usually prevents these unwanted interactions.

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

Globs are opinionated about path separators. Forward slash `/` is **always** the
path separator and back slashes `\` are forbidden (back slash is used for escape
sequences, but the literal sequence `\\` is not supported). Separators are
normalized across platforms; glob patterns can match paths on Windows, for
example.

### Wildcards

Wildcards match some amount of arbitrary text in paths and are the most
fundamental tool provided by globs.

The tree wildcard `**` matches zero or more sub-directories. **This is the only
way to match against arbitrary directories**; all other wildcards do **not**
match across directory boundaries. When a tree wildcard participates in a match
and does not terminate the pattern, its capture includes a trailing path
separator. If a tree wildcard does not participate in a match, its capture is an
empty string with no path separator. Tree wildcards must be delimited by path
separators or a termination (such as the beginning and/or end of a glob or
sub-glob). Tree wildcards cannot be adjacent to other tree wildcards. If a glob
consists solely of a tree wildcard, then it matches all files in the working
directory tree.

The zero-or-more wildcards `*` and `$` match zero or more of any character
**except path separators**. Zero-or-more wildcards cannot be adjacent to other
zero-or-more wildcards. The `*` wildcard is eager and will match the longest
possible text while the `$` wildcard is lazy and will match the shortest
possible text. When followed by a literal, `*` stops at the last occurrence of
that literal while `$` stops at the first occurence.

The exactly-one wildcard `?` matches any single character **except path
separators**. Exactly-one wildcards do not group automatically, so a pattern of
contiguous wildcards such as `???` form distinct captures for each `?` wildcard.
An alternative can be used to group exactly-one wildcards into a single capture,
such as `{???}` (see below).

### Character Classes

Character classes match any single character from a group of literals and ranges
**except path separators**. Classes are delimited by square brackets `[...]`.
Individual character literals are specified as is, such as `[ab]` to match
either `a` or `b`. Character ranges are formed from two characters separated by
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
nested, such as in `{a,{b,{c,d}}}`.

Alternatives form a single capture group regardless of the contents of their
sub-globs. This capture is formed from the complete match of the sub-glob, so if
the sub-glob `a?c` matches `abc` in `{a?c,x?z}`, then the capture text will be
`abc` (**not** `b` as it would be outside of an alternative sequence).
Alternatives can be used to group capture text using a single sub-glob, such as
`{*.{go,rs}}` to capture an entire file name with a particular extension or
`{??}` to group a sequence of exactly-one wildcards.

Sub-globs, especially those with path boundaries, must consider neighboring
patterns and have limitations. For example, wildcards and path separators
generally cannot be adjacent, so `a{b,c/**}` and `a{/b,/c}` are allowed but
`a{b,**/c}` and `a/{/b,c}` are not. Additionally, singular tree wildcards are
never allowed in alternatives, such as `{a,**}`. Such an alternative is
equivalent to a tree wildcard `**`, which should be used instead.

Regarding the above limitations, note that tree wildcards parse any surrounding
forward slashes `/`, so `a/{/**/b,c}` is allowed despite appearing to have
adjacent path separators; the leading `/` in the sub-glob `/**/b` is parsed as a
tree wildcard and **not** an independent path separator.

### Literals and Platform-specific Features

Any components not recognized by globs are interpreted as literals. In
combination with strict interpretations of path separators, this means some
platform-specific features cannot be used as part of a from-pattern.

In particular, while from-patterns can be rooted, they cannot include schemes
nor Windows path prefixes. On Windows, UNC paths or paths with other prefixes
can be used via the `--tree`/`-C` option, which establishes the directory in
which from-patterns are applied using native paths. For example, the following
command copies all files from the UNC share path `\\server\share\src`.

```shell
nym copy -p --tree=\\server\share 'src/**' 'C:\\backup\\{#1}'
```

Globs do not explicitly support the notion of a parent directory. However, any
invariant (literal) prefix is re-interpreted by the platform as a native path,
so from-patterns that begin with `..` behave as expected on Unix and Windows.
For example, the following command intuitively operates in the parent of the
current working directory.

```shell
nym find '../src/*.rs'
```

However, `..` is interpreted as a literal and when it follows variant
(non-literal) components in a glob it only matches paths with the literal
component `..`. This never occurs when traversing directory trees, **so `..`
literals following variant patterns like wildcards match nothing and should not
be used**. For example, the from-pattern `src/**/../*.rs` never yields any
matching files.

## To-Patterns

To-patterns resolve destination paths. These patterns consist of literals and
substitutions. A substitution is either a capture from a corresponding
from-pattern or a property that reads file metadata. Substitutions are delimited
by curly braces `{...}`. Literals form a native path as-is.

### Captures

Captures index a from-pattern using a hash followed by the index, like `{#1}`.
These indices count from one; the zero index is used for the full text of a
match. Empty braces also represent the full text of a match, so `{#0}` and `{}`
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

| Pattern     | Metadata               | Type / Format | Cargo Feature               |
|-------------|------------------------|---------------|-----------------------------|
| `{!b3sum}`  | [BLAKE3] hash digest   | digest        | `property-b3sum` (default)  |
| `{!ctime}`  | creation timestamp     | date-time     | n/a                         |
| `{!md5sum}` | [MD5] hash digest      | digest        | `property-md5sum` (default) |
| `{!mtime}`  | modification timestamp | date-time     | n/a                         |

For example, `{!b3sum}` is replaced by the [BLAKE3] hash digest of the matched
file.

Properties are associated with a data type and corresponding format that
transforms them into the output text of a substitution. Formats are optionally
specified after a property name following a colon `:` and delimited by square
brackets `[...]`. For example, the date-time data type uses a [`strftime`]-like
format and the pattern `{!mtime:[%Y]}` outputs the text of the four-digit year
of a source file's modification timestamp.

Properties may require additional dependencies and some can be toggled in a
build using [Cargo features][features].

### Text Formatters

Substitutions (both captures and properties) support optional text formatters.
Text formatters must appear last in a substitution following a vertical bar `|`.
Any number of text formatters may be used separated by commas `,` and they are
applied from left to right in the order in which they appear.

Text formatters are distinct from property formats and, as their name suggests,
operate exclusively on the output text of a substitution (they do not operate on
non-textual data).

The pad formatter pads substitution text to a specified width and alignment
using the given character shim. For example, `{#1|>4[0]}` pads the substitution
text into four columns using right alignment and the character `0` for padding.
If the original substitution text is `13`, then it becomes `0013` after
formatting in this example. Left and center alignment are also supported via `<`
and `^`, respectively.

There are three casing formatters: lowercase, uppercase, and titlecase, with the
case-insensitive patterns `lower`, `upper`, and `title`, respectively. These
formatters take no parameters and change the casing of supported characters.
Note that `title` is sensitive to word breaks, which only occur across
whitespace and hyphens `-` (and **not** underscores `_`, for example).

The coalesce formatter replaces matching input characters with an output
character. For example, `{#1|%[_-][~]}` replaces any instances of `_` or `-`
with a tilde `~`.

Text formatters can be combined to perform complex formatting. For example, the
following command extracts a part of file names delimited by underscores `_` and
formats that part using title casing with spaces.

```shell
nym move '$_$_*.mp4' '{#2|%[-][ ],title}.mp4'
```

Given a file named `the-show-title_the-episode-title_the-encoding.mp4`, the
above transform would move it to `The Episode Title.mp4`.

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
build and install `nym` using `cargo`. Nym requires Rust 1.56 or higher.

```shell
git clone https://github.com/olson-sean-k/nym.git
cd nym/nym-cli
cargo install --locked --path=. --force
```

By default, this will build the `master` branch, which generally tracks tested
upcoming changes. To install a specific release, checkout a version tag before
using `cargo install`. Note that the build instructions may differ between
versions; refer to the `README` in the clone.

```shell
git checkout v0.0.0
```

### Registry

To install a release of `nym` from the [crates.io] Rust package registry,
[install Rust][rustup] and then build and install `nym` using `cargo`.

```shell
cargo install nym-cli --locked --force
```

To install a specific release, use the `--version` option.

```shell
cargo install nym-cli --version=0.0.0 --locked --force
```

### Package

The following table describes packages that can be used to install `nym`.

| Platform   | Package                      |
|------------|------------------------------|
| Arch (AUR) | [`nym-git`][pkg-aur-nym-git] |

## Disclaimer

Nym is provided as is and with **no** warranty. At the time of writing, Nym is
experimental and likely has bugs. Data loss may occur. **Use at your own risk.**

[repository]: https://github.com/olson-sean-k/nym

[BLAKE3]: https://github.com/BLAKE3-team/BLAKE3
[crates.io]: https://crates.io
[features]: https://doc.rust-lang.org/cargo/reference/features.html
[MD5]: https://en.wikipedia.org/wiki/MD5
[rustup]: https://rustup.rs/
[`strftime`]: https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html

[`nym`]: https://crates.io/crates/nym
[`nym-cli`]: https://crates.io/crates/nym-cli

[pkg-aur-nym-git]: https://aur.archlinux.org/packages/nym-git/
