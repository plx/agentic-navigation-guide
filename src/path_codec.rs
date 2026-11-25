//! Exact path-component codecs shared by the parser, validator, verifier,
//! and dumper.
//!
//! Every function here except [`render_os_component`] is verified by Verus
//! against the v0.2 contract (`docs/v0.2-contract.md`, "Path expressions",
//! "Filesystem-name representation", and "Diagnostic confidentiality"):
//!
//! - [`contains_forbidden_control`] and [`has_windows_drive_prefix`] decide
//!   exactly the scalar classes that the contract names (the latter is proven
//!   over scalars although it inspects UTF-8 bytes);
//! - [`serialize_component`] emits the canonical bare or whole-quoted form.
//!   `theorem_serialized_has_no_control`, `theorem_quoted_component_decodes`,
//!   and `theorem_serialize_injective` prove that its output never contains a
//!   forbidden control, that a decoder for the contract's `quoted-path`
//!   grammar recovers the exact name from the quoted form and stops at its
//!   closing quote, and that serialization is injective;
//! - [`render_utf8_component`] and the non-UTF-8 renderers `render_raw_bytes`
//!   (Unix bytes) and `render_wide_units` (Windows code units) emit the
//!   reversible double-quoted diagnostic forms. The `theorem_rendered_*`
//!   theorems prove that each contains no C0 control and no DEL, that it is
//!   delimited by double quotes, and that a decoder recovers the exact input
//!   from it, so distinct names never share a diagnostic.
//!
//! [`render_os_component`] inspects host `OsStr` representations, which
//! Verus does not model; it only selects which verified renderer runs.

use std::ffi::OsStr;

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;
#[cfg(verus_keep_ghost)]
use vstd::utf8::{
    encode_scalar, encode_utf8, has_width_1_encoding, has_width_2_encoding, has_width_3_encoding,
    is_scalar, leading_byte_width_1, leading_byte_width_2, leading_byte_width_3,
    leading_byte_width_4,
};

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Component classes ("Path expressions")
// ---------------------------------------------------------------------------

/// NUL, CR, LF, HTAB, every other C0 control (U+0000 through U+001F), and
/// DEL (U+007F): the scalars that no decoded path component may contain.
pub(crate) open spec fn is_forbidden_control(c: char) -> bool {
    c <= '\u{1f}' || c == '\u{7f}'
}

/// Some scalar of `s` is a forbidden control.
pub(crate) open spec fn has_forbidden_control(s: Seq<char>) -> bool {
    exists|i: int| 0 <= i < s.len() && #[trigger] is_forbidden_control(s[i])
}

/// ASCII `[A-Za-z]`.
pub(crate) open spec fn is_ascii_letter(c: char) -> bool {
    ('A' <= c && c <= 'Z') || ('a' <= c && c <= 'z')
}

/// ASCII `[A-Za-z]` as a UTF-8 byte.
pub(crate) open spec fn is_ascii_letter_byte(b: u8) -> bool {
    (0x41u8 <= b && b <= 0x5Au8) || (0x61u8 <= b && b <= 0x7Au8)
}

/// The component begins with an ASCII letter followed by `:`, the shape of
/// every Windows drive-relative and drive-absolute prefix.
pub(crate) open spec fn has_drive_prefix(s: Seq<char>) -> bool {
    s.len() >= 2 && is_ascii_letter(s[0]) && s[1] == ':'
}

// ---------------------------------------------------------------------------
// Canonical serialization ("Filesystem-name representation")
// ---------------------------------------------------------------------------

/// `#`, `\`, `[`, `]`, comma, or double quote: scalars that canonical
/// generation never emits in a bare component.
pub(crate) open spec fn is_bare_syntax_char(c: char) -> bool {
    c == '#' || c == '\\' || c == '[' || c == ']' || c == ',' || c == '"'
}

/// The reserved placeholder spelling `...`.
pub(crate) open spec fn is_reserved_ellipsis(s: Seq<char>) -> bool {
    s == seq!['.', '.', '.']
}

/// Canonical generation emits the component as a whole quoted path: it is
/// `...`, has a leading or trailing U+0020, or contains a bare syntax scalar.
pub(crate) open spec fn needs_quotes(s: Seq<char>) -> bool {
    ||| is_reserved_ellipsis(s)
    ||| (s.len() > 0 && s[0] == ' ')
    ||| (s.len() > 0 && s[s.len() - 1] == ' ')
    ||| exists|i: int| 0 <= i < s.len() && #[trigger] is_bare_syntax_char(s[i])
}

/// One scalar inside a canonical whole quoted path: `"` and `\` are escaped
/// with a backslash, and every other scalar is literal.
pub(crate) open spec fn quote_escape_char(c: char) -> Seq<char> {
    if c == '"' || c == '\\' {
        seq!['\\', c]
    } else {
        seq![c]
    }
}

pub(crate) open spec fn quote_escape_fn() -> spec_fn(char) -> Seq<char> {
    |c: char| quote_escape_char(c)
}

/// The escaped scalars between the quotes of a whole quoted path.
pub(crate) open spec fn quote_escape(s: Seq<char>) -> Seq<char> {
    s.flat_map(quote_escape_fn())
}

/// The canonical whole quoted path for one component.
pub(crate) open spec fn quoted_component(s: Seq<char>) -> Seq<char> {
    seq!['"'] + quote_escape(s) + seq!['"']
}

/// The canonical serialization of one component: `None` when the component
/// is unrepresentable (empty or containing a forbidden control), the whole
/// quoted form when [`needs_quotes`], and otherwise the component itself.
pub(crate) open spec fn serialized_component(s: Seq<char>) -> Option<Seq<char>> {
    if s.len() == 0 || has_forbidden_control(s) {
        None
    } else if needs_quotes(s) {
        Some(quoted_component(s))
    } else {
        Some(s)
    }
}

/// The scalar view of an optional executable string.
pub(crate) open spec fn option_string_view(r: Option<String>) -> Option<Seq<char>> {
    match r {
        Some(s) => Some(s@),
        None => None,
    }
}

/// Decodes `*quoted-path-char DQUOTE`, the text after the opening quote of a
/// `quoted-path`:
///
/// ```text
/// quoted-path-char   = quoted-plain / quoted-path-escape
/// quoted-path-escape = BACKSLASH (DQUOTE / BACKSLASH)
/// quoted-plain       = path-source-character except DQUOTE / BACKSLASH
///                      / line-ending
/// ```
///
/// Returns the decoded scalars and the text after the closing quote, or
/// `None` when `t` is not of that form.
pub(crate) open spec fn decode_quoted_chars(t: Seq<char>) -> Option<(Seq<char>, Seq<char>)>
    decreases t.len(),
{
    if t.len() == 0 {
        None
    } else if t[0] == '"' {
        Some((Seq::empty(), t.drop_first()))
    } else if t[0] == '\\' {
        if t.len() >= 2 && (t[1] == '"' || t[1] == '\\') {
            match decode_quoted_chars(t.subrange(2, t.len() as int)) {
                Some(r) => Some((seq![t[1]] + r.0, r.1)),
                None => None,
            }
        } else {
            None
        }
    } else if t[0] == '\r' || t[0] == '\n' {
        None
    } else {
        match decode_quoted_chars(t.drop_first()) {
            Some(r) => Some((seq![t[0]] + r.0, r.1)),
            None => None,
        }
    }
}

