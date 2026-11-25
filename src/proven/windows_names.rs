//! Verified guide-name spelling rules, including the Windows name rules.
//!
//! These kernels make the pure decisions of guide-path validation in
//! `guide_input.rs` (`docs/v0.2-contract.md`, "Guide-file input": guide-name
//! and explicit-guide configuration MUST reject alternate data stream syntax,
//! reserved DOS device aliases, named-pipe or device namespaces, and
//! verbatim/device prefixes):
//!
//! - [`implicit_name_shape`]: an implicit guide name is rejected first when it
//!   is empty and then when it contains `/` or `\`;
//! - `has_namespace_prefix`: a Windows path spelling that begins with a
//!   device, named-pipe, or verbatim namespace prefix (`\\?\`, `\\.\`,
//!   `\??\`, or `\\??\`, with either separator in every separator position);
//! - `windows_component_verdict`: one Windows path component, in the exact
//!   production precedence: alternate-data-stream syntax (`:` anywhere),
//!   then a forbidden code unit (below U+0020 or one of `<>"|?*`), then a
//!   trailing space or dot, then a reserved DOS device base name;
//! - `is_non_filesystem_share`: a UNC share that names the named-pipe,
//!   mailslot, or IPC namespace, ignoring ASCII case.
//!
//! The reserved-device rule reads the host's Unicode uppercase mapping of the
//! component, which Verus does not model: the caller passes that uppercase
//! text as an input (a trusted standard-library conversion), so the
//! non-ASCII mappings that make, for example, `conın$` reserved are kept
//! exactly. The device base is the uppercase text before its first `.`, with
//! trailing spaces removed; it is reserved when it is `CON`, `PRN`, `AUX`,
//! `NUL`, `CLOCK$`, `CONIN$`, or `CONOUT$`, or `COM` or `LPT` followed by one
//! of `1`-`9`, `¹`, `²`, or `³`.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_accepted_component_is_ordinary`: an accepted component has no
//!   `:`, no code unit below U+0020, none of `<>"|?*`, does not end in a
//!   space or `.`, and its device base is not reserved;
//! - `theorem_component_precedence`: each rejection is reported exactly when
//!   every earlier rule passes, so `:` outranks every other rule;
//! - `theorem_device_base_shape`: the device base is exactly the text before
//!   the first `.` with its trailing spaces removed;
//! - `theorem_extension_does_not_unreserve`: adding an extension (or spaces
//!   before it) never changes the device base, so `CON.txt` and `CON .md` are
//!   reserved exactly like `CON`;
//! - `theorem_namespace_spellings` and `theorem_ordinary_spellings_are_not_namespaces`:
//!   every listed namespace prefix is rejected with either separator, while
//!   drive-absolute, relative, and ordinary UNC (`\\server\share`) spellings
//!   are never mistaken for one;
//! - `theorem_implicit_candidate_has_no_separator`: an implicit name that
//!   passes the shape rule is nonempty and contains neither separator.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

/// Shape of a configured implicit guide name before host path parsing.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImplicitNameShape {
    /// The name is empty.
    Empty,
    /// The name contains `/` or `\`.
    ContainsSeparator,
    /// A nonempty name without separators, still subject to host parsing.
    Candidate,
}

/// Verdict of the Windows component rules, in production precedence.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowsComponentVerdict {
    /// Every Windows component rule holds.
    Ordinary,
    /// The component contains `:` (alternate-data-stream syntax).
    AlternateDataStream,
    /// A code unit below U+0020 or one of `<`, `>`, `"`, `|`, `?`, `*`.
    ForbiddenCharacter,
    /// The component ends in a space or `.`.
    TrailingSpaceOrDot,
    /// The device base names a reserved DOS device.
    ReservedDevice,
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Implicit names
// ---------------------------------------------------------------------------

/// `/` or `\`.
pub(crate) open spec fn is_name_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

/// The shape rule of an implicit guide name: empty first, then separators.
pub(crate) open spec fn implicit_name_shape_spec(s: Seq<char>) -> ImplicitNameShape {
    if s.len() == 0 {
        ImplicitNameShape::Empty
    } else if exists|i: int| 0 <= i < s.len() && #[trigger] is_name_separator(s[i]) {
        ImplicitNameShape::ContainsSeparator
    } else {
        ImplicitNameShape::Candidate
    }
}

// ---------------------------------------------------------------------------
// Namespace prefixes (UTF-16 code units)
// ---------------------------------------------------------------------------

/// A Windows separator code unit: `\` or `/`.
pub(crate) open spec fn is_separator_unit(u: u16) -> bool {
    u == '\\' as u16 || u == '/' as u16
}

/// The spelling begins with `\\?\` or `\\.\`, `\??\`, or `\\??\`, with either
/// separator in every separator position.
pub(crate) open spec fn namespace_prefix(raw: Seq<u16>) -> bool {
    ||| (raw.len() >= 4 && is_separator_unit(raw[0]) && is_separator_unit(raw[1]) && (raw[2]
        == '.' as u16 || raw[2] == '?' as u16) && is_separator_unit(raw[3]))
    ||| (raw.len() >= 4 && is_separator_unit(raw[0]) && raw[1] == '?' as u16 && raw[2] == '?' as u16
        && is_separator_unit(raw[3]))
    ||| (raw.len() >= 5 && is_separator_unit(raw[0]) && is_separator_unit(raw[1]) && raw[2] == '?' as u16
        && raw[3] == '?' as u16 && is_separator_unit(raw[4]))
}