/// Decodes `quoted-path = DQUOTE 1*quoted-path-char DQUOTE` at the start of
/// `t`, returning the decoded scalars and the text after the closing quote.
pub(crate) open spec fn decode_quoted_path(t: Seq<char>) -> Option<(Seq<char>, Seq<char>)> {
    if t.len() > 0 && t[0] == '"' {
        match decode_quoted_chars(t.drop_first()) {
            Some(r) => if r.0.len() > 0 {
                Some(r)
            } else {
                None
            },
            None => None,
        }
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Reversible diagnostic rendering ("Filesystem-name representation" and
// "Diagnostic confidentiality")
// ---------------------------------------------------------------------------

/// The uppercase hexadecimal digit for `n < 16`.
pub(crate) open spec fn hex_digit(n: u32) -> char {
    seq!['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F'][n as int]
}

/// The value of an uppercase hexadecimal digit.
pub(crate) open spec fn hex_value(c: char) -> Option<u32> {
    if '0' <= c && c <= '9' {
        Some((c as u32 - '0' as u32) as u32)
    } else if 'A' <= c && c <= 'F' {
        Some((c as u32 - 'A' as u32 + 10) as u32)
    } else {
        None
    }
}

/// `\u{XXXX}`: the four uppercase hexadecimal digits of `code`.
pub(crate) open spec fn unicode_escape(code: u16) -> Seq<char> {
    let v = code as u32;
    seq![
        '\\',
        'u',
        '{',
        hex_digit((v >> 12u32) & 0xFu32),
        hex_digit((v >> 8u32) & 0xFu32),
        hex_digit((v >> 4u32) & 0xFu32),
        hex_digit(v & 0xFu32),
        '}',
    ]
}

/// `\xNN`: the two uppercase hexadecimal digits of `byte`.
pub(crate) open spec fn byte_escape(byte: u8) -> Seq<char> {
    let v = byte as u32;
    seq!['\\', 'x', hex_digit((v >> 4u32) & 0xFu32), hex_digit(v & 0xFu32)]
}

/// The diagnostic rendering of one valid UTF-8 scalar: `\"`, `\\`, `\0`,
/// `\t`, `\n`, and `\r`; uppercase `\u{XXXX}` for every other C0 control and
/// DEL; and the scalar itself otherwise.
pub(crate) open spec fn render_char(c: char) -> Seq<char> {
    if c == '"' {
        seq!['\\', '"']
    } else if c == '\\' {
        seq!['\\', '\\']
    } else if c == '\0' {
        seq!['\\', '0']
    } else if c == '\t' {
        seq!['\\', 't']
    } else if c == '\n' {
        seq!['\\', 'n']
    } else if c == '\r' {
        seq!['\\', 'r']
    } else if is_forbidden_control(c) {
        unicode_escape(c as u16)
    } else {
        seq![c]
    }
}

pub(crate) open spec fn render_char_fn() -> spec_fn(char) -> Seq<char> {
    |c: char| render_char(c)
}

pub(crate) open spec fn byte_escape_fn() -> spec_fn(u8) -> Seq<char> {
    |b: u8| byte_escape(b)
}

pub(crate) open spec fn unit_escape_fn() -> spec_fn(u16) -> Seq<char> {
    |u: u16| unicode_escape(u)
}

/// The reversible diagnostic form of a valid UTF-8 name.
pub(crate) open spec fn rendered_component(s: Seq<char>) -> Seq<char> {
    seq!['"'] + s.flat_map(render_char_fn()) + seq!['"']
}

/// The diagnostic form of a non-UTF-8 Unix name: every raw byte as `\xNN`.
pub(crate) open spec fn rendered_bytes(bytes: Seq<u8>) -> Seq<char> {
    seq!['"'] + bytes.flat_map(byte_escape_fn()) + seq!['"']
}

/// The diagnostic form of an ill-formed Windows name: every UTF-16 code unit
/// as `\u{XXXX}`.
pub(crate) open spec fn rendered_units(units: Seq<u16>) -> Seq<char> {
    seq!['"'] + units.flat_map(unit_escape_fn()) + seq!['"']
}

/// The scalar denoted by a one-character escape after a backslash.
pub(crate) open spec fn simple_unescape(e: char) -> Option<char> {
    if e == '"' {
        Some('"')
    } else if e == '\\' {
        Some('\\')
    } else if e == '0' {
        Some('\0')
    } else if e == 't' {
        Some('\t')
    } else if e == 'n' {
        Some('\n')
    } else if e == 'r' {
        Some('\r')
    } else {
        None
    }
}

/// The code unit denoted by a `\u{XXXX}` escape at the start of `t`.
pub(crate) open spec fn decode_unicode_escape(t: Seq<char>) -> Option<u16> {
    if t.len() >= 8 && t[0] == '\\' && t[1] == 'u' && t[2] == '{' && t[7] == '}' {
        match (hex_value(t[3]), hex_value(t[4]), hex_value(t[5]), hex_value(t[6])) {
            (Some(a), Some(b), Some(c), Some(d)) => Some(
                ((a << 12u32) | (b << 8u32) | (c << 4u32) | d) as u16,
            ),
            _ => None,
        }
    } else {
        None
    }
}

/// The byte denoted by a `\xNN` escape at the start of `t`.
pub(crate) open spec fn decode_byte_escape(t: Seq<char>) -> Option<u8> {
    if t.len() >= 4 && t[0] == '\\' && t[1] == 'x' {
        match (hex_value(t[2]), hex_value(t[3])) {
            (Some(a), Some(b)) => Some(((a << 4u32) | b) as u8),
            _ => None,
        }
    } else {
        None
    }
}

/// Decodes the text after the opening quote of a rendered UTF-8 name through
/// its closing quote, returning the scalars and the text that follows.
/// Raw forbidden controls are never accepted.
pub(crate) open spec fn decode_rendered_chars(t: Seq<char>) -> Option<(Seq<char>, Seq<char>)>
    decreases t.len(),
{
    if t.len() == 0 {
        None
    } else if t[0] == '"' {
        Some((Seq::empty(), t.drop_first()))
    } else if t[0] == '\\' {
        if t.len() >= 2 && simple_unescape(t[1]) is Some {
            match decode_rendered_chars(t.subrange(2, t.len() as int)) {
                Some(r) => Some((seq![simple_unescape(t[1])->0] + r.0, r.1)),
                None => None,
            }
        } else if t.len() >= 8 && decode_unicode_escape(t) is Some
            && decode_unicode_escape(t)->0 <= 0x7Fu16
        {
            match decode_rendered_chars(t.subrange(8, t.len() as int)) {
                Some(r) => Some((seq![(decode_unicode_escape(t)->0 as u32) as char] + r.0, r.1)),
                None => None,
            }
        } else {
            None
        }
    } else if is_forbidden_control(t[0]) {
        None
    } else {
        match decode_rendered_chars(t.drop_first()) {
            Some(r) => Some((seq![t[0]] + r.0, r.1)),
            None => None,
        }
    }
}

/// Decodes a rendered UTF-8 name at the start of `t`.
pub(crate) open spec fn decode_rendered(t: Seq<char>) -> Option<(Seq<char>, Seq<char>)> {
    if t.len() > 0 && t[0] == '"' {
        decode_rendered_chars(t.drop_first())
    } else {
        None
    }
}

/// Decodes the text after the opening quote of a rendered raw-byte name.
pub(crate) open spec fn decode_byte_escapes(t: Seq<char>) -> Option<(Seq<u8>, Seq<char>)>
    decreases t.len(),
{
    if t.len() == 0 {
        None
    } else if t[0] == '"' {
        Some((Seq::empty(), t.drop_first()))
    } else if t.len() >= 4 && decode_byte_escape(t) is Some {
        match decode_byte_escapes(t.subrange(4, t.len() as int)) {
            Some(r) => Some((seq![decode_byte_escape(t)->0] + r.0, r.1)),
            None => None,
        }
    } else {
        None
    }
}

/// Decodes a rendered raw-byte name at the start of `t`.
pub(crate) open spec fn decode_rendered_bytes(t: Seq<char>) -> Option<(Seq<u8>, Seq<char>)> {
    if t.len() > 0 && t[0] == '"' {
        decode_byte_escapes(t.drop_first())
    } else {
        None
    }
}

/// Decodes the text after the opening quote of a rendered code-unit name.
pub(crate) open spec fn decode_unit_escapes(t: Seq<char>) -> Option<(Seq<u16>, Seq<char>)>
    decreases t.len(),
{
    if t.len() == 0 {
        None
    } else if t[0] == '"' {
        Some((Seq::empty(), t.drop_first()))
    } else if t.len() >= 8 && decode_unicode_escape(t) is Some {
        match decode_unit_escapes(t.subrange(8, t.len() as int)) {
            Some(r) => Some((seq![decode_unicode_escape(t)->0] + r.0, r.1)),
            None => None,
        }
    } else {
        None
    }
}

/// Decodes a rendered code-unit name at the start of `t`.
pub(crate) open spec fn decode_rendered_units(t: Seq<char>) -> Option<(Seq<u16>, Seq<char>)> {
    if t.len() > 0 && t[0] == '"' {
        decode_unit_escapes(t.drop_first())
    } else {
        None
    }
}

} // verus!

/// Return whether `ch` is NUL, CR, LF, HTAB, another C0 control, or DEL.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_forbidden_control(ch),
))]
pub(crate) fn is_forbidden_control_char(ch: char) -> bool {
    ch <= '\u{1f}' || ch == '\u{7f}'
}