// ---------------------------------------------------------------------------
// Component rules
// ---------------------------------------------------------------------------

/// A code unit Windows does not allow in a filesystem name: every unit below
/// U+0020 and `<`, `>`, `"`, `|`, `?`, `*`.
pub(crate) open spec fn is_forbidden_unit(u: u16) -> bool {
    u < 32u16 || u == '<' as u16 || u == '>' as u16 || u == '"' as u16 || u == '|' as u16 || u
        == '?' as u16 || u == '*' as u16
}

/// Some code unit of `s` is `u`.
pub(crate) open spec fn has_unit(s: Seq<u16>, u: u16) -> bool {
    exists|i: int| 0 <= i < s.len() && s[i] == u
}

/// Some code unit of `s` is forbidden.
pub(crate) open spec fn has_forbidden_unit(s: Seq<u16>) -> bool {
    exists|i: int| 0 <= i < s.len() && #[trigger] is_forbidden_unit(s[i])
}

/// The last code unit is a space or `.`.
pub(crate) open spec fn ends_in_space_or_dot(s: Seq<u16>) -> bool {
    s.len() > 0 && (s.last() == ' ' as u16 || s.last() == '.' as u16)
}

/// The index of the first `c` in `s`, or `s.len()` when there is none.
pub(crate) open spec fn first_index_of(s: Seq<char>, c: char) -> int
    decreases s.len(),
{
    if s.len() == 0 {
        0
    } else if s[0] == c {
        0
    } else {
        1 + first_index_of(s.drop_first(), c)
    }
}

/// `s` without its trailing U+0020 spaces (`str::trim_end_matches(' ')`).
pub(crate) open spec fn trim_end_spaces(s: Seq<char>) -> Seq<char>
    decreases s.len(),
{
    if s.len() > 0 && s.last() == ' ' {
        trim_end_spaces(s.drop_last())
    } else {
        s
    }
}

/// The device base of an uppercase component: the text before its first
/// `.`, without trailing spaces.
pub(crate) open spec fn device_base_spec(upper: Seq<char>) -> Seq<char> {
    trim_end_spaces(upper.take(first_index_of(upper, '.')))
}

/// `1` through `9`, `¹`, `²`, or `³`.
pub(crate) open spec fn is_alias_digit(c: char) -> bool {
    ('1' <= c && c <= '9') || c == '\u{b9}' || c == '\u{b2}' || c == '\u{b3}'
}

/// `prefix` followed by exactly one alias digit.
pub(crate) open spec fn is_numbered_alias(b: Seq<char>, prefix: Seq<char>) -> bool {
    b.len() == prefix.len() + 1 && b.take(prefix.len() as int) == prefix && is_alias_digit(
        b[prefix.len() as int],
    )
}

/// The reserved DOS device base names.
pub(crate) open spec fn is_reserved_device_base_spec(b: Seq<char>) -> bool {
    ||| b == seq!['C', 'O', 'N']
    ||| b == seq!['P', 'R', 'N']
    ||| b == seq!['A', 'U', 'X']
    ||| b == seq!['N', 'U', 'L']
    ||| b == seq!['C', 'L', 'O', 'C', 'K', '$']
    ||| b == seq!['C', 'O', 'N', 'I', 'N', '$']
    ||| b == seq!['C', 'O', 'N', 'O', 'U', 'T', '$']
    ||| is_numbered_alias(b, seq!['C', 'O', 'M'])
    ||| is_numbered_alias(b, seq!['L', 'P', 'T'])
}

/// The verdict for one component given its UTF-16 code units and the
/// uppercase form of its (lossy) text, in production precedence.
pub(crate) open spec fn windows_component_verdict_spec(
    units: Seq<u16>,
    upper: Seq<char>,
) -> WindowsComponentVerdict {
    if has_unit(units, ':' as u16) {
        WindowsComponentVerdict::AlternateDataStream
    } else if has_forbidden_unit(units) {
        WindowsComponentVerdict::ForbiddenCharacter
    } else if ends_in_space_or_dot(units) {
        WindowsComponentVerdict::TrailingSpaceOrDot
    } else if is_reserved_device_base_spec(device_base_spec(upper)) {
        WindowsComponentVerdict::ReservedDevice
    } else {
        WindowsComponentVerdict::Ordinary
    }
}

// ---------------------------------------------------------------------------
// Non-filesystem shares (UTF-8 bytes, ASCII case folding)
// ---------------------------------------------------------------------------

/// `u8::to_ascii_lowercase`.
pub(crate) open spec fn ascii_lower(b: u8) -> u8 {
    if 0x41u8 <= b && b <= 0x5Au8 {
        (b + 32) as u8
    } else {
        b
    }
}

/// `<[u8]>::eq_ignore_ascii_case`: equal lengths and bytewise equal after
/// ASCII lowercasing.
pub(crate) open spec fn ascii_ci_eq(a: Seq<u8>, b: Seq<u8>) -> bool {
    a.len() == b.len() && forall|i: int| 0 <= i < a.len() ==> ascii_lower(#[trigger] a[i])
        == ascii_lower(b[i])
}

/// `pipe`
pub(crate) open spec fn pipe_name() -> Seq<u8> {
    seq![0x70u8, 0x69u8, 0x70u8, 0x65u8]
}

/// `mailslot`
pub(crate) open spec fn mailslot_name() -> Seq<u8> {
    seq![0x6Du8, 0x61u8, 0x69u8, 0x6Cu8, 0x73u8, 0x6Cu8, 0x6Fu8, 0x74u8]
}

/// `IPC$`
pub(crate) open spec fn ipc_name() -> Seq<u8> {
    seq![0x49u8, 0x50u8, 0x43u8, 0x24u8]
}

/// The share names the named-pipe, mailslot, or IPC namespace.
pub(crate) open spec fn non_filesystem_share(share: Seq<u8>) -> bool {
    ascii_ci_eq(share, pipe_name()) || ascii_ci_eq(share, mailslot_name()) || ascii_ci_eq(
        share,
        ipc_name(),
    )
}

} // verus!

/// Classify an implicit guide name: empty, containing `/` or `\`, or a
/// candidate for host component parsing.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == implicit_name_shape_spec(name@),
))]
pub(crate) fn implicit_name_shape(name: &str) -> ImplicitNameShape {
    if name.is_empty() {
        return ImplicitNameShape::Empty;
    }
    let mut chars = name.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= name@.len(),
            name@.len() > 0,
            chars.remaining() == name@.skip(index),
            forall|k: int| 0 <= k < index ==> !is_name_separator(#[trigger] name@[k]),
        decreases name@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                if ch == '/' || ch == '\\' {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(is_name_separator(name@[index]));
                    }
                    return ImplicitNameShape::ContainsSeparator;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => return ImplicitNameShape::Candidate,
        }
    }
}

/// Return whether `unit` is a Windows separator, `\` or `/`.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_separator_unit(unit),
))]
fn is_separator(unit: u16) -> bool {
    unit == '\\' as u16 || unit == '/' as u16
}

/// Return whether a raw Windows path spelling begins with a device,
/// named-pipe, or verbatim namespace prefix.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == namespace_prefix(raw@),
))]
pub(crate) fn has_namespace_prefix(raw: &[u16]) -> bool {
    let question = '?' as u16;
    (raw.len() >= 4
        && is_separator(raw[0])
        && is_separator(raw[1])
        && (raw[2] == '.' as u16 || raw[2] == question)
        && is_separator(raw[3]))
        || (raw.len() >= 4
            && is_separator(raw[0])
            && raw[1] == question
            && raw[2] == question
            && is_separator(raw[3]))
        || (raw.len() >= 5
            && is_separator(raw[0])
            && is_separator(raw[1])
            && raw[2] == question
            && raw[3] == question
            && is_separator(raw[4]))
}

/// Return whether `unit` is below U+0020 or one of `<>"|?*`.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_forbidden_unit(unit),
))]
fn is_forbidden_windows_unit(unit: u16) -> bool {
    unit < 32
        || unit == '<' as u16
        || unit == '>' as u16
        || unit == '"' as u16
        || unit == '|' as u16
        || unit == '?' as u16
        || unit == '*' as u16
}

/// Decide one Windows path component from its UTF-16 code units and the
/// Unicode uppercase form of its text, in production precedence.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == windows_component_verdict_spec(units@, uppercase@),
))]
pub(crate) fn windows_component_verdict(units: &[u16], uppercase: &str) -> WindowsComponentVerdict {
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= units.len(),
            forall|k: int| 0 <= k < index ==> units@[k] != ':' as u16,
        decreases units.len() - index
    ))]
    while index < units.len() {
        if units[index] == ':' as u16 {
            return WindowsComponentVerdict::AlternateDataStream;
        }
        index += 1;
    }

    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= units.len(),
            !has_unit(units@, ':' as u16),
            forall|k: int| 0 <= k < index ==> !is_forbidden_unit(#[trigger] units@[k]),
        decreases units.len() - index
    ))]
    while index < units.len() {
        if is_forbidden_windows_unit(units[index]) {
            return WindowsComponentVerdict::ForbiddenCharacter;
        }
        index += 1;
    }

    let len = units.len();
    if len > 0 && (units[len - 1] == ' ' as u16 || units[len - 1] == '.' as u16) {
        return WindowsComponentVerdict::TrailingSpaceOrDot;
    }

    let base = device_base(uppercase);
    if is_reserved_device_base(base.as_slice()) {
        WindowsComponentVerdict::ReservedDevice
    } else {
        WindowsComponentVerdict::Ordinary
    }
}

/// Return the scalars of `value` before its first `.`.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == value@.take(first_index_of(value@, '.')),
))]
fn before_first_dot(value: &str) -> Vec<char> {
    let mut before: Vec<char> = Vec::new();
    let mut chars = value.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= value@.len(),
            chars.remaining() == value@.skip(index),
            before@ == value@.take(index),
            forall|k: int| 0 <= k < index ==> value@[k] != '.',
        decreases value@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                if ch == '.' {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_first_index_of(value@, '.', index);
                    }
                    return before;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(value@.take(index + 1) =~= value@.take(index).push(ch));
                }
                before.push(ch);
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_first_index_of(value@, '.', index);
                }
                return before;
            }
        }
    }
}