/// Return whether `component` begins with an ASCII letter followed by `:`.
// Verus has no specification for `u8::is_ascii_alphabetic`, so the ASCII
// letter ranges are spelled out and verified directly.
#[allow(clippy::manual_is_ascii_check)]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == has_drive_prefix(component@),
))]
pub(crate) fn has_windows_drive_prefix(component: &str) -> bool {
    let bytes = component.as_bytes();
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_drive_prefix_bytes(component@);
    }
    bytes.len() >= 2 && matches!(bytes[0], b'A'..=b'Z' | b'a'..=b'z') && bytes[1] == b':'
}

/// Return whether any scalar of `value` is NUL, CR, LF, HTAB, another C0
/// control, or DEL.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == has_forbidden_control(value@),
))]
pub(crate) fn contains_forbidden_control(value: &str) -> bool {
    let mut chars = value.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= value@.len(),
            chars.remaining() == value@.skip(index),
            forall|k: int| 0 <= k < index ==> !is_forbidden_control(#[trigger] value@[k]),
        decreases value@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                if is_forbidden_control_char(ch) {
                    return true;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => return false,
        }
    }
}

/// Return whether canonical generation must emit `value` as a whole quoted
/// path: it is `...`, starts or ends with a space, or contains `#`, `\`, `[`,
/// `]`, comma, or a double quote.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == needs_quotes(value@),
))]
fn requires_quotes(value: &str) -> bool {
    let mut chars = value.chars();
    // The number of scalars seen, saturating at four, and whether all of
    // them are `.`, recognize the reserved `...` in the same single pass.
    let mut count: usize = 0;
    let mut all_dots = true;
    let mut last_is_space = false;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= value@.len(),
            chars.remaining() == value@.skip(index),
            count == if index < 4 { index } else { 4 },
            all_dots == forall|k: int| 0 <= k < index ==> #[trigger] value@[k] == '.',
            last_is_space == (index > 0 && value@[index - 1] == ' '),
            index > 0 ==> value@[0] != ' ',
            forall|k: int| 0 <= k < index ==> !is_bare_syntax_char(#[trigger] value@[k]),
        decreases value@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(ch == value@[index]);
                    assert(is_bare_syntax_char(value@[index]) == is_bare_syntax_char(ch));
                }
                if ch == '#' || ch == '\\' || ch == '[' || ch == ']' || ch == ',' || ch == '"' {
                    return true;
                }
                if count == 0 && ch == ' ' {
                    return true;
                }
                if ch != '.' {
                    all_dots = false;
                }
                last_is_space = ch == ' ';
                if count < 4 {
                    count += 1;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    if count == 3 && all_dots {
                        assert(value@ =~= seq!['.', '.', '.']);
                    }
                    if is_reserved_ellipsis(value@) {
                        assert(value@[0] == '.' && value@[1] == '.' && value@[2] == '.');
                    }
                }
                return last_is_space || (count == 3 && all_dots);
            }
        }
    }
}

/// Serialize one filesystem name as a canonical bare or whole-quoted guide
/// component, or return `None` when the name is empty or contains a
/// forbidden control.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        option_string_view(r) == serialized_component(value@),
))]
pub(crate) fn serialize_component(value: &str) -> Option<String> {
    if value.is_empty() || contains_forbidden_control(value) {
        return None;
    }

    if !requires_quotes(value) {
        return Some(value.to_owned());
    }

    let mut serialized = String::new();
    serialized.push('"');
    let mut chars = value.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= value@.len(),
            chars.remaining() == value@.skip(index),
            serialized@ == seq!['"'] + quote_escape(value@.take(index)),
            serialized_component(value@) == Some(quoted_component(value@)),
        decreases value@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    value@.lemma_flat_map_take_append(quote_escape_fn(), index);
                    assert(value@.take(index + 1) =~= value@.subrange(0, index + 1));
                    assert(value@.take(index) =~= value@.subrange(0, index));
                }
                if ch == '"' || ch == '\\' {
                    serialized.push('\\');
                }
                serialized.push(ch);
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(value@.take(index) =~= value@);
                }
                serialized.push('"');
                return Some(serialized);
            }
        }
    }
}

/// Return the uppercase hexadecimal digit for `n < 16`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        n < 16,
    ensures
        r == hex_digit(n),
))]
fn upper_hex_digit(n: u32) -> char {
    let digits: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
    ];
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(digits@ =~= seq![
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F'
        ]);
    }
    digits[n as usize]
}

/// Append `\u{XXXX}` with the four uppercase hexadecimal digits of `code`.
#[cfg_attr(verus_keep_ghost, verus_spec(
    ensures
        final(out)@ == old(out)@ + unicode_escape(code),
))]
fn push_unicode_escape(out: &mut String, code: u16) {
    let value = code as u32;
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_nibble(value >> 12u32);
        lemma_nibble(value >> 8u32);
        lemma_nibble(value >> 4u32);
        lemma_nibble(value);
    }
    out.push('\\');
    out.push('u');
    out.push('{');
    out.push(upper_hex_digit((value >> 12) & 0xF));
    out.push(upper_hex_digit((value >> 8) & 0xF));
    out.push(upper_hex_digit((value >> 4) & 0xF));
    out.push(upper_hex_digit(value & 0xF));
    out.push('}');
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(out@ =~= old(out)@ + unicode_escape(code));
    }
}

/// Append `\xNN` with the two uppercase hexadecimal digits of `byte`.
#[cfg(any(unix, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(
    ensures
        final(out)@ == old(out)@ + byte_escape(byte),
))]
fn push_byte_escape(out: &mut String, byte: u8) {
    let value = byte as u32;
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_nibble(value >> 4u32);
        lemma_nibble(value);
    }
    out.push('\\');
    out.push('x');
    out.push(upper_hex_digit((value >> 4) & 0xF));
    out.push(upper_hex_digit(value & 0xF));
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(out@ =~= old(out)@ + byte_escape(byte));
    }
}

/// Append the diagnostic rendering of one scalar.
#[cfg_attr(verus_keep_ghost, verus_spec(
    ensures
        final(out)@ == old(out)@ + render_char(ch),
))]
fn push_rendered_char(out: &mut String, ch: char) {
    match ch {
        '"' => {
            out.push('\\');
            out.push('"');
        }
        '\\' => {
            out.push('\\');
            out.push('\\');
        }
        '\0' => {
            out.push('\\');
            out.push('0');
        }
        '\t' => {
            out.push('\\');
            out.push('t');
        }
        '\n' => {
            out.push('\\');
            out.push('n');
        }
        '\r' => {
            out.push('\\');
            out.push('r');
        }
        '\u{1}'..='\u{1f}' | '\u{7f}' => push_unicode_escape(out, ch as u16),
        _ => out.push(ch),
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(out@ =~= old(out)@ + render_char(ch));
    }
}

/// Render a valid UTF-8 name in the reversible, control-free, double-quoted
/// diagnostic form.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == rendered_component(value@),
))]
pub(crate) fn render_utf8_component(value: &str) -> String {
    let mut rendered = String::new();
    rendered.push('"');
    let mut chars = value.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= value@.len(),
            chars.remaining() == value@.skip(index),
            rendered@ == seq!['"'] + value@.take(index).flat_map(render_char_fn()),
        decreases value@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    value@.lemma_flat_map_take_append(render_char_fn(), index);
                    assert(value@.take(index + 1) =~= value@.subrange(0, index + 1));
                    assert(value@.take(index) =~= value@.subrange(0, index));
                }
                push_rendered_char(&mut rendered, ch);
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(value@.take(index) =~= value@);
                }
                rendered.push('"');
                return rendered;
            }
        }
    }
}

/// Render a non-UTF-8 Unix name with every raw byte as uppercase `\xNN`.
#[cfg(any(unix, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == rendered_bytes(bytes@),
))]
pub(crate) fn render_raw_bytes(bytes: &[u8]) -> String {
    let mut rendered = String::new();
    rendered.push('"');
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= bytes.len(),
            rendered@ == seq!['"'] + bytes@.take(index as int).flat_map(byte_escape_fn()),
        decreases bytes.len() - index
    ))]
    while index < bytes.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            bytes@.lemma_flat_map_take_append(byte_escape_fn(), index as int);
            assert(bytes@.take(index + 1) =~= bytes@.subrange(0, index + 1));
            assert(bytes@.take(index as int) =~= bytes@.subrange(0, index as int));
        }
        push_byte_escape(&mut rendered, bytes[index]);
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(bytes@.take(index as int) =~= bytes@);
    }
    rendered.push('"');
    rendered
}

/// Render an ill-formed Windows name with every UTF-16 code unit as
/// uppercase `\u{XXXX}`.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == rendered_units(units@),
))]
pub(crate) fn render_wide_units(units: &[u16]) -> String {
    let mut rendered = String::new();
    rendered.push('"');
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= units.len(),
            rendered@ == seq!['"'] + units@.take(index as int).flat_map(unit_escape_fn()),
        decreases units.len() - index
    ))]
    while index < units.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            units@.lemma_flat_map_take_append(unit_escape_fn(), index as int);
            assert(units@.take(index + 1) =~= units@.subrange(0, index + 1));
            assert(units@.take(index as int) =~= units@.subrange(0, index as int));
        }
        push_unicode_escape(&mut rendered, units[index]);
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(units@.take(index as int) =~= units@);
    }
    rendered.push('"');
    rendered
}

/// Render a host name in its reversible diagnostic form: the UTF-8 form when
/// it is valid UTF-8, and otherwise every raw Unix byte or every Windows
/// UTF-16 code unit escaped.
pub(crate) fn render_os_component(value: &OsStr) -> String {
    if let Some(value) = value.to_str() {
        return render_utf8_component(value);
    }

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        render_raw_bytes(value.as_bytes())
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let units: Vec<u16> = value.encode_wide().collect();
        render_wide_units(&units)
    }

    #[cfg(not(any(unix, windows)))]
    {
        compile_error!("exact non-UTF-8 name rendering requires a Unix or Windows target");
    }
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Supporting lemmas
// ---------------------------------------------------------------------------

/// A nonempty `flat_map` is the image of its first element followed by the
/// `flat_map` of the rest.
proof fn lemma_flat_map_cons<A>(s: Seq<A>, f: spec_fn(A) -> Seq<char>)
    requires
        s.len() > 0,
    ensures
        s.flat_map(f) == f(s[0]) + s.drop_first().flat_map(f),
{
    assert(s.map_values(f).drop_first() =~= s.drop_first().map_values(f));
}

/// The first UTF-8 byte of a scalar is ASCII exactly when the scalar is, and
/// it is then the scalar itself.
proof fn lemma_first_byte(c: char)
    ensures
        encode_scalar(c as u32).len() >= 1,
        (c as u32) < 0x80 ==> encode_scalar(c as u32)[0] == (c as u32) as u8,
        (c as u32) >= 0x80 ==> encode_scalar(c as u32)[0] >= 0xC0u8,
{
    let v = c as u32;
    assert(is_scalar(v));
    if has_width_1_encoding(v) {
        assert(((v & 0x7Fu32) as u8) == (v as u8)) by (bit_vector)
            requires
                v <= 0x7Fu32,
        ;
        assert(leading_byte_width_1(v) == v as u8);
    } else if has_width_2_encoding(v) {
        let x = ((v >> 6u32) & 0x1Fu32) as u8;
        assert((0xC0u8 | x) >= 0xC0u8) by (bit_vector);
        assert(leading_byte_width_2(v) >= 0xC0u8);
    } else if has_width_3_encoding(v) {
        let x = ((v >> 12u32) & 0x0Fu32) as u8;
        assert((0xE0u8 | x) >= 0xC0u8) by (bit_vector);
        assert(leading_byte_width_3(v) >= 0xC0u8);
    } else {
        let x = ((v >> 18u32) & 0x7u32) as u8;
        assert((0xF0u8 | x) >= 0xC0u8) by (bit_vector);
        assert(leading_byte_width_4(v) >= 0xC0u8);
    }
}