/// Return the device base of an uppercase component: the scalars before its
/// first `.`, without trailing spaces.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == device_base_spec(uppercase@),
))]
fn device_base(uppercase: &str) -> Vec<char> {
    let mut base = before_first_dot(uppercase);
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            trim_end_spaces(base@) == device_base_spec(uppercase@),
        decreases base@.len()
    ))]
    while !base.is_empty() && base[base.len() - 1] == ' ' {
        base.pop();
    }
    base
}

/// Return whether `left` and `right` are the same scalar sequence.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (left@ == right@),
))]
fn same_scalars(left: &[char], right: &[char]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= left.len(),
            left.len() == right.len(),
            forall|k: int| 0 <= k < index ==> left@[k] == right@[k],
        decreases left.len() - index
    ))]
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(left@ =~= right@);
    }
    true
}

/// Return whether `c` is `1` through `9`, `¹`, `²`, or `³`.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_alias_digit(c),
))]
fn is_alias_digit_char(c: char) -> bool {
    matches!(c, '1'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}')
}

/// Return whether `base` is `prefix` followed by exactly one alias digit.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_numbered_alias(base@, prefix@),
))]
fn numbered_alias(base: &[char], prefix: &[char]) -> bool {
    if base.is_empty() || base.len() - 1 != prefix.len() {
        return false;
    }
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= prefix.len(),
            base.len() == prefix.len() + 1,
            forall|k: int| 0 <= k < index ==> base@[k] == prefix@[k],
        decreases prefix.len() - index
    ))]
    while index < prefix.len() {
        if base[index] != prefix[index] {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(base@.take(prefix.len() as int)[index as int] != prefix@[index as int]);
            }
            return false;
        }
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(base@.take(prefix.len() as int) =~= prefix@);
    }
    is_alias_digit_char(base[prefix.len()])
}

/// Return whether a device base names a reserved DOS device.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_reserved_device_base_spec(base@),
))]
fn is_reserved_device_base(base: &[char]) -> bool {
    let con: [char; 3] = ['C', 'O', 'N'];
    let prn: [char; 3] = ['P', 'R', 'N'];
    let aux: [char; 3] = ['A', 'U', 'X'];
    let nul: [char; 3] = ['N', 'U', 'L'];
    let clock: [char; 6] = ['C', 'L', 'O', 'C', 'K', '$'];
    let conin: [char; 6] = ['C', 'O', 'N', 'I', 'N', '$'];
    let conout: [char; 7] = ['C', 'O', 'N', 'O', 'U', 'T', '$'];
    let com: [char; 3] = ['C', 'O', 'M'];
    let lpt: [char; 3] = ['L', 'P', 'T'];
    let con = con.as_slice();
    let prn = prn.as_slice();
    let aux = aux.as_slice();
    let nul = nul.as_slice();
    let clock = clock.as_slice();
    let conin = conin.as_slice();
    let conout = conout.as_slice();
    let com = com.as_slice();
    let lpt = lpt.as_slice();
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(con@ =~= seq!['C', 'O', 'N']);
        assert(prn@ =~= seq!['P', 'R', 'N']);
        assert(aux@ =~= seq!['A', 'U', 'X']);
        assert(nul@ =~= seq!['N', 'U', 'L']);
        assert(clock@ =~= seq!['C', 'L', 'O', 'C', 'K', '$']);
        assert(conin@ =~= seq!['C', 'O', 'N', 'I', 'N', '$']);
        assert(conout@ =~= seq!['C', 'O', 'N', 'O', 'U', 'T', '$']);
        assert(com@ =~= seq!['C', 'O', 'M']);
        assert(lpt@ =~= seq!['L', 'P', 'T']);
    }
    same_scalars(base, con)
        || same_scalars(base, prn)
        || same_scalars(base, aux)
        || same_scalars(base, nul)
        || same_scalars(base, clock)
        || same_scalars(base, conin)
        || same_scalars(base, conout)
        || numbered_alias(base, com)
        || numbered_alias(base, lpt)
}

/// `u8::to_ascii_lowercase`.
// Verus has no specification for `u8::is_ascii_uppercase`, so the ASCII
// uppercase range is spelled out and verified directly.
#[allow(clippy::manual_is_ascii_check)]
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == ascii_lower(byte),
))]
fn ascii_lowercase(byte: u8) -> u8 {
    if matches!(byte, b'A'..=b'Z') {
        byte + 32
    } else {
        byte
    }
}

/// `<[u8]>::eq_ignore_ascii_case`.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == ascii_ci_eq(left@, right@),
))]
fn ascii_case_insensitive_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= left.len(),
            left.len() == right.len(),
            forall|k: int| 0 <= k < index ==> ascii_lower(#[trigger] left@[k]) == ascii_lower(right@[k]),
        decreases left.len() - index
    ))]
    while index < left.len() {
        if ascii_lowercase(left[index]) != ascii_lowercase(right[index]) {
            return false;
        }
        index += 1;
    }
    true
}