/// The byte-level drive-prefix test on a UTF-8 encoding agrees with the
/// scalar-level [`has_drive_prefix`].
proof fn lemma_drive_prefix_bytes(s: Seq<char>)
    ensures
        ({
            let b = encode_utf8(s);
            (b.len() >= 2 && is_ascii_letter_byte(b[0]) && b[1] == 0x3Au8) == has_drive_prefix(s)
        }),
{
    let b = encode_utf8(s);
    if s.len() > 0 {
        lemma_first_byte(s[0]);
        let rest = s.drop_first();
        assert(b == encode_scalar(s[0] as u32) + encode_utf8(rest));
        if (s[0] as u32) < 0x80 {
            assert(encode_scalar(s[0] as u32).len() == 1);
            assert(b[0] == (s[0] as u32) as u8);
            if s.len() > 1 {
                lemma_first_byte(rest[0]);
                assert(encode_utf8(rest) == encode_scalar(rest[0] as u32) + encode_utf8(
                    rest.drop_first(),
                ));
                assert(b[1] == encode_scalar(rest[0] as u32)[0]);
                assert(rest[0] == s[1]);
            } else {
                assert(rest.len() == 0);
                assert(b.len() == 1);
            }
        } else {
            assert(b[0] >= 0xC0u8);
        }
    }
}

/// Every scalar of a `flat_map` comes from the image of some element.
proof fn lemma_flat_map_index<A>(s: Seq<A>, f: spec_fn(A) -> Seq<char>, k: int)
    requires
        0 <= k < s.flat_map(f).len(),
    ensures
        exists|i: int, j: int|
            0 <= i < s.len() && 0 <= j < f(s[i]).len() && #[trigger] f(s[i])[j] == s.flat_map(
                f,
            )[k],
    decreases s.len(),
{
    lemma_flat_map_cons(s, f);
    let head = f(s[0]);
    if k < head.len() {
        assert(f(s[0])[k] == s.flat_map(f)[k]);
    } else {
        lemma_flat_map_index(s.drop_first(), f, k - head.len());
        let (i, j) = choose|i: int, j: int|
            0 <= i < s.drop_first().len() && 0 <= j < f(s.drop_first()[i]).len()
                && #[trigger] f(s.drop_first()[i])[j] == s.drop_first().flat_map(f)[k
                - head.len()];
        assert(s.drop_first()[i] == s[i + 1]);
        assert(f(s[i + 1])[j] == s.flat_map(f)[k]);
    }
}

/// Every nibble is a valid hexadecimal digit index.
pub(crate) proof fn lemma_nibble(x: u32)
    ensures
        x & 0xFu32 < 16,
{
    assert(x & 0xFu32 < 16) by (bit_vector);
}

proof fn lemma_hex_digit_value(n: u32)
    requires
        n < 16,
    ensures
        hex_value(hex_digit(n)) == Some(n),
        !is_forbidden_control(hex_digit(n)),
{
}

/// The digit of every nibble decodes to that nibble and is not a control.
proof fn lemma_nibble_digit(x: u32)
    ensures
        x & 0xFu32 < 16,
        hex_value(hex_digit(x & 0xFu32)) == Some(x & 0xFu32),
        !is_forbidden_control(hex_digit(x & 0xFu32)),
{
    lemma_nibble(x);
    lemma_hex_digit_value(x & 0xFu32);
}

/// The four digits of `\u{XXXX}` decode back to the code unit.
proof fn lemma_unicode_escape_decodes(code: u16, rest: Seq<char>)
    ensures
        decode_unicode_escape(unicode_escape(code) + rest) == Some(code),
        (unicode_escape(code) + rest).subrange(8, (unicode_escape(code) + rest).len() as int)
            == rest,
{
    let v = code as u32;
    let t = unicode_escape(code) + rest;
    lemma_nibble_digit(v >> 12u32);
    lemma_nibble_digit(v >> 8u32);
    lemma_nibble_digit(v >> 4u32);
    lemma_nibble_digit(v);
    assert(((((v >> 12u32) & 0xFu32) << 12u32) | (((v >> 8u32) & 0xFu32) << 8u32) | (((v
        >> 4u32) & 0xFu32) << 4u32) | (v & 0xFu32)) == v) by (bit_vector)
        requires
            v <= 0xFFFFu32,
    ;
    assert(t.subrange(8, t.len() as int) =~= rest);
}

/// The two digits of `\xNN` decode back to the byte.
proof fn lemma_byte_escape_decodes(byte: u8, rest: Seq<char>)
    ensures
        decode_byte_escape(byte_escape(byte) + rest) == Some(byte),
        (byte_escape(byte) + rest).subrange(4, (byte_escape(byte) + rest).len() as int) == rest,
{
    let v = byte as u32;
    let t = byte_escape(byte) + rest;
    lemma_nibble_digit(v >> 4u32);
    lemma_nibble_digit(v);
    assert((((((v >> 4u32) & 0xFu32) << 4u32) | (v & 0xFu32)) as u8) == byte) by (bit_vector)
        requires
            v == byte as u32,
    ;
    assert(t.subrange(4, t.len() as int) =~= rest);
}

/// No scalar of a rendered UTF-8 scalar is a forbidden control.
proof fn lemma_render_char_is_safe(c: char)
    ensures
        forall|j: int|
            0 <= j < render_char(c).len() ==> !is_forbidden_control(#[trigger] render_char(c)[j]),
{
    if is_forbidden_control(c) && c != '\0' && c != '\t' && c != '\n' && c != '\r' {
        let v = (c as u16) as u32;
        lemma_nibble_digit(v >> 12u32);
        lemma_nibble_digit(v >> 8u32);
        lemma_nibble_digit(v >> 4u32);
        lemma_nibble_digit(v);
    }
}

proof fn lemma_unicode_escape_is_safe(code: u16)
    ensures
        forall|j: int|
            0 <= j < unicode_escape(code).len() ==> !is_forbidden_control(
                #[trigger] unicode_escape(code)[j],
            ),
{
    let v = code as u32;
    lemma_nibble_digit(v >> 12u32);
    lemma_nibble_digit(v >> 8u32);
    lemma_nibble_digit(v >> 4u32);
    lemma_nibble_digit(v);
}

proof fn lemma_byte_escape_is_safe(byte: u8)
    ensures
        forall|j: int|
            0 <= j < byte_escape(byte).len() ==> !is_forbidden_control(
                #[trigger] byte_escape(byte)[j],
            ),
{
    let v = byte as u32;
    lemma_nibble_digit(v >> 4u32);
    lemma_nibble_digit(v);
}

/// A delimited `flat_map` whose every image is control-free is control-free.
proof fn lemma_delimited_flat_map_is_safe<A>(s: Seq<A>, f: spec_fn(A) -> Seq<char>)
    requires
        forall|i: int, j: int|
            0 <= i < s.len() && 0 <= j < f(s[i]).len() ==> !is_forbidden_control(
                #[trigger] f(s[i])[j],
            ),
    ensures
        !has_forbidden_control(seq!['"'] + s.flat_map(f) + seq!['"']),
{
    let t = seq!['"'] + s.flat_map(f) + seq!['"'];
    if has_forbidden_control(t) {
        let k = choose|k: int| 0 <= k < t.len() && #[trigger] is_forbidden_control(t[k]);
        if 1 <= k < t.len() - 1 {
            assert(t[k] == s.flat_map(f)[k - 1]);
            lemma_flat_map_index(s, f, k - 1);
            let (i, j) = choose|i: int, j: int|
                0 <= i < s.len() && 0 <= j < f(s[i]).len() && #[trigger] f(s[i])[j]
                    == s.flat_map(f)[k - 1];
            assert(!is_forbidden_control(f(s[i])[j]));
        }
    }
}

proof fn lemma_quote_escape_body_decodes(s: Seq<char>, rest: Seq<char>)
    requires
        !has_forbidden_control(s),
    ensures
        decode_quoted_chars(quote_escape(s) + seq!['"'] + rest) == Some((s, rest)),
    decreases s.len(),
{
    let t = quote_escape(s) + seq!['"'] + rest;
    if s.len() == 0 {
        assert(quote_escape(s) =~= Seq::<char>::empty());
        assert(t =~= seq!['"'] + rest);
        assert(t.drop_first() =~= rest);
    } else {
        let c = s[0];
        let tail = s.drop_first();
        lemma_flat_map_cons(s, quote_escape_fn());
        assert(!is_forbidden_control(s[0]));
        assert(!has_forbidden_control(tail)) by {
            if has_forbidden_control(tail) {
                let k = choose|k: int| 0 <= k < tail.len() && #[trigger] is_forbidden_control(
                    tail[k],
                );
                assert(s[k + 1] == tail[k]);
            }
        }
        lemma_quote_escape_body_decodes(tail, rest);
        let after = quote_escape(tail) + seq!['"'] + rest;
        assert(t =~= quote_escape_char(c) + after);
        if c == '"' || c == '\\' {
            assert(t.subrange(2, t.len() as int) =~= after);
            assert(seq![c] + s.drop_first() =~= s);
        } else {
            assert(t.drop_first() =~= after);
            assert(seq![c] + s.drop_first() =~= s);
        }
    }
}

/// Decoding the rendering of one scalar followed by more text yields that
/// scalar and then whatever the remaining text decodes to.
proof fn lemma_render_char_decodes(c: char, after: Seq<char>)
    ensures
        decode_rendered_chars(render_char(c) + after) == match decode_rendered_chars(after) {
            Some(r) => Some((seq![c] + r.0, r.1)),
            None => None,
        },
{
    let t = render_char(c) + after;
    if c == '"' || c == '\\' || c == '\0' || c == '\t' || c == '\n' || c == '\r' {
        assert(t.subrange(2, t.len() as int) =~= after);
    } else if is_forbidden_control(c) {
        lemma_unicode_escape_decodes(c as u16, after);
        assert(((c as u16) as u32) == c as u32);
        assert((((c as u16) as u32) as char) == c);
    } else {
        assert(t.drop_first() =~= after);
    }
}

proof fn lemma_render_body_decodes(s: Seq<char>, rest: Seq<char>)
    ensures
        decode_rendered_chars(s.flat_map(render_char_fn()) + seq!['"'] + rest) == Some((s, rest)),
    decreases s.len(),
{
    let t = s.flat_map(render_char_fn()) + seq!['"'] + rest;
    if s.len() == 0 {
        assert(s.flat_map(render_char_fn()) =~= Seq::<char>::empty());
        assert(t =~= seq!['"'] + rest);
        assert(t.drop_first() =~= rest);
    } else {
        let c = s[0];
        let tail = s.drop_first();
        lemma_flat_map_cons(s, render_char_fn());
        lemma_render_body_decodes(tail, rest);
        let after = tail.flat_map(render_char_fn()) + seq!['"'] + rest;
        assert(t =~= render_char(c) + after);
        assert(seq![c] + tail =~= s);
        lemma_render_char_decodes(c, after);
    }
}

proof fn lemma_byte_escapes_decode(bytes: Seq<u8>, rest: Seq<char>)
    ensures
        decode_byte_escapes(bytes.flat_map(byte_escape_fn()) + seq!['"'] + rest) == Some(
            (bytes, rest),
        ),
    decreases bytes.len(),
{
    let t = bytes.flat_map(byte_escape_fn()) + seq!['"'] + rest;
    if bytes.len() == 0 {
        assert(bytes.flat_map(byte_escape_fn()) =~= Seq::<char>::empty());
        assert(t =~= seq!['"'] + rest);
        assert(t.drop_first() =~= rest);
    } else {
        let tail = bytes.drop_first();
        lemma_flat_map_cons(bytes, byte_escape_fn());
        lemma_byte_escapes_decode(tail, rest);
        let after = tail.flat_map(byte_escape_fn()) + seq!['"'] + rest;
        assert(t =~= byte_escape(bytes[0]) + after);
        lemma_byte_escape_decodes(bytes[0], after);
        assert(seq![bytes[0]] + tail =~= bytes);
    }
}

proof fn lemma_unit_escapes_decode(units: Seq<u16>, rest: Seq<char>)
    ensures
        decode_unit_escapes(units.flat_map(unit_escape_fn()) + seq!['"'] + rest) == Some(
            (units, rest),
        ),
    decreases units.len(),
{
    let t = units.flat_map(unit_escape_fn()) + seq!['"'] + rest;
    if units.len() == 0 {
        assert(units.flat_map(unit_escape_fn()) =~= Seq::<char>::empty());
        assert(t =~= seq!['"'] + rest);
        assert(t.drop_first() =~= rest);
    } else {
        let tail = units.drop_first();
        lemma_flat_map_cons(units, unit_escape_fn());
        lemma_unit_escapes_decode(tail, rest);
        let after = tail.flat_map(unit_escape_fn()) + seq!['"'] + rest;
        assert(t =~= unicode_escape(units[0]) + after);
        lemma_unicode_escape_decodes(units[0], after);
        assert(seq![units[0]] + tail =~= units);
    }
}

// ---------------------------------------------------------------------------
// Contract theorems
// ---------------------------------------------------------------------------

/// Serialization never emits a forbidden control.
pub(crate) proof fn theorem_serialized_has_no_control(s: Seq<char>)
    ensures
        serialized_component(s) matches Some(out) ==> !has_forbidden_control(out),
{
    if serialized_component(s) is Some && needs_quotes(s) {
        assert forall|i: int, j: int|
            0 <= i < s.len() && 0 <= j < quote_escape_fn()(s[i]).len()
            implies !is_forbidden_control(#[trigger] quote_escape_fn()(s[i])[j]) by {
            // An escaped scalar is either the escaping backslash or a scalar
            // of the control-free name.
            assert(!is_forbidden_control(s[i]));
        }
        lemma_delimited_flat_map_is_safe(s, quote_escape_fn());
    }
}

/// The canonical quoted form decodes, under the contract's `quoted-path`
/// grammar, to exactly the original name, and decoding stops at its closing
/// quote whatever text follows (for example `/` or ` # comment`).
pub(crate) proof fn theorem_quoted_component_decodes(s: Seq<char>, rest: Seq<char>)
    requires
        s.len() > 0,
        !has_forbidden_control(s),
    ensures
        decode_quoted_path(quoted_component(s) + rest) == Some((s, rest)),
{
    let t = quoted_component(s) + rest;
    assert(t.drop_first() =~= quote_escape(s) + seq!['"'] + rest);
    lemma_quote_escape_body_decodes(s, rest);
}