/// Return whether a UNC share's UTF-8 text names the named-pipe, mailslot,
/// or IPC namespace (`pipe`, `mailslot`, or `IPC$`, ignoring ASCII case).
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == non_filesystem_share(share@),
))]
pub(crate) fn is_non_filesystem_share(share: &[u8]) -> bool {
    let pipe: [u8; 4] = [0x70, 0x69, 0x70, 0x65];
    let mailslot: [u8; 8] = [0x6D, 0x61, 0x69, 0x6C, 0x73, 0x6C, 0x6F, 0x74];
    let ipc: [u8; 4] = [0x49, 0x50, 0x43, 0x24];
    let pipe = pipe.as_slice();
    let mailslot = mailslot.as_slice();
    let ipc = ipc.as_slice();
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(pipe@ =~= pipe_name());
        assert(mailslot@ =~= mailslot_name());
        assert(ipc@ =~= ipc_name());
    }
    ascii_case_insensitive_eq(share, pipe)
        || ascii_case_insensitive_eq(share, mailslot)
        || ascii_case_insensitive_eq(share, ipc)
}

#[cfg(verus_keep_ghost)]
verus! {

/// `first_index_of` is the unique index before which `c` does not occur and
/// at which `c` occurs (or the end).
pub(crate) proof fn lemma_first_index_of(s: Seq<char>, c: char, k: int)
    requires
        0 <= k <= s.len(),
        forall|j: int| 0 <= j < k ==> s[j] != c,
        k == s.len() || s[k] == c,
    ensures
        first_index_of(s, c) == k,
    decreases s.len(),
{
    if s.len() > 0 && s[0] != c {
        assert forall|j: int| 0 <= j < k - 1 implies s.drop_first()[j] != c by {
            assert(s.drop_first()[j] == s[j + 1]);
        }
        if k < s.len() {
            assert(s.drop_first()[k - 1] == s[k]);
        }
        lemma_first_index_of(s.drop_first(), c, k - 1);
    }
}

pub(crate) proof fn lemma_first_index_of_bounds(s: Seq<char>, c: char)
    ensures
        0 <= first_index_of(s, c) <= s.len(),
        forall|j: int| 0 <= j < first_index_of(s, c) ==> s[j] != c,
        first_index_of(s, c) == s.len() || s[first_index_of(s, c)] == c,
    decreases s.len(),
{
    if s.len() > 0 && s[0] != c {
        lemma_first_index_of_bounds(s.drop_first(), c);
        assert forall|j: int| 0 <= j < first_index_of(s, c) implies s[j] != c by {
            if j > 0 {
                assert(s[j] == s.drop_first()[j - 1]);
            }
        }
    }
}

/// `trim_end_spaces(s)` is the prefix `s[..m]` such that `s[m..]` is all
/// spaces and `s[m - 1]` (if any) is not a space.
proof fn lemma_trim_end_spaces(s: Seq<char>)
    ensures
        trim_end_spaces(s).len() <= s.len(),
        trim_end_spaces(s) == s.take(trim_end_spaces(s).len() as int),
        forall|j: int| trim_end_spaces(s).len() <= j < s.len() ==> s[j] == ' ',
        trim_end_spaces(s).len() == 0 || trim_end_spaces(s).last() != ' ',
    decreases s.len(),
{
    if s.len() > 0 && s.last() == ' ' {
        lemma_trim_end_spaces(s.drop_last());
        let t = trim_end_spaces(s);
        assert(t == s.drop_last().take(t.len() as int));
        assert(t =~= s.take(t.len() as int));
        assert forall|j: int| t.len() <= j < s.len() implies s[j] == ' ' by {
            if j < s.len() - 1 {
                assert(s[j] == s.drop_last()[j]);
            }
        }
    } else {
        assert(s.take(s.len() as int) =~= s);
    }
}

/// Appending spaces never changes the trimmed text.
proof fn lemma_trim_appended_spaces(s: Seq<char>, n: nat)
    ensures
        trim_end_spaces(s + Seq::new(n, |_i: int| ' ')) == trim_end_spaces(s),
    decreases n,
{
    let spaces = Seq::new(n, |_i: int| ' ');
    if n == 0 {
        assert(s + spaces =~= s);
    } else {
        let shorter = Seq::new((n - 1) as nat, |_i: int| ' ');
        assert((s + spaces).drop_last() =~= s + shorter);
        assert((s + spaces).last() == ' ');
        lemma_trim_appended_spaces(s, (n - 1) as nat);
    }
}

// ---------------------------------------------------------------------------
// Theorems
// ---------------------------------------------------------------------------

/// An accepted component has no `:`, no code unit below U+0020, none of
/// `<>"|?*`, does not end in a space or `.`, and its device base is not a
/// reserved DOS device name.
pub(crate) proof fn theorem_accepted_component_is_ordinary(units: Seq<u16>, upper: Seq<char>)
    requires
        windows_component_verdict_spec(units, upper) == WindowsComponentVerdict::Ordinary,
    ensures
        forall|i: int|
            0 <= i < units.len() ==> {
                &&& #[trigger] units[i] != ':' as u16
                &&& units[i] >= 32u16
                &&& units[i] != '<' as u16
                &&& units[i] != '>' as u16
                &&& units[i] != '"' as u16
                &&& units[i] != '|' as u16
                &&& units[i] != '?' as u16
                &&& units[i] != '*' as u16
            },
        units.len() > 0 ==> units.last() != ' ' as u16 && units.last() != '.' as u16,
        !is_reserved_device_base_spec(device_base_spec(upper)),
{
    assert forall|i: int| 0 <= i < units.len() implies {
        &&& #[trigger] units[i] != ':' as u16
        &&& units[i] >= 32u16
        &&& units[i] != '<' as u16
        &&& units[i] != '>' as u16
        &&& units[i] != '"' as u16
        &&& units[i] != '|' as u16
        &&& units[i] != '?' as u16
        &&& units[i] != '*' as u16
    } by {
        assert(!is_forbidden_unit(units[i]));
    }
}

/// The verdict precedence: `:` decides first, then a forbidden unit, then a
/// trailing space or dot, and only then the device base.
pub(crate) proof fn theorem_component_precedence(units: Seq<u16>, upper: Seq<char>)
    ensures
        (windows_component_verdict_spec(units, upper) == WindowsComponentVerdict::AlternateDataStream)
            == has_unit(units, ':' as u16),
        (windows_component_verdict_spec(units, upper) == WindowsComponentVerdict::ForbiddenCharacter)
            == (!has_unit(units, ':' as u16) && has_forbidden_unit(units)),
        (windows_component_verdict_spec(units, upper) == WindowsComponentVerdict::TrailingSpaceOrDot)
            == (!has_unit(units, ':' as u16) && !has_forbidden_unit(units) && ends_in_space_or_dot(
            units,
        )),
        (windows_component_verdict_spec(units, upper) == WindowsComponentVerdict::ReservedDevice)
            == (!has_unit(units, ':' as u16) && !has_forbidden_unit(units) && !ends_in_space_or_dot(
            units,
        ) && is_reserved_device_base_spec(device_base_spec(upper))),
{
}

/// `upper[..k]` is the text before the first `.` (or all of `upper`), and
/// `upper[..m]` is that text without its trailing spaces.
pub(crate) open spec fn base_split(upper: Seq<char>, k: int, m: int) -> bool {
    &&& 0 <= m <= k <= upper.len()
    &&& (k == upper.len() || upper[k] == '.')
    &&& forall|j: int| 0 <= j < k ==> upper[j] != '.'
    &&& forall|j: int| m <= j < k ==> upper[j] == ' '
    &&& (m == 0 || upper[m - 1] != ' ')
}

/// The device base is the text before the first `.` with its trailing spaces
/// removed: some prefix `upper[..m]` of the dot-free text `upper[..k]`, where
/// `upper[m..k]` is all spaces and `upper[m - 1]` (if any) is not a space.
pub(crate) proof fn theorem_device_base_shape(upper: Seq<char>)
    ensures
        exists|k: int, m: int|
            #[trigger] base_split(upper, k, m) && device_base_spec(upper) == upper.take(m),
{
    lemma_first_index_of_bounds(upper, '.');
    let k = first_index_of(upper, '.');
    let before = upper.take(k);
    lemma_trim_end_spaces(before);
    let m = trim_end_spaces(before).len() as int;
    assert(before.take(m) =~= upper.take(m));
    assert forall|j: int| m <= j < k implies upper[j] == ' ' by {
        assert(before[j] == upper[j]);
    }
    if m > 0 {
        assert(trim_end_spaces(before).last() == upper[m - 1]);
    }
    assert(forall|j: int| 0 <= j < k ==> upper[j] != '.');
    assert(base_split(upper, k, m));
}

/// An extension (and any spaces before its dot) never changes the device
/// base, so `CON.txt` and `CON .md` are reserved exactly like `CON`.
pub(crate) proof fn theorem_extension_does_not_unreserve(stem: Seq<char>, spaces: nat, extension: Seq<char>)
    requires
        forall|j: int| 0 <= j < stem.len() ==> stem[j] != '.',
    ensures
        device_base_spec(stem + Seq::new(spaces, |_i: int| ' ') + seq!['.'] + extension)
            == trim_end_spaces(stem),
        device_base_spec(stem) == trim_end_spaces(stem),
{
    let padded = stem + Seq::new(spaces, |_i: int| ' ');
    let whole = padded + seq!['.'] + extension;
    assert forall|j: int| 0 <= j < padded.len() implies whole[j] != '.' by {
        assert(whole[j] == padded[j]);
        if j >= stem.len() {
            assert(padded[j] == ' ');
        } else {
            assert(padded[j] == stem[j]);
        }
    }
    assert(whole[padded.len() as int] == '.');
    lemma_first_index_of(whole, '.', padded.len() as int);
    assert(whole.take(padded.len() as int) =~= padded);
    lemma_trim_appended_spaces(stem, spaces);
    lemma_first_index_of(stem, '.', stem.len() as int);
    assert(stem.take(stem.len() as int) =~= stem);
}

/// Every listed namespace prefix is rejected, with either separator in every
/// separator position and whatever follows it.
pub(crate) proof fn theorem_namespace_spellings(a: u16, b: u16, c: u16, d: u16, rest: Seq<u16>)
    requires
        is_separator_unit(a),
        is_separator_unit(b),
        is_separator_unit(c),
        is_separator_unit(d),
    ensures
        namespace_prefix(seq![a, b, '?' as u16, c] + rest),
        namespace_prefix(seq![a, b, '.' as u16, c] + rest),
        namespace_prefix(seq![a, '?' as u16, '?' as u16, c] + rest),
        namespace_prefix(seq![a, b, '?' as u16, '?' as u16, d] + rest),
{
    let s1 = seq![a, b, '?' as u16, c] + rest;
    let s2 = seq![a, b, '.' as u16, c] + rest;
    let s3 = seq![a, '?' as u16, '?' as u16, c] + rest;
    let s4 = seq![a, b, '?' as u16, '?' as u16, d] + rest;
    assert(s1[0] == a && s1[1] == b && s1[2] == '?' as u16 && s1[3] == c);
    assert(s2[0] == a && s2[1] == b && s2[2] == '.' as u16 && s2[3] == c);
    assert(s3[0] == a && s3[1] == '?' as u16 && s3[2] == '?' as u16 && s3[3] == c);
    assert(s4[0] == a && s4[1] == b && s4[2] == '?' as u16 && s4[3] == '?' as u16 && s4[4] == d);
}

/// Drive-absolute and relative spellings (whose first unit is not a
/// separator), ordinary UNC spellings `\\server...` (whose server does not
/// begin with `.` or `?`), and current-drive-rooted spellings `\name` (whose
/// second unit is not `?`) are never treated as namespace spellings.
pub(crate) proof fn theorem_ordinary_spellings_are_not_namespaces(raw: Seq<u16>)
    ensures
        raw.len() > 0 && !is_separator_unit(raw[0]) ==> !namespace_prefix(raw),
        raw.len() > 2 && is_separator_unit(raw[0]) && is_separator_unit(raw[1]) && raw[2] != '.' as u16
            && raw[2] != '?' as u16 ==> !namespace_prefix(raw),
        raw.len() > 1 && is_separator_unit(raw[0]) && !is_separator_unit(raw[1]) && raw[1]
            != '?' as u16 ==> !namespace_prefix(raw),
{
}

/// An implicit name that passes the shape rule is nonempty and contains
/// neither `/` nor `\`.
pub(crate) proof fn theorem_implicit_candidate_has_no_separator(s: Seq<char>)
    requires
        implicit_name_shape_spec(s) == ImplicitNameShape::Candidate,
    ensures
        s.len() > 0,
        forall|i: int| 0 <= i < s.len() ==> s[i] != '/' && s[i] != '\\',
{
    assert forall|i: int| 0 <= i < s.len() implies s[i] != '/' && s[i] != '\\' by {
        assert(!is_name_separator(s[i]));
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn units(value: &str) -> Vec<u16> {
        value.encode_utf16().collect()
    }

    fn verdict(value: &str) -> WindowsComponentVerdict {
        windows_component_verdict(&units(value), &value.to_uppercase())
    }

    #[test]
    fn implicit_name_shape_covers_every_branch() {
        assert_eq!(implicit_name_shape(""), ImplicitNameShape::Empty);
        assert_eq!(
            implicit_name_shape("a/b"),
            ImplicitNameShape::ContainsSeparator
        );
        assert_eq!(
            implicit_name_shape("a\\b"),
            ImplicitNameShape::ContainsSeparator
        );
        assert_eq!(
            implicit_name_shape("/"),
            ImplicitNameShape::ContainsSeparator
        );
        assert_eq!(
            implicit_name_shape("GUIDE.md"),
            ImplicitNameShape::Candidate
        );
        assert_eq!(implicit_name_shape("é"), ImplicitNameShape::Candidate);
        for name in ["", "a", "a/b", "\\", "x\\", "..", "名前"] {
            let expected = if name.is_empty() {
                ImplicitNameShape::Empty
            } else if name.contains(['/', '\\']) {
                ImplicitNameShape::ContainsSeparator
            } else {
                ImplicitNameShape::Candidate
            };
            assert_eq!(implicit_name_shape(name), expected, "{name:?}");
        }
    }

    fn reference_namespace(raw: &[u16]) -> bool {
        let separator = |unit| unit == u16::from(b'\\') || unit == u16::from(b'/');
        (raw.len() >= 4
            && separator(raw[0])
            && separator(raw[1])
            && matches!(raw[2], 46 | 63)
            && separator(raw[3]))
            || (raw.len() >= 4
                && separator(raw[0])
                && raw[1] == u16::from(b'?')
                && raw[2] == u16::from(b'?')
                && separator(raw[3]))
            || (raw.len() >= 5
                && separator(raw[0])
                && separator(raw[1])
                && raw[2] == u16::from(b'?')
                && raw[3] == u16::from(b'?')
                && separator(raw[4]))
    }

    #[test]
    fn namespace_prefix_matches_reference_exhaustively() {
        let alphabet = [
            u16::from(b'\\'),
            u16::from(b'/'),
            u16::from(b'?'),
            u16::from(b'.'),
            u16::from(b'a'),
        ];
        let mut spellings: Vec<Vec<u16>> = vec![Vec::new()];
        let mut frontier: Vec<Vec<u16>> = vec![Vec::new()];
        for _ in 0..5 {
            let mut longer = Vec::new();
            for spelling in &frontier {
                for unit in alphabet {
                    let mut next = spelling.clone();
                    next.push(unit);
                    longer.push(next);
                }
            }
            spellings.extend(longer.iter().cloned());
            frontier = longer;
        }
        assert_eq!(spellings.len(), 3906);
        for spelling in &spellings {
            assert_eq!(
                has_namespace_prefix(spelling),
                reference_namespace(spelling),
                "{spelling:?}"
            );
        }
        for spelling in [
            "\\\\?\\C:\\x",
            "//./pipe/x",
            "\\??\\C:",
            "\\\\??\\x",
            "/\\?/x",
        ] {
            assert!(has_namespace_prefix(&units(spelling)), "{spelling}");
        }
        for spelling in [
            "C:\\x",
            "\\\\server\\share",
            "\\x",
            "relative",
            "",
            "\\\\?",
            "\\\\??",
        ] {
            assert!(!has_namespace_prefix(&units(spelling)), "{spelling}");
        }
    }

    #[test]
    fn component_verdict_covers_every_rule_in_precedence_order() {
        assert_eq!(verdict("guide.md"), WindowsComponentVerdict::Ordinary);
        assert_eq!(verdict("a:b"), WindowsComponentVerdict::AlternateDataStream);
        assert_eq!(
            verdict("a<b:"),
            WindowsComponentVerdict::AlternateDataStream
        );
        assert_eq!(
            verdict("CON:"),
            WindowsComponentVerdict::AlternateDataStream
        );
        for forbidden in [
            "a<b", "a>b", "a\"b", "a|b", "a?b", "a*b", "a\u{1}b", "\u{1f}", "\t",
        ] {
            assert_eq!(
                verdict(forbidden),
                WindowsComponentVerdict::ForbiddenCharacter,
                "{forbidden:?}"
            );
        }
        assert_eq!(verdict("a*."), WindowsComponentVerdict::ForbiddenCharacter);
        assert_eq!(
            verdict("name."),
            WindowsComponentVerdict::TrailingSpaceOrDot
        );
        assert_eq!(
            verdict("name "),
            WindowsComponentVerdict::TrailingSpaceOrDot
        );
        assert_eq!(verdict("CON."), WindowsComponentVerdict::TrailingSpaceOrDot);
        for reserved in [
            "con",
            "CON",
            "Con.txt",
            "con .md",
            "con  .tar.gz",
            "prn",
            "aux",
            "nul",
            "clock$",
            "conin$",
            "CONOUT$",
            "com1",
            "COM9",
            "lpt5",
            "com¹",
            "LPT²",
            "lpt³",
            "conın$",
        ] {
            assert_eq!(
                verdict(reserved),
                WindowsComponentVerdict::ReservedDevice,
                "{reserved:?}"
            );
        }
        for ordinary in [
            "console", "com0", "com10", "lpt", "com", "comx", "nul1", "x.con", "con x", "clock",
            "conin", "conout$x", "com⁴", "a", "é", "lpt¹x", " con",
        ] {
            assert_eq!(
                verdict(ordinary),
                WindowsComponentVerdict::Ordinary,
                "{ordinary:?}"
            );
        }
        assert_eq!(
            windows_component_verdict(&[], ""),
            WindowsComponentVerdict::Ordinary
        );
    }

    #[test]
    fn device_base_trims_only_before_the_first_dot() {
        assert_eq!(device_base("CON  .TXT"), vec!['C', 'O', 'N']);
        assert_eq!(device_base("CON.TXT.  "), vec!['C', 'O', 'N']);
        assert_eq!(device_base("  "), Vec::<char>::new());
        assert_eq!(device_base(".CON"), Vec::<char>::new());
        assert_eq!(device_base("A B "), vec!['A', ' ', 'B']);
        assert!(same_scalars(&['a'], &['a']));
        assert!(!same_scalars(&['a'], &['b']));
        assert!(!same_scalars(&['a'], &['a', 'b']));
        assert!(numbered_alias(&['C', 'O', 'M', '1'], &['C', 'O', 'M']));
        assert!(!numbered_alias(&[], &['C', 'O', 'M']));
        assert!(!numbered_alias(&['C', 'O', 'X', '1'], &['C', 'O', 'M']));
        assert!(!numbered_alias(&['C', 'O', 'M', '0'], &['C', 'O', 'M']));
    }

    #[test]
    fn non_filesystem_shares_ignore_ascii_case_only() {
        for share in [
            "pipe", "PIPE", "PiPe", "mailslot", "MailSlot", "IPC$", "ipc$",
        ] {
            assert!(is_non_filesystem_share(share.as_bytes()), "{share}");
            assert!(
                share.eq_ignore_ascii_case("pipe")
                    || share.eq_ignore_ascii_case("mailslot")
                    || share.eq_ignore_ascii_case("IPC$")
            );
        }
        for share in [
            "pipes",
            "pip",
            "share",
            "",
            "ipc",
            "ıpc$",
            "PİPE",
            "mail slot",
        ] {
            assert!(!is_non_filesystem_share(share.as_bytes()), "{share}");
        }
        assert_eq!(ascii_lowercase(b'A'), b'a');
        assert_eq!(ascii_lowercase(b'z'), b'z');
        assert_eq!(ascii_lowercase(b'@'), b'@');
        assert_eq!(ascii_lowercase(b'['), b'[');
        assert!(ascii_case_insensitive_eq(b"aB", b"Ab"));
        assert!(!ascii_case_insensitive_eq(b"a", b"ab"));
        assert!(!ascii_case_insensitive_eq(b"a", b"b"));
    }
}