/// Canonical serialization is injective on representable names.
pub(crate) proof fn theorem_serialize_injective(a: Seq<char>, b: Seq<char>)
    requires
        serialized_component(a) is Some,
        serialized_component(a) == serialized_component(b),
    ensures
        a == b,
{
    let out = serialized_component(a)->0;
    if needs_quotes(a) && needs_quotes(b) {
        theorem_quoted_component_decodes(a, Seq::empty());
        theorem_quoted_component_decodes(b, Seq::empty());
        assert(quoted_component(a) + Seq::<char>::empty() =~= quoted_component(a));
        assert(quoted_component(b) + Seq::<char>::empty() =~= quoted_component(b));
    } else if needs_quotes(a) {
        // `b` is emitted bare, so it is the quoted form of `a`, which starts
        // with a double quote and therefore needs quotes itself.
        assert(out[0] == '"');
        assert(b[0] == '"');
        assert(is_bare_syntax_char(b[0]));
    } else if needs_quotes(b) {
        assert(out[0] == '"');
        assert(a[0] == '"');
        assert(is_bare_syntax_char(a[0]));
    }
}

/// The canonical form is bare exactly when the contract allows it: a bare
/// form is the name itself, is not `...`, has no leading or trailing space,
/// and contains no `#`, `\`, `[`, `]`, comma, or double quote; every other
/// representable name is emitted as one whole quoted component, which starts
/// and ends with a double quote and so is never a bare form.
pub(crate) proof fn theorem_serialized_form_shape(s: Seq<char>)
    requires
        serialized_component(s) is Some,
    ensures
        ({
            let out = serialized_component(s)->0;
            &&& out.len() > 0
            &&& !needs_quotes(s) ==> {
                &&& out == s
                &&& !is_reserved_ellipsis(out)
                &&& out[0] != ' '
                &&& out[out.len() - 1] != ' '
                &&& forall|i: int| 0 <= i < out.len() ==> !is_bare_syntax_char(#[trigger] out[i])
            }
            &&& needs_quotes(s) ==> {
                &&& out == quoted_component(s)
                &&& out.len() >= 2
                &&& out[0] == '"'
                &&& out[out.len() - 1] == '"'
                &&& needs_quotes(out)
            }
        }),
{
    if needs_quotes(s) {
        let out = quoted_component(s);
        assert(is_bare_syntax_char(out[0]));
    }
}

/// The rendered form of a valid UTF-8 name contains no C0 control and no DEL.
pub(crate) proof fn theorem_rendered_has_no_control(s: Seq<char>)
    ensures
        !has_forbidden_control(rendered_component(s)),
{
    assert forall|i: int, j: int|
        0 <= i < s.len() && 0 <= j < render_char_fn()(s[i]).len() implies !is_forbidden_control(
        #[trigger] render_char_fn()(s[i])[j],
    ) by {
        lemma_render_char_is_safe(s[i]);
    }
    lemma_delimited_flat_map_is_safe(s, render_char_fn());
}

/// Every rendered form is delimited by double quotes.
pub(crate) proof fn theorem_rendered_is_delimited(s: Seq<char>)
    ensures
        rendered_component(s).len() >= 2,
        rendered_component(s)[0] == '"',
        rendered_component(s)[rendered_component(s).len() - 1] == '"',
{
}

/// The rendered form of a valid UTF-8 name decodes to exactly that name, and
/// decoding stops at its closing quote whatever text follows.
pub(crate) proof fn theorem_rendered_decodes(s: Seq<char>, rest: Seq<char>)
    ensures
        decode_rendered(rendered_component(s) + rest) == Some((s, rest)),
{
    let t = rendered_component(s) + rest;
    assert(t.drop_first() =~= s.flat_map(render_char_fn()) + seq!['"'] + rest);
    lemma_render_body_decodes(s, rest);
}

/// Distinct valid UTF-8 names have distinct rendered forms.
pub(crate) proof fn theorem_rendered_injective(a: Seq<char>, b: Seq<char>)
    requires
        rendered_component(a) == rendered_component(b),
    ensures
        a == b,
{
    theorem_rendered_decodes(a, Seq::empty());
    theorem_rendered_decodes(b, Seq::empty());
}

/// The rendered form of raw Unix bytes contains no C0 control and no DEL.
pub(crate) proof fn theorem_rendered_bytes_have_no_control(bytes: Seq<u8>)
    ensures
        !has_forbidden_control(rendered_bytes(bytes)),
{
    assert forall|i: int, j: int|
        0 <= i < bytes.len() && 0 <= j < byte_escape_fn()(bytes[i]).len()
        implies !is_forbidden_control(#[trigger] byte_escape_fn()(bytes[i])[j]) by {
        lemma_byte_escape_is_safe(bytes[i]);
    }
    lemma_delimited_flat_map_is_safe(bytes, byte_escape_fn());
}

/// The rendered form of raw Unix bytes decodes to exactly those bytes.
pub(crate) proof fn theorem_rendered_bytes_decode(bytes: Seq<u8>, rest: Seq<char>)
    ensures
        decode_rendered_bytes(rendered_bytes(bytes) + rest) == Some((bytes, rest)),
{
    let t = rendered_bytes(bytes) + rest;
    assert(t.drop_first() =~= bytes.flat_map(byte_escape_fn()) + seq!['"'] + rest);
    lemma_byte_escapes_decode(bytes, rest);
}

/// Distinct byte names have distinct rendered forms.
pub(crate) proof fn theorem_rendered_bytes_injective(a: Seq<u8>, b: Seq<u8>)
    requires
        rendered_bytes(a) == rendered_bytes(b),
    ensures
        a == b,
{
    theorem_rendered_bytes_decode(a, Seq::empty());
    theorem_rendered_bytes_decode(b, Seq::empty());
}

/// The rendered form of Windows code units contains no C0 control and no DEL.
pub(crate) proof fn theorem_rendered_units_have_no_control(units: Seq<u16>)
    ensures
        !has_forbidden_control(rendered_units(units)),
{
    assert forall|i: int, j: int|
        0 <= i < units.len() && 0 <= j < unit_escape_fn()(units[i]).len()
        implies !is_forbidden_control(#[trigger] unit_escape_fn()(units[i])[j]) by {
        lemma_unicode_escape_is_safe(units[i]);
    }
    lemma_delimited_flat_map_is_safe(units, unit_escape_fn());
}

/// The rendered form of Windows code units decodes to exactly those units.
pub(crate) proof fn theorem_rendered_units_decode(units: Seq<u16>, rest: Seq<char>)
    ensures
        decode_rendered_units(rendered_units(units) + rest) == Some((units, rest)),
{
    let t = rendered_units(units) + rest;
    assert(t.drop_first() =~= units.flat_map(unit_escape_fn()) + seq!['"'] + rest);
    lemma_unit_escapes_decode(units, rest);
}

/// Distinct code-unit names have distinct rendered forms.
pub(crate) proof fn theorem_rendered_units_injective(a: Seq<u16>, b: Seq<u16>)
    requires
        rendered_units(a) == rendered_units(b),
    ensures
        a == b,
{
    theorem_rendered_units_decode(a, Seq::empty());
    theorem_rendered_units_decode(b, Seq::empty());
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    /// The renderer this module used before it was verified, kept as a
    /// byte-for-byte oracle.
    fn reference_render_utf8(value: &str) -> String {
        let mut rendered = String::with_capacity(value.len() + 2);
        rendered.push('"');
        for ch in value.chars() {
            match ch {
                '"' => rendered.push_str("\\\""),
                '\\' => rendered.push_str("\\\\"),
                '\0' => rendered.push_str("\\0"),
                '\t' => rendered.push_str("\\t"),
                '\n' => rendered.push_str("\\n"),
                '\r' => rendered.push_str("\\r"),
                '\u{1}'..='\u{1f}' | '\u{7f}' => {
                    write!(rendered, "\\u{{{:04X}}}", ch as u32).unwrap();
                }
                _ => rendered.push(ch),
            }
        }
        rendered.push('"');
        rendered
    }

    /// The serializer this module used before it was verified.
    fn reference_serialize(value: &str) -> Option<String> {
        if value.is_empty() || value.chars().any(|ch| ch <= '\u{1f}' || ch == '\u{7f}') {
            return None;
        }
        let requires_quotes = value == "..."
            || value.starts_with(' ')
            || value.ends_with(' ')
            || value
                .chars()
                .any(|ch| matches!(ch, '#' | '\\' | '[' | ']' | ',' | '"'));
        if !requires_quotes {
            return Some(value.to_string());
        }
        let mut serialized = String::with_capacity(value.len() + 2);
        serialized.push('"');
        for ch in value.chars() {
            if matches!(ch, '"' | '\\') {
                serialized.push('\\');
            }
            serialized.push(ch);
        }
        serialized.push('"');
        Some(serialized)
    }

    fn sample_names() -> Vec<String> {
        let mut names: Vec<String> = [
            "",
            "a",
            "main.rs",
            ".",
            "..",
            "...",
            "....",
            ".. .",
            " ",
            " lead",
            "trail ",
            "in side",
            "a#b",
            "a\\b",
            "a[b",
            "a]b",
            "a,b",
            "a\"b",
            "\"",
            "\\",
            "C:",
            "C:notes",
            "c:\\x",
            "1:",
            "é:",
            "目录",
            "résumé|draft(1).txt",
            "tab\there",
            "nul\0",
            "del\u{7f}",
            "esc\u{1b}[31m",
            "cr\rlf\n",
            "c1\u{9b}",
            "emoji 😀",
        ]
        .iter()
        .map(|name| (*name).to_string())
        .collect();
        for code in 0u32..=0x80 {
            if let Some(ch) = char::from_u32(code) {
                names.push(ch.to_string());
                names.push(format!("x{ch}y"));
            }
        }
        names
    }

    #[test]
    fn forbidden_control_classification_is_exact() {
        for code in 0u32..=0x10FFFF {
            if let Some(ch) = char::from_u32(code) {
                assert_eq!(
                    is_forbidden_control_char(ch),
                    code <= 0x1F || code == 0x7F,
                    "U+{code:04X}"
                );
            }
        }
        assert!(!contains_forbidden_control(""));
        assert!(!contains_forbidden_control("plain é 😀"));
        assert!(contains_forbidden_control("a\0"));
        assert!(contains_forbidden_control("\u{7f}b"));
        assert!(contains_forbidden_control("x\ty"));
        assert!(!contains_forbidden_control("\u{80}\u{9f}"));
    }

    #[test]
    fn drive_prefix_matches_ascii_letter_then_colon() {
        for name in ["C:", "c:", "Z:notes", "a:/", "z:\\"] {
            assert!(has_windows_drive_prefix(name), "{name:?}");
        }
        for name in [
            "", "C", ":", "1:", "é:", "C;", "CC:", " C:", "[:", "@:", "`:", "{:",
        ] {
            assert!(!has_windows_drive_prefix(name), "{name:?}");
        }
    }

    #[test]
    fn serialize_matches_reference_and_quotes_exactly_when_required() {
        for name in sample_names() {
            assert_eq!(
                serialize_component(&name),
                reference_serialize(&name),
                "{name:?}"
            );
        }
        assert_eq!(serialize_component(""), None);
        assert_eq!(serialize_component("a\nb"), None);
        assert_eq!(serialize_component("plain"), Some("plain".to_string()));
        assert_eq!(serialize_component("...."), Some("....".to_string()));
        assert_eq!(serialize_component(".."), Some("..".to_string()));
        assert_eq!(serialize_component("..."), Some("\"...\"".to_string()));
        assert_eq!(serialize_component(" a"), Some("\" a\"".to_string()));
        assert_eq!(serialize_component("a "), Some("\"a \"".to_string()));
        assert_eq!(
            serialize_component("a\"b\\c"),
            Some("\"a\\\"b\\\\c\"".to_string())
        );
        assert_eq!(serialize_component("a,b"), Some("\"a,b\"".to_string()));
    }

    #[test]
    fn render_utf8_matches_reference_for_every_scalar() {
        let mut all = String::new();
        for code in 0u32..=0x10FFFF {
            if let Some(ch) = char::from_u32(code) {
                let single = ch.to_string();
                assert_eq!(
                    render_utf8_component(&single),
                    reference_render_utf8(&single),
                    "U+{code:04X}"
                );
                if code <= 0x200 {
                    all.push(ch);
                }
            }
        }
        assert_eq!(render_utf8_component(&all), reference_render_utf8(&all));
        assert_eq!(render_utf8_component(""), "\"\"");
        assert_eq!(render_utf8_component("\u{1}"), "\"\\u{0001}\"");
        assert_eq!(render_utf8_component("\u{7f}"), "\"\\u{007F}\"");
        assert_eq!(render_utf8_component("\u{1b}"), "\"\\u{001B}\"");
        assert_eq!(
            render_utf8_component("\"\\\0\t\n\r"),
            "\"\\\"\\\\\\0\\t\\n\\r\""
        );
    }

    #[test]
    fn raw_byte_rendering_escapes_every_byte_in_uppercase() {
        assert_eq!(render_raw_bytes(&[]), "\"\"");
        assert_eq!(
            render_raw_bytes(&[0x00, 0x2F, 0x7F, 0x80, 0xAB, 0xFF]),
            "\"\\x00\\x2F\\x7F\\x80\\xAB\\xFF\""
        );
        for byte in 0u8..=0xFF {
            assert_eq!(
                render_raw_bytes(&[byte]),
                format!("\"\\x{byte:02X}\""),
                "{byte:#04X}"
            );
        }
    }

    #[test]
    fn wide_unit_rendering_escapes_every_unit_in_uppercase() {
        assert_eq!(render_wide_units(&[]), "\"\"");
        assert_eq!(
            render_wide_units(&[0x0062, 0xD800, 0x0061, 0xDC00]),
            "\"\\u{0062}\\u{D800}\\u{0061}\\u{DC00}\""
        );
        for unit in [0u16, 0x1, 0x7F, 0xABCD, 0xFFFF] {
            assert_eq!(
                render_wide_units(&[unit]),
                format!("\"\\u{{{unit:04X}}}\""),
                "{unit:#06X}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_unix_name_diagnostic_preserves_every_byte() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        assert_eq!(
            render_os_component(OsStr::from_bytes(b"a\xFFb")),
            "\"\\x61\\xFF\\x62\""
        );
        assert_eq!(render_os_component(OsStr::new("ok\n")), "\"ok\\n\"");
    }

    #[cfg(windows)]
    #[test]
    fn ill_formed_windows_name_diagnostic_preserves_every_utf16_unit() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let name = OsString::from_wide(&[0x0062, 0xD800, 0x0061, 0xDC00]);

        assert_eq!(
            render_os_component(&name),
            "\"\\u{0062}\\u{D800}\\u{0061}\\u{DC00}\""
        );
    }
}
