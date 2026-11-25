//! Machine-checked textual round trip of generated guides.
//!
//! The contract's core invariant (`docs/v0.2-contract.md`, "Core
//! invariant") requires that a filesystem tree serialized by `dump` or
//! `init`, then parsed, checked, and verified against the same tree,
//! succeeds for the same names, hierarchy, and regular-file/directory
//! kinds. This proof-only module proves the textual half of that invariant
//! (serialize, then parse and check) by composing the specifications of the
//! verified kernels that generation and parsing call. It is compiled only
//! by Verus through `proofs/verus_root.rs`; it is not part of the Cargo
//! package and adds no assumption: every theorem follows from the kernels'
//! specifications, which Verus proves the shipped functions satisfy.
//!
//! The kernels involved are: generation's name admission
//! (`proven::generation::host_name_verdict`), canonical serialization
//! (`path_codec::serialize_component`), and line formatting
//! (`proven::generation::format_entry_line`); and the parser's marker
//! classification (`proven::marker`), envelope fold (`proven::document`),
//! line-ending check (`proven::document::first_lone_cr_line`), list-item
//! layout and indentation (`proven::layout`), entry-text parsing
//! (`proven::entry`), and the validator's structural path rules
//! (`proven::path_rules`).
//!
//! # Model
//!
//! A generation tree is a [`SpecTree`] forest: the generation root's
//! children, each with a name, a kind, and its children in stored order.
//! It is well formed ([`well_formed_tree`]) when every name is admitted by
//! generation at its level (the root's children with `at_root`), is
//! nonempty, is not `.` or `..`, and contains no `/` (as every
//! directory-entry name), every logical depth is at most 256, and only
//! directories have children. [`flatten_from`] is the preorder emission of
//! `Dumper::format_tree`: each child's line in stored order, then, when the
//! child has children, its children one level deeper. An emission is a
//! sequence of [`Emitted`] `(depth, name, kind)`; its body lines are the
//! `format_entry_line` outputs without their final line feed, as
//! `str::lines` yields them.
//!
//! # Theorems
//!
//! - T1 [`theorem_entry_line_round_trip`]: for every admitted name, kind,
//!   depth, and indent size, the generated line is ordinary text for the
//!   marker kernel; it is a list item whose indentation is exactly
//!   `depth * indent` spaces and whose entry text is the serialized name
//!   followed by `/` exactly for a directory; the entry kernel parses that
//!   text, at that level, as exactly the name with the same kind and no
//!   comment (so the line can own children exactly when it is a
//!   directory); and the validator's structural path rules accept the name.
//! - T2 [`theorem_body_round_trip`]: for every valid emission
//!   ([`valid_emission`]: first depth 0, depths at most 256 that grow by at
//!   most one and only below a directory, admitted names) and every indent
//!   size from 1 to 16, the layout kernel accepts the body lines with
//!   exactly the emitted depths, the body satisfies the contract's
//!   indentation and parentage rules with the indent size as its unit, and
//!   every line recovers its name and kind.
//!   [`theorem_parse_recovers_emission`]: the parser's per-line loop
//!   ([`parse_lines`]: layout kernel, entry kernel, `advance`) accepts the
//!   body and yields, line by line, exactly the emitted depth and the entry
//!   with the emitted name and kind.
//! - T3 [`theorem_tree_emission_valid`]: the emission of every well-formed
//!   tree is a valid emission, so [`theorem_tree_round_trip`] applies T2 to
//!   trees; [`theorem_tree_emission_injective`]: distinct trees have
//!   distinct emissions, so the recovered `(depth, name, kind)` sequence
//!   determines the tree.
//! - T4 [`theorem_document_envelope`]: body lines that are all ordinary
//!   text, wrapped in the opening and closing lines `dump` writes, classify
//!   as `Opening`, `Ordinary`..., `Closing`, and the envelope fold accepts
//!   them as one active (not ignored) block whose opening marker is line 1,
//!   whose closing marker is the last line, and whose body is exactly the
//!   given lines.
//! - [`theorem_document_text_lines`]: the text `dump_with_wrapper` builds
//!   has no carriage return, so the parser's lone-CR check accepts it; split
//!   as `str::lines` documents ([`lines_spec`]) it gives exactly the opening
//!   line, the body lines, and the closing line; and the body the parser
//!   re-joins with line feeds splits back into the same lines and is not
//!   blank.
//! - [`theorem_dump_round_trip`]: all of the above for the document `dump`
//!   writes for a nonempty well-formed tree, ending in the parser's items
//!   and the uniqueness of the tree they came from.
//! - [`theorem_init_text_lines`], [`theorem_framed_document_envelope`], and
//!   [`theorem_init_round_trip`]: the same for the file `init` writes, which
//!   surrounds that document with five Markdown prologue lines and two
//!   epilogue lines; every one of them is ordinary text, so the envelope
//!   fold accepts the block on lines 6 through `body.len() + 7` with the
//!   same body.
//!
//! `lemma_example_tree` and `lemma_example_round_trip` instantiate the
//! hypotheses with a concrete tree (a directory `src` holding the file
//! `a#b`, which generation must quote), so the theorems are not vacuous.
//!
//! # What the theorems do not cover
//!
//! The theorems state what the kernels' specifications imply when they are
//! called in the order the unverified glue calls them. That glue is checked
//! by tests, not by these proofs:
//!
//! - Generation traversal and tree building: directory enumeration, entry
//!   classification, exclusion, and depth limits; `Dumper::build_tree`
//!   producing one node per included entry, children only under
//!   directories, and distinct sibling names; `Dumper::prepare_tree`
//!   sorting children (the theorems hold for every child order) and
//!   calling `host_name_verdict` with `depth == 0` and `serialize_component`
//!   for every child; `Dumper::format_tree`'s recursion emitting exactly
//!   [`flatten_from`]; `Dumper::dump_with_wrapper`'s `format!` building
//!   exactly [`document_text`]; and the raw-string `format!` in
//!   `cli/init.rs` building exactly [`init_text`] (its prose lines are
//!   transcribed in [`init_title`], [`init_intro`], [`init_scope`], and
//!   [`init_note`]). `format_entry_line` fails only when
//!   `depth * indent` overflows `usize`, which no depth up to 256 with an
//!   indent size up to 16 does; T1 itself holds for every depth and indent.
//! - The standard library: `str::lines` splitting carriage-return-free text
//!   as [`lines_spec`] (its documented behavior), `[&str]::join("\n")` as
//!   [`join_lf`], and `String` and `Vec` plumbing.
//! - Parser assembly: `Parser::extract_guide_block` slicing
//!   `lines[start + 1..end]` from the envelope's marker lines (T4 states the
//!   slice) and re-splitting it; `Parser::parse_guide_content` performing
//!   exactly [`parse_lines`] (one `layout_entry_line`, one `parse_entry` at
//!   `depth == 0`, and one `advance` per line, in order);
//!   `Parser::parse_entry_items` turning a single parsed path into one
//!   `FilesystemItem` of its kind; and `Parser::build_hierarchy` assembling
//!   the items into a hierarchy. The theorems prove that the recovered
//!   `(depth, name, kind)` sequence determines the tree, not that
//!   `build_hierarchy` rebuilds it.
//! - Validator checks other than the structural path rules: a nonempty
//!   item list, unique full paths (true of a filesystem tree, whose sibling
//!   names are distinct, but not modeled here), indentation consistency and
//!   nesting over the assembled hierarchy, and placeholder rules (generated
//!   guides have no placeholders). T1 applies the structural path rules
//!   with `at_root` exactly for depth 0, which is where the validator
//!   applies them once `build_hierarchy` has placed depth-0 items at the
//!   top level.
//! - File I/O and encoding: writing the generated document (`init`, or
//!   `dump` to stdout or a file), reading it back through the guide-input
//!   checks, and UTF-8 decoding; the theorems are over Unicode scalar
//!   sequences, and the marker kernel's input is their UTF-8 encoding.
//! - The filesystem half of the invariant: `verify` against the same tree.

use vstd::prelude::*;

verus! {

use vstd::std_specs::char::is_white_space;
use vstd::utf8::{encode_scalar, encode_utf8, encode_utf8_concat, has_width_1_encoding};

use crate::path_codec::{
    has_forbidden_control, is_bare_syntax_char, is_forbidden_control, is_reserved_ellipsis,
    needs_quotes, quoted_component, serialized_component, theorem_serialized_form_shape,
    theorem_serialized_has_no_control,
};
use crate::proven::choice::{has_dot_component, has_invalid_component, invalid_decoded_component};
use crate::proven::document::{
    is_lone_cr, locate_envelope, no_lone_cr, single_envelope, theorem_envelope_accepts_exactly,
    Envelope, EnvelopeError,
};
use crate::proven::entry::{
    bare_path_rule, can_own_children, entry_spec, is_plain_bare_text, single_path_spec,
    theorem_plain_bare_entry, theorem_quoted_component_entry, EntryView, FailureView, ShapeView,
};
use crate::proven::generation::{
    directory_marker, entry_line_spec, host_name_verdict_spec, spaces,
    theorem_representable_names_are_valid_paths, HostNameVerdict,
};
use crate::proven::layout::{
    advance_state, all_white_space, depth_rules, entry_shape, initial_indent_state, is_list_item,
    item_width, layout_line, layout_lines, list_item_at, spaces_only, theorem_layout_complete,
    valid_layout, width_rule, IndentState, LayoutError,
};
use crate::proven::marker::{
    active_opening_line, bom_bytes, bom_prefixed, candidate, closing_line, closing_name, is_wsp,
    lit_at, marker_class, opening_name, skip_wsp, wsp_run, MarkerClass,
};
use crate::proven::path_rules::{is_dot_component, path_verdict, split_components, PathVerdict};
use crate::proven::placeholder::lemma_split_free;

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

/// A directory-entry name that the dumper admits at its level: the
/// generation kernel's verdict is `Representable` (no forbidden control,
/// and at the root no leading `\` and no drive prefix), and, as for every
/// name a directory enumeration returns, it is nonempty, not `.` or `..`,
/// and free of `/`.
pub(crate) open spec fn dumpable_name(n: Seq<char>, at_root: bool) -> bool {
    &&& host_name_verdict_spec(n, at_root) == HostNameVerdict::Representable
    &&& n.len() > 0
    &&& !is_dot_component(n)
    &&& forall|i: int| 0 <= i < n.len() ==> n[i] != '/'
}

/// The canonical serialization of a representable name.
pub(crate) open spec fn serialized(n: Seq<char>) -> Seq<char> {
    serialized_component(n)->0
}

/// The entry text of a generated line: the serialized name and, for a
/// directory, `/`.
pub(crate) open spec fn entry_text(n: Seq<char>, is_dir: bool) -> Seq<char> {
    serialized(n) + directory_marker(is_dir)
}

/// The generated line as `str::lines` yields it: [`entry_line_spec`] without
/// its final line feed.
pub(crate) open spec fn emitted_line(depth: nat, indent: nat, n: Seq<char>, is_dir: bool) -> Seq<
    char,
> {
    entry_line_spec(depth, indent, serialized(n), is_dir).drop_last()
}

/// The entry an accepted generated line denotes: exactly one path, its
/// kind, and no comment.
pub(crate) open spec fn single_entry(n: Seq<char>, is_dir: bool) -> EntryView {
    EntryView { shape: ShapeView::Single { path: n, directory: is_dir }, comment: None }
}

/// No scalar of `s` is a line feed.
pub(crate) open spec fn lf_free(s: Seq<char>) -> bool {
    forall|i: int| 0 <= i < s.len() ==> #[trigger] s[i] != '\n'
}

/// No scalar of `s` is a carriage return.
pub(crate) open spec fn cr_free(s: Seq<char>) -> bool {
    forall|i: int| 0 <= i < s.len() ==> #[trigger] s[i] != '\r'
}

// ---------------------------------------------------------------------------
// UTF-8 and marker lemmas
// ---------------------------------------------------------------------------

/// An ASCII scalar sequence encodes to one byte per scalar, the scalar's
/// value.
proof fn lemma_encode_ascii(s: Seq<char>)
    requires
        forall|i: int| 0 <= i < s.len() ==> (#[trigger] s[i] as u32) < 0x80,
    ensures
        encode_utf8(s).len() == s.len(),
        forall|i: int| 0 <= i < s.len() ==> #[trigger] encode_utf8(s)[i] == (s[i] as u32) as u8,
    decreases s.len(),
{
    if s.len() > 0 {
        let rest = s.drop_first();
        assert forall|i: int| 0 <= i < rest.len() implies (#[trigger] rest[i] as u32) < 0x80 by {
            assert(rest[i] == s[i + 1]);
        }
        lemma_encode_ascii(rest);
        let v = s[0] as u32;
        assert(v < 0x80);
        assert(has_width_1_encoding(v));
        assert((v & 0x7Fu32) as u8 == v as u8) by (bit_vector)
            requires
                v < 0x80u32,
        ;
        assert(encode_scalar(v) == seq![v as u8]);
        assert(encode_utf8(s) == encode_scalar(v) + encode_utf8(rest));
        assert forall|i: int| 0 <= i < s.len() implies #[trigger] encode_utf8(s)[i]
            == (s[i] as u32) as u8 by {
            if i > 0 {
                assert(encode_utf8(s)[i] == encode_utf8(rest)[i - 1]);
                assert(rest[i - 1] == s[i]);
            }
        }
    }
}

/// The greedy whitespace skip stops at the first byte that is not `WSP`.
proof fn lemma_skip_wsp_to(b: Seq<u8>, i: int, w: int)
    requires
        0 <= i <= w < b.len(),
        forall|k: int| i <= k < w ==> #[trigger] is_wsp(b[k]),
        !is_wsp(b[w]),
    ensures
        skip_wsp(b, i) == w,
    decreases w - i,
{
    if i < w {
        assert(is_wsp(b[i]));
        lemma_skip_wsp_to(b, i + 1, w);
    }
}

/// A literal occurrence fixes the byte at its first position.
proof fn lemma_lit_at_first(b: Seq<u8>, p: int, lit: Seq<u8>)
    requires
        lit_at(b, p, lit),
        lit.len() > 0,
    ensures
        b[p] == lit[0],
{
    assert(b.subrange(p, p + lit.len())[0] == b[p]);
}

/// A line whose first non-blank byte is preceded only by spaces and is
/// neither `<` nor the first byte of a byte-order mark is not a marker
/// candidate.
proof fn lemma_ordinary_bytes(b: Seq<u8>, w: int)
    requires
        0 <= w < b.len(),
        forall|k: int| 0 <= k < w ==> #[trigger] b[k] == 0x20u8,
        !is_wsp(b[w]),
        b[w] != 0x3Cu8,
        b[w] != 0xEFu8,
    ensures
        marker_class(b) == MarkerClass::Ordinary,
{
    assert forall|k: int| 0 <= k < w implies #[trigger] is_wsp(b[k]) by {
        assert(b[k] == 0x20u8);
    }
    lemma_skip_wsp_to(b, 0, w);
    assert(bom_bytes()[0] == 0xEFu8);
    assert(opening_name()[0] == 0x3Cu8);
    assert(closing_name()[0] == 0x3Cu8);
    if lit_at(b, w, bom_bytes()) {
        lemma_lit_at_first(b, w, bom_bytes());
    }
    assert(!bom_prefixed(b, opening_name()));
    assert(!bom_prefixed(b, closing_name()));
    assert(!candidate(b, opening_name())) by {
        if candidate(b, opening_name()) {
            let p = choose|p: int| wsp_run(b, 0, p) && lit_at(b, p, opening_name());
            lemma_lit_at_first(b, p, opening_name());
            if p > w {
                assert(is_wsp(b[w]));
            }
        }
    }
    assert(!candidate(b, closing_name())) by {
        if candidate(b, closing_name()) {
            let p = choose|p: int| wsp_run(b, 0, p) && lit_at(b, p, closing_name());
            lemma_lit_at_first(b, p, closing_name());
            if p > w {
                assert(is_wsp(b[w]));
            }
        }
    }
}

/// A line that starts with `w` spaces and then `-` is ordinary text, whatever
/// scalars follow.
proof fn lemma_dash_line_is_ordinary(l: Seq<char>, w: int)
    requires
        0 <= w < l.len(),
        forall|k: int| 0 <= k < w ==> #[trigger] l[k] == ' ',
        l[w] == '-',
    ensures
        marker_class(encode_utf8(l)) == MarkerClass::Ordinary,
{
    let head = l.take(w + 1);
    let tail = l.skip(w + 1);
    assert(l =~= head + tail);
    encode_utf8_concat(head, tail);
    assert forall|i: int| 0 <= i < head.len() implies (#[trigger] head[i] as u32) < 0x80 by {
        if i < w {
            assert(head[i] == ' ');
        } else {
            assert(head[i] == '-');
        }
    }
    lemma_encode_ascii(head);
    let b = encode_utf8(l);
    assert(b == encode_utf8(head) + encode_utf8(tail));
    assert forall|k: int| 0 <= k < w implies #[trigger] b[k] == 0x20u8 by {
        assert(b[k] == encode_utf8(head)[k]);
        assert(head[k] == ' ');
    }
    assert(b[w] == encode_utf8(head)[w]);
    assert(head[w] == '-');
    assert(b[w] == 0x2Du8);
    lemma_ordinary_bytes(b, w);
}

// ---------------------------------------------------------------------------
// Serialized names
// ---------------------------------------------------------------------------

/// No scalar of a representable name's serialization is a forbidden control,
/// and it starts neither with a space nor with a tab.
proof fn lemma_serialized_facts(n: Seq<char>)
    requires
        serialized_component(n) is Some,
    ensures
        serialized(n).len() > 0,
        !has_forbidden_control(serialized(n)),
        serialized(n)[0] != ' ',
        serialized(n)[0] != '\t',
{
    theorem_serialized_form_shape(n);
    theorem_serialized_has_no_control(n);
    let s = serialized(n);
    if s[0] == '\t' {
        assert(is_forbidden_control(s[0]));
    }
}

/// The bare single-path rules accept a representable, `/`-free name that
/// canonical generation emits bare.
proof fn lemma_bare_name_valid(n: Seq<char>, at_root: bool)
    requires
        dumpable_name(n, at_root),
        !needs_quotes(n),
    ensures
        bare_path_rule(n, at_root) == PathVerdict::Valid,
{
    lemma_split_free(n);
    let comps = split_components(n);
    assert(comps[0] == n);
    assert(!has_dot_component(n)) by {
        if has_dot_component(n) {
            let k = choose|k: int| 0 <= k < comps.len() && #[trigger] is_dot_component(comps[k]);
            assert(k == 0);
        }
    }
    assert(!has_forbidden_control(n));
    assert(!is_reserved_ellipsis(n));
    assert(!has_invalid_component(n)) by {
        if has_invalid_component(n) {
            let k = choose|k: int|
                0 <= k < comps.len() && #[trigger] invalid_decoded_component(comps[k]);
            assert(k == 0);
        }
    }
    assert(n[0] != '/');
    assert(n.last() == n[n.len() - 1]);
}

// ---------------------------------------------------------------------------
// T1: one generated entry line
// ---------------------------------------------------------------------------

/// The shape of a generated line: `depth * indent` spaces, `-`, a space,
/// and the entry text, which has no line feed and no carriage return and
/// starts with neither a space nor a tab.
proof fn lemma_line_shape(n: Seq<char>, is_dir: bool, depth: nat, indent: nat)
    requires
        serialized_component(n) is Some,
    ensures
        ({
            let w = (depth * indent) as int;
            let l = emitted_line(depth, indent, n, is_dir);
            let text = entry_text(n, is_dir);
            &&& entry_line_spec(depth, indent, serialized(n), is_dir) == l.push('\n')
            &&& l == spaces(depth * indent) + seq!['-', ' '] + text
            &&& text.len() > 0
            &&& text[0] != ' ' && text[0] != '\t'
            &&& l.len() == w + 2 + text.len()
            &&& forall|k: int| 0 <= k < w ==> #[trigger] l[k] == ' '
            &&& l[w] == '-'
            &&& l[w + 1] == ' '
            &&& l.skip(w + 2) == text
            &&& lf_free(l)
            &&& cr_free(l)
        }),
{
    lemma_serialized_facts(n);
    let w = (depth * indent) as int;
    let s = serialized(n);
    let text = entry_text(n, is_dir);
    let line = entry_line_spec(depth, indent, s, is_dir);
    let l = emitted_line(depth, indent, n, is_dir);
    assert(line =~= spaces(depth * indent) + seq!['-', ' '] + text + seq!['\n']);
    assert(l =~= spaces(depth * indent) + seq!['-', ' '] + text);
    assert(line =~= l.push('\n'));
    assert(l.skip(w + 2) =~= text);
    assert(text[0] == s[0]);
    assert forall|k: int| 0 <= k < l.len() implies #[trigger] l[k] != '\n' && l[k] != '\r' by {
        if k < w {
            assert(l[k] == ' ');
        } else if k == w || k == w + 1 {
        } else if k - w - 2 < s.len() {
            assert(l[k] == s[k - w - 2]);
            if s[k - w - 2] == '\n' || s[k - w - 2] == '\r' {
                assert(is_forbidden_control(s[k - w - 2]));
            }
        } else {
            assert(l[k] == '/');
        }
    }
}

/// (a) and (b) of T1: the generated line is ordinary text and a list item
/// whose indentation is exactly `depth * indent` spaces.
proof fn lemma_line_layout(n: Seq<char>, is_dir: bool, depth: nat, indent: nat)
    requires
        serialized_component(n) is Some,
    ensures
        ({
            let w = (depth * indent) as int;
            let l = emitted_line(depth, indent, n, is_dir);
            &&& marker_class(encode_utf8(l)) == MarkerClass::Ordinary
            &&& list_item_at(l, w)
            &&& is_list_item(l)
            &&& item_width(l) == w
            &&& entry_shape(l) == Ok::<int, LayoutError>(w)
        }),
{
    lemma_line_shape(n, is_dir, depth, indent);
    let w = (depth * indent) as int;
    let l = emitted_line(depth, indent, n, is_dir);
    lemma_dash_line_is_ordinary(l, w);
    assert forall|k: int| 0 <= k < w implies is_white_space(#[trigger] l[k]) by {
        assert(l[k] == ' ');
    }
    assert forall|k: int| w + 2 <= k < l.len() implies #[trigger] l[k] != '\n' by {
        assert(lf_free(l));
    }
    assert(list_item_at(l, w));
    assert(is_list_item(l));
    assert(item_width(l) == w) by {
        let v = item_width(l);
        assert(list_item_at(l, v));
        if v < w {
            assert(l[v] == ' ');
        }
        if v > w {
            assert(is_white_space(l[w]));
        }
    }
    assert(l[w + 2] == entry_text(n, is_dir)[0]);
    reveal(entry_shape);
    assert(!all_white_space(l)) by {
        assert(!is_white_space(l[w]));
    }
    assert(spaces_only(l, w));
}

/// (c) of T1: the entry text of a generated line parses as exactly its
/// name and kind, without a comment.
proof fn lemma_entry_text_parses(n: Seq<char>, is_dir: bool, at_root: bool)
    requires
        dumpable_name(n, at_root),
    ensures
        entry_spec(entry_text(n, is_dir), at_root) == Ok::<EntryView, FailureView>(
            single_entry(n, is_dir),
        ),
{
    assert(!has_forbidden_control(n));
    assert(serialized_component(n) is Some);
    theorem_serialized_form_shape(n);
    let s = serialized(n);
    let text = entry_text(n, is_dir);
    if needs_quotes(n) {
        theorem_quoted_component_entry(n, is_dir, at_root);
        assert(text == quoted_component(n) + directory_marker(is_dir));
    } else {
        assert(s == n);
        assert(is_plain_bare_text(text)) by {
            assert forall|k: int| 0 <= k < text.len() implies {
                let c = #[trigger] text[k];
                c != '#' && c != '\\' && c != '[' && c != ']' && c != '"'
            } by {
                if k < n.len() {
                    assert(text[k] == n[k]);
                    assert(!is_bare_syntax_char(n[k]));
                } else {
                    assert(text[k] == '/');
                }
            }
        }
        assert(text.last() != ' ') by {
            if !is_dir {
                assert(text =~= n);
            }
        }
        assert(text != seq!['.', '.', '.']) by {
            if is_dir {
                assert(text.last() == '/');
            } else {
                assert(text =~= n);
            }
        }
        theorem_plain_bare_entry(text, at_root);
        lemma_bare_name_valid(n, at_root);
        if is_dir {
            assert(text.drop_last() =~= n);
        } else {
            assert(text =~= n);
            assert(n.last() != '/');
        }
        assert(single_path_spec(text, at_root) == Ok::<ShapeView, FailureView>(
            ShapeView::Single { path: n, directory: is_dir },
        ));
    }
}

/// **T1.** For every name `n` the dumper admits at its level (`depth == 0`
/// is the generation root), every kind, every depth, and every indent size,
/// the line `Dumper::format_tree` emits for it ([`entry_line_spec`] of the
/// canonical serialization `s`), read back without its final line feed as
/// `str::lines` yields it, is:
///
/// - (a) ordinary text for the marker kernel: never a marker candidate, so
///   it can neither open, close, nor break the guide envelope;
/// - (b) a list item for the layout kernel whose indentation is exactly
///   `depth * indent` spaces and whose entry text is `s` followed by `/`
///   exactly for a directory, with no layout error;
/// - (c) an entry the entry kernel parses as exactly one path, `n` itself,
///   with the same kind and no comment, so its line can own children
///   exactly when it is a directory;
/// - (d) a path the validator's structural rules accept at that level.
pub(crate) proof fn theorem_entry_line_round_trip(
    n: Seq<char>,
    is_dir: bool,
    depth: nat,
    indent: nat,
)
    requires
        dumpable_name(n, depth == 0),
    ensures
        serialized_component(n) is Some,
        entry_line_spec(depth, indent, serialized(n), is_dir) == emitted_line(
            depth,
            indent,
            n,
            is_dir,
        ).push('\n'),
        marker_class(encode_utf8(emitted_line(depth, indent, n, is_dir))) == MarkerClass::Ordinary,
        is_list_item(emitted_line(depth, indent, n, is_dir)),
        list_item_at(emitted_line(depth, indent, n, is_dir), (depth * indent) as int),
        item_width(emitted_line(depth, indent, n, is_dir)) == depth * indent,
        emitted_line(depth, indent, n, is_dir).skip((depth * indent + 2) as int) == entry_text(
            n,
            is_dir,
        ),
        entry_shape(emitted_line(depth, indent, n, is_dir)) == Ok::<int, LayoutError>(
            (depth * indent) as int,
        ),
        entry_spec(entry_text(n, is_dir), depth == 0) == Ok::<EntryView, FailureView>(
            single_entry(n, is_dir),
        ),
        can_own_children(single_entry(n, is_dir)) == is_dir,
        path_verdict(n, depth == 0) == PathVerdict::Valid,
{
    assert(!has_forbidden_control(n));
    assert(serialized_component(n) is Some);
    lemma_line_shape(n, is_dir, depth, indent);
    lemma_line_layout(n, is_dir, depth, indent);
    lemma_entry_text_parses(n, is_dir, depth == 0);
    theorem_representable_names_are_valid_paths(n, depth == 0);
}

// ---------------------------------------------------------------------------
// T2: a generated body
// ---------------------------------------------------------------------------

/// One generated entry in emission order: its logical depth, its name, and
/// whether it is a directory.
pub(crate) struct Emitted {
    pub(crate) depth: nat,
    pub(crate) name: Seq<char>,
    pub(crate) is_dir: bool,
}

/// The entry after `prev` is at most one level deeper, and deeper only
/// below a directory.
pub(crate) open spec fn step_ok(prev: Emitted, next: Emitted) -> bool {
    next.depth <= prev.depth + 1 && (next.depth == prev.depth + 1 ==> prev.is_dir)
}

/// Each depth grows by at most one, and only directly below a directory.
pub(crate) open spec fn steps_ok(es: Seq<Emitted>) -> bool {
    forall|i: int| 0 < i < es.len() ==> #[trigger] step_ok(es[i - 1], es[i])
}

/// A preorder emission the dumper can produce: it starts at depth 0, every
/// depth is at most 256, every name is admitted at its level, and depths
/// grow by at most one and only below a directory.
pub(crate) open spec fn valid_emission(es: Seq<Emitted>) -> bool {
    &&& es.len() > 0 ==> es[0].depth == 0
    &&& forall|i: int|
        0 <= i < es.len() ==> (#[trigger] es[i]).depth <= 256 && dumpable_name(
            es[i].name,
            es[i].depth == 0,
        )
    &&& steps_ok(es)
}

/// The body lines of the emission, as `str::lines` yields them.
pub(crate) open spec fn body_lines(es: Seq<Emitted>, indent: nat) -> Seq<Seq<char>> {
    Seq::new(es.len(), |i: int| emitted_line(es[i].depth, indent, es[i].name, es[i].is_dir))
}

/// The depths of the emission.
pub(crate) open spec fn emitted_depths(es: Seq<Emitted>) -> Seq<int> {
    Seq::new(es.len(), |i: int| es[i].depth as int)
}

/// The kinds of the emission (`true` for a directory).
pub(crate) open spec fn emitted_kinds(es: Seq<Emitted>) -> Seq<bool> {
    Seq::new(es.len(), |i: int| es[i].is_dir)
}

/// The entry text the layout kernel hands to the entry kernel for a list
/// item: everything after `indent "- "`.
pub(crate) open spec fn item_text(l: Seq<char>) -> Seq<char> {
    l.skip(item_width(l) + 2)
}

/// What the parser recovers from the body line `l` laid out at `depth`: the
/// line is ordinary text and a list item; its entry text (the text after
/// `indent "- "`) is the canonical form of `name` and its directory marker;
/// the entry kernel parses that text, at that depth, as exactly one path
/// `name` of kind `is_dir` without a comment, whose line can own children
/// exactly when it is a directory; and the validator's structural path
/// rules accept `name` at that depth.
pub(crate) open spec fn line_recovers(
    l: Seq<char>,
    depth: int,
    name: Seq<char>,
    is_dir: bool,
) -> bool {
    &&& marker_class(encode_utf8(l)) == MarkerClass::Ordinary
    &&& is_list_item(l)
    &&& item_text(l) == entry_text(name, is_dir)
    &&& entry_spec(item_text(l), depth == 0) == Ok::<EntryView, FailureView>(
        single_entry(name, is_dir),
    )
    &&& can_own_children(single_entry(name, is_dir)) == is_dir
    &&& path_verdict(name, depth == 0) == PathVerdict::Valid
}

/// The layout half of T2: the layout kernel accepts the emitted body lines
/// with exactly the emitted depths, and the body satisfies the contract's
/// indentation and parentage rules with the indent size as its unit.
proof fn lemma_body_layout(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
        1 <= indent <= 16,
    ensures
        layout_lines(body_lines(es, indent), emitted_kinds(es)) matches Ok((state, depths))
            && depths == emitted_depths(es),
        valid_layout(body_lines(es, indent), emitted_kinds(es), emitted_depths(es), indent as int),
{
    let lines = body_lines(es, indent);
    let owns = emitted_kinds(es);
    let depths = emitted_depths(es);
    assert forall|i: int| 0 <= i < es.len() implies #[trigger] entry_shape(lines[i]) == Ok::<
        int,
        LayoutError,
    >(depths[i] * (indent as int)) by {
        let e = es[i];
        assert(!has_forbidden_control(e.name));
        lemma_line_layout(e.name, e.is_dir, e.depth, indent);
        assert(lines[i] == emitted_line(e.depth, indent, e.name, e.is_dir));
        assert(depths[i] == e.depth as int);
        assert((e.depth * indent) as int == (e.depth as int) * (indent as int));
    }
    assert(depth_rules(depths, owns)) by {
        assert forall|i: int| 0 < i < depths.len() && #[trigger] depths[i] == depths[i - 1]
            + 1 implies owns[i - 1] by {
            assert(step_ok(es[i - 1], es[i]));
        }
        assert forall|i: int| 0 < i < depths.len() implies #[trigger] depths[i] <= depths[i - 1]
            + 1 by {
            assert(step_ok(es[i - 1], es[i]));
        }
        assert forall|i: int| 0 <= i < depths.len() implies 0 <= #[trigger] depths[i] <= 256 by {
            assert(es[i].depth <= 256);
        }
    }
    assert(width_rule(lines, depths, indent as int));
    assert(valid_layout(lines, owns, depths, indent as int));
    theorem_layout_complete(lines, owns, depths, indent as int);
}

/// **T2.** For every emission the dumper can produce and every supported
/// indent size, the layout kernel accepts the emitted body lines with
/// exactly the emitted depths (when each line's ability to own children is
/// its kind, which is what the entry kernel reports for it), the body
/// satisfies the contract's indentation and parentage rules with the
/// indent size as its unit, and every line is ordinary text whose entry
/// text parses, at its recovered depth, to exactly its name and kind.
/// Since the hierarchy builder is a function of the per-line depths and
/// items alone, parsing recovers `(depth, name, kind)` for every line.
pub(crate) proof fn theorem_body_round_trip(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
        1 <= indent <= 16,
    ensures
        layout_lines(body_lines(es, indent), emitted_kinds(es)) matches Ok((state, depths))
            && depths == emitted_depths(es),
        valid_layout(body_lines(es, indent), emitted_kinds(es), emitted_depths(es), indent as int),
        forall|i: int|
            0 <= i < es.len() ==> line_recovers(
                #[trigger] body_lines(es, indent)[i],
                emitted_depths(es)[i],
                es[i].name,
                es[i].is_dir,
            ),
{
    lemma_body_layout(es, indent);
    assert forall|i: int| 0 <= i < es.len() implies line_recovers(
        #[trigger] body_lines(es, indent)[i],
        emitted_depths(es)[i],
        es[i].name,
        es[i].is_dir,
    ) by {
        let e = es[i];
        theorem_entry_line_round_trip(e.name, e.is_dir, e.depth, indent);
        assert(body_lines(es, indent)[i] == emitted_line(e.depth, indent, e.name, e.is_dir));
    }
}

// ---------------------------------------------------------------------------
// The parser's per-line loop
// ---------------------------------------------------------------------------

/// The per-line loop of `Parser::parse_guide_content`, composed from the
/// kernels' specifications: for each line in order, the layout kernel
/// (`layout_entry_line`, specified by [`layout_line`]) yields the depth,
/// unit, and entry text; the entry kernel (`parse_entry`, specified by
/// [`entry_spec`]) parses the entry text at that depth; and `advance`
/// records whether the line can own children. On success the result is
/// the final layout state and, per line, its depth and parsed entry; on
/// failure, the index of the first rejected line.
pub(crate) open spec fn parse_lines(lines: Seq<Seq<char>>) -> Result<
    (IndentState, Seq<(int, EntryView)>),
    int,
>
    decreases lines.len(),
{
    if lines.len() == 0 {
        Ok((initial_indent_state(), Seq::empty()))
    } else {
        let n = lines.len() - 1;
        match parse_lines(lines.drop_last()) {
            Err(i) => Err(i),
            Ok((state, parsed)) => match layout_line(state, lines[n]) {
                Err(_) => Err(n),
                Ok((depth, unit)) => match entry_spec(item_text(lines[n]), depth == 0) {
                    Err(_) => Err(n),
                    Ok(v) => Ok(
                        (
                            advance_state(depth, unit, can_own_children(v)),
                            parsed.push((depth as int, v)),
                        ),
                    ),
                },
            },
        }
    }
}

/// The items the parser's per-line loop recovers, or `None` when it rejects
/// a line.
pub(crate) open spec fn parsed_items(lines: Seq<Seq<char>>) -> Option<Seq<(int, EntryView)>> {
    match parse_lines(lines) {
        Ok((_, items)) => Some(items),
        Err(_) => None,
    }
}

/// The parsed items an emission should yield: per line, its depth and
/// exactly its name and kind without a comment.
pub(crate) open spec fn expected_items(es: Seq<Emitted>) -> Seq<(int, EntryView)> {
    Seq::new(es.len(), |i: int| (es[i].depth as int, single_entry(es[i].name, es[i].is_dir)))
}

/// The layout of a body depends only on the ownership flags of its lines.
proof fn lemma_layout_owns_prefix(lines: Seq<Seq<char>>, owns1: Seq<bool>, owns2: Seq<bool>)
    requires
        forall|i: int| 0 <= i < lines.len() ==> owns1[i] == owns2[i],
    ensures
        layout_lines(lines, owns1) == layout_lines(lines, owns2),
    decreases lines.len(),
{
    if lines.len() > 0 {
        lemma_layout_owns_prefix(lines.drop_last(), owns1, owns2);
    }
}

/// A prefix of a valid emission is a valid emission.
proof fn lemma_emission_prefix(es: Seq<Emitted>)
    requires
        valid_emission(es),
        es.len() > 0,
    ensures
        valid_emission(es.drop_last()),
{
    let p = es.drop_last();
    assert forall|i: int| 0 < i < p.len() implies #[trigger] step_ok(p[i - 1], p[i]) by {
        assert(p[i] == es[i] && p[i - 1] == es[i - 1]);
        assert(step_ok(es[i - 1], es[i]));
    }
    assert forall|i: int| 0 <= i < p.len() implies (#[trigger] p[i]).depth <= 256 && dumpable_name(
        p[i].name,
        p[i].depth == 0,
    ) by {
        assert(p[i] == es[i]);
    }
}

/// **T2 (parser loop).** The parser's per-line loop accepts every body the
/// dumper emits and recovers, for every line in order, exactly the emitted
/// depth, name, and kind, with no comment; its final layout state is the
/// layout kernel's.
pub(crate) proof fn theorem_parse_recovers_emission(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
        1 <= indent <= 16,
    ensures
        parse_lines(body_lines(es, indent)) == Ok::<(IndentState, Seq<(int, EntryView)>), int>(
            (
                layout_lines(body_lines(es, indent), emitted_kinds(es))->Ok_0.0,
                expected_items(es),
            ),
        ),
    decreases es.len(),
{
    let lines = body_lines(es, indent);
    let owns = emitted_kinds(es);
    lemma_body_layout(es, indent);
    if es.len() == 0 {
        assert(expected_items(es) =~= Seq::<(int, EntryView)>::empty());
    } else {
        let n = es.len() - 1;
        let prev = es.drop_last();
        lemma_emission_prefix(es);
        theorem_parse_recovers_emission(prev, indent);
        assert(body_lines(prev, indent) =~= lines.drop_last());
        lemma_layout_owns_prefix(lines.drop_last(), emitted_kinds(prev), owns);
        let (state, depths) = layout_lines(lines.drop_last(), owns)->Ok_0;
        assert(layout_lines(lines.drop_last(), owns) is Ok);
        let (depth, unit) = layout_line(state, lines[n])->Ok_0;
        assert(layout_line(state, lines[n]) is Ok);
        assert(layout_lines(lines, owns)->Ok_0.1 == depths.push(depth as int));
        assert(depths.push(depth as int)[n] == emitted_depths(es)[n]);
        assert(depth as int == es[n].depth as int);
        let e = es[n];
        assert(!has_forbidden_control(e.name));
        lemma_line_shape(e.name, e.is_dir, e.depth, indent);
        lemma_line_layout(e.name, e.is_dir, e.depth, indent);
        lemma_entry_text_parses(e.name, e.is_dir, e.depth == 0);
        assert(lines[n] == emitted_line(e.depth, indent, e.name, e.is_dir));
        assert(item_text(lines[n]) == entry_text(e.name, e.is_dir));
        assert(expected_items(prev).push((depth as int, single_entry(es[n].name, es[n].is_dir)))
            =~= expected_items(es));
    }
}

// ---------------------------------------------------------------------------
// T3: a generated tree
// ---------------------------------------------------------------------------

/// A generation tree node as `Dumper::build_tree` and
/// `Dumper::prepare_tree` leave it: the entry name, its kind, and its
/// children in their stored (sorted) order.
pub(crate) struct SpecTree {
    pub(crate) name: Seq<char>,
    pub(crate) is_dir: bool,
    pub(crate) children: Seq<SpecTree>,
}

/// The emission of `Dumper::format_tree` for the children `cs[i..]` of a
/// node whose children are at logical depth `depth`: each child's line, then
/// (when the child has children) its own children one level deeper, then
/// the next child.
pub(crate) open spec fn flatten_from(cs: Seq<SpecTree>, i: int, depth: nat) -> Seq<Emitted>
    decreases cs, cs.len() - i,
{
    if i < 0 || i >= cs.len() {
        Seq::empty()
    } else {
        let t = cs[i];
        seq![Emitted { depth, name: t.name, is_dir: t.is_dir }] + (if t.children.len() > 0 {
            flatten_from(t.children, 0, depth + 1)
        } else {
            Seq::empty()
        }) + flatten_from(cs, i + 1, depth)
    }
}

/// The whole emission for the generation root's children: `format_tree`
/// called on the root at depth 0.
pub(crate) open spec fn tree_emission(roots: Seq<SpecTree>) -> Seq<Emitted> {
    flatten_from(roots, 0, 0)
}

/// The children `cs[i..]` at logical depth `depth`, and everything below
/// them, are what generation admits: every depth is at most 256, every name
/// is admitted at its level, and only directories have children.
pub(crate) open spec fn well_formed_from(cs: Seq<SpecTree>, i: int, depth: nat) -> bool
    decreases cs, cs.len() - i,
{
    if i < 0 || i >= cs.len() {
        true
    } else {
        let t = cs[i];
        &&& depth <= 256
        &&& dumpable_name(t.name, depth == 0)
        &&& (!t.is_dir ==> t.children.len() == 0)
        &&& well_formed_from(t.children, 0, depth + 1)
        &&& well_formed_from(cs, i + 1, depth)
    }
}

/// A generation tree: the root's children and everything below them are
/// well formed, with the root's children at logical depth 0.
pub(crate) open spec fn well_formed_tree(roots: Seq<SpecTree>) -> bool {
    well_formed_from(roots, 0, 0)
}

/// The steps inside two emissions and across their junction are valid.
proof fn lemma_steps_concat(a: Seq<Emitted>, b: Seq<Emitted>)
    requires
        steps_ok(a),
        steps_ok(b),
        a.len() > 0 && b.len() > 0 ==> step_ok(a.last(), b[0]),
    ensures
        steps_ok(a + b),
{
    let c = a + b;
    assert forall|i: int| 0 < i < c.len() implies #[trigger] step_ok(c[i - 1], c[i]) by {
        if i < a.len() {
            assert(c[i] == a[i] && c[i - 1] == a[i - 1]);
            assert(step_ok(a[i - 1], a[i]));
        } else if i == a.len() {
            assert(c[i] == b[0] && c[i - 1] == a.last());
        } else {
            let j = i - a.len();
            assert(c[i] == b[j] && c[i - 1] == b[j - 1]);
            assert(step_ok(b[j - 1], b[j]));
        }
    }
}

/// Every entry of `flatten_from(cs, i, depth)` is admitted and at least as
/// deep as `depth`, the first one is at `depth`, and its steps are valid.
proof fn lemma_flatten_shape(cs: Seq<SpecTree>, i: int, depth: nat)
    requires
        well_formed_from(cs, i, depth),
    ensures
        ({
            let f = flatten_from(cs, i, depth);
            &&& forall|k: int|
                0 <= k < f.len() ==> depth <= (#[trigger] f[k]).depth <= 256 && dumpable_name(
                    f[k].name,
                    f[k].depth == 0,
                )
            &&& (f.len() > 0) == (0 <= i < cs.len())
            &&& f.len() > 0 ==> f[0].depth == depth
            &&& steps_ok(f)
        }),
    decreases cs, cs.len() - i,
{
    if 0 <= i < cs.len() {
        let t = cs[i];
        let e = Emitted { depth, name: t.name, is_dir: t.is_dir };
        let sub = if t.children.len() > 0 {
            flatten_from(t.children, 0, depth + 1)
        } else {
            Seq::empty()
        };
        let rest = flatten_from(cs, i + 1, depth);
        lemma_flatten_shape(cs, i + 1, depth);
        if t.children.len() > 0 {
            lemma_flatten_shape(t.children, 0, depth + 1);
        }
        let head = seq![e] + sub;
        assert(steps_ok(seq![e]));
        lemma_steps_concat(seq![e], sub);
        assert forall|k: int| 0 <= k < head.len() implies depth <= (#[trigger] head[k]).depth by {
            if k > 0 {
                assert(head[k] == sub[k - 1]);
            }
        }
        assert(head.last() == head[head.len() - 1]);
        lemma_steps_concat(head, rest);
        let f = flatten_from(cs, i, depth);
        assert(f == head + rest);
        assert forall|k: int| 0 <= k < f.len() implies depth <= (#[trigger] f[k]).depth <= 256
            && dumpable_name(f[k].name, f[k].depth == 0) by {
            if k == 0 {
                assert(f[k] == e);
            } else if k < head.len() {
                assert(f[k] == sub[k - 1]);
            } else {
                assert(f[k] == rest[k - head.len()]);
            }
        }
    }
}

/// **T3 (emission).** `format_tree`'s preorder emission of every generation
/// tree is an emission the body theorem covers.
pub(crate) proof fn theorem_tree_emission_valid(roots: Seq<SpecTree>)
    requires
        well_formed_tree(roots),
    ensures
        valid_emission(tree_emission(roots)),
        (tree_emission(roots).len() > 0) == (roots.len() > 0),
{
    lemma_flatten_shape(roots, 0, 0);
}

/// **T3 (round trip).** For every generation tree and every supported
/// indent size, the emission is valid; the layout kernel accepts the
/// emitted body with exactly the emitted depths; every line parses back to
/// exactly the name and kind of the tree node it was emitted for; and the
/// parser's per-line loop recovers exactly the emission.
pub(crate) proof fn theorem_tree_round_trip(roots: Seq<SpecTree>, indent: nat)
    requires
        well_formed_tree(roots),
        1 <= indent <= 16,
    ensures
        valid_emission(tree_emission(roots)),
        ({
            let es = tree_emission(roots);
            &&& layout_lines(body_lines(es, indent), emitted_kinds(es)) matches Ok((state, depths))
                && depths == emitted_depths(es)
            &&& forall|i: int|
                0 <= i < es.len() ==> line_recovers(
                    #[trigger] body_lines(es, indent)[i],
                    emitted_depths(es)[i],
                    es[i].name,
                    es[i].is_dir,
                )
            &&& parsed_items(body_lines(es, indent)) == Some(expected_items(es))
        }),
{
    theorem_tree_emission_valid(roots);
    theorem_body_round_trip(tree_emission(roots), indent);
    theorem_parse_recovers_emission(tree_emission(roots), indent);
}

// ---------------------------------------------------------------------------
// The flattening determines the tree
// ---------------------------------------------------------------------------

/// Every entry of `flatten_from(cs, i, depth)` is at least `depth` deep, the
/// first one is exactly at `depth`, and the emission is nonempty exactly
/// when `cs[i]` exists.
proof fn lemma_flatten_depths(cs: Seq<SpecTree>, i: int, depth: nat)
    ensures
        ({
            let f = flatten_from(cs, i, depth);
            &&& forall|k: int| 0 <= k < f.len() ==> depth <= (#[trigger] f[k]).depth
            &&& (f.len() > 0) == (0 <= i < cs.len())
            &&& f.len() > 0 ==> f[0].depth == depth
        }),
    decreases cs, cs.len() - i,
{
    if 0 <= i < cs.len() {
        let t = cs[i];
        let sub = if t.children.len() > 0 {
            flatten_from(t.children, 0, depth + 1)
        } else {
            Seq::empty()
        };
        let rest = flatten_from(cs, i + 1, depth);
        lemma_flatten_depths(cs, i + 1, depth);
        if t.children.len() > 0 {
            lemma_flatten_depths(t.children, 0, depth + 1);
        }
        let f = flatten_from(cs, i, depth);
        let e = Emitted { depth, name: t.name, is_dir: t.is_dir };
        assert(f == seq![e] + sub + rest);
        assert forall|k: int| 0 <= k < f.len() implies depth <= (#[trigger] f[k]).depth by {
            if k == 0 {
                assert(f[k] == e);
            } else if k < 1 + sub.len() {
                assert(f[k] == sub[k - 1]);
            } else {
                assert(f[k] == rest[k - 1 - sub.len()]);
            }
        }
    }
}

/// Two concatenations whose first parts are strictly deeper than `d` and
/// whose second parts start at depth `d` or shallower split at the same
/// place.
proof fn lemma_split_at_depth(
    a1: Seq<Emitted>,
    r1: Seq<Emitted>,
    a2: Seq<Emitted>,
    r2: Seq<Emitted>,
    d: nat,
)
    requires
        a1 + r1 == a2 + r2,
        forall|k: int| 0 <= k < a1.len() ==> (#[trigger] a1[k]).depth > d,
        forall|k: int| 0 <= k < a2.len() ==> (#[trigger] a2[k]).depth > d,
        r1.len() > 0 ==> r1[0].depth <= d,
        r2.len() > 0 ==> r2[0].depth <= d,
    ensures
        a1 == a2,
        r1 == r2,
{
    let c = a1 + r1;
    if a1.len() < a2.len() {
        if r1.len() > 0 {
            assert(c[a1.len() as int] == r1[0]);
            assert((a2 + r2)[a1.len() as int] == a2[a1.len() as int]);
        } else {
            assert(c.len() == a1.len());
        }
    }
    if a2.len() < a1.len() {
        if r2.len() > 0 {
            assert((a2 + r2)[a2.len() as int] == r2[0]);
            assert(c[a2.len() as int] == a1[a2.len() as int]);
        } else {
            assert((a2 + r2).len() == a2.len());
        }
    }
    assert(a1.len() == a2.len());
    assert(a1 =~= c.take(a1.len() as int));
    assert(a2 =~= (a2 + r2).take(a2.len() as int));
    assert(r1 =~= c.skip(a1.len() as int));
    assert(r2 =~= (a2 + r2).skip(a2.len() as int));
}

/// `flatten_from` is injective: equal emissions come from equal children
/// lists (names, kinds, and, recursively, children).
proof fn lemma_flatten_injective(
    cs1: Seq<SpecTree>,
    i1: int,
    cs2: Seq<SpecTree>,
    i2: int,
    depth: nat,
)
    requires
        0 <= i1 <= cs1.len(),
        0 <= i2 <= cs2.len(),
        flatten_from(cs1, i1, depth) == flatten_from(cs2, i2, depth),
    ensures
        cs1.skip(i1) == cs2.skip(i2),
    decreases cs1, cs1.len() - i1,
{
    lemma_flatten_depths(cs1, i1, depth);
    lemma_flatten_depths(cs2, i2, depth);
    if i1 == cs1.len() {
        assert(cs1.skip(i1) =~= Seq::<SpecTree>::empty());
        assert(cs2.skip(i2) =~= Seq::<SpecTree>::empty());
    } else {
        let t1 = cs1[i1];
        let t2 = cs2[i2];
        let f = flatten_from(cs1, i1, depth);
        let sub1 = if t1.children.len() > 0 {
            flatten_from(t1.children, 0, depth + 1)
        } else {
            Seq::empty()
        };
        let sub2 = if t2.children.len() > 0 {
            flatten_from(t2.children, 0, depth + 1)
        } else {
            Seq::empty()
        };
        let rest1 = flatten_from(cs1, i1 + 1, depth);
        let rest2 = flatten_from(cs2, i2 + 1, depth);
        let e1 = Emitted { depth, name: t1.name, is_dir: t1.is_dir };
        let e2 = Emitted { depth, name: t2.name, is_dir: t2.is_dir };
        assert(f == seq![e1] + sub1 + rest1);
        assert(f == seq![e2] + sub2 + rest2);
        assert(f[0] == e1 && f[0] == e2);
        assert(f.drop_first() =~= sub1 + rest1);
        assert(f.drop_first() =~= sub2 + rest2);
        lemma_flatten_depths(cs1, i1 + 1, depth);
        lemma_flatten_depths(cs2, i2 + 1, depth);
        if t1.children.len() > 0 {
            lemma_flatten_depths(t1.children, 0, depth + 1);
        }
        if t2.children.len() > 0 {
            lemma_flatten_depths(t2.children, 0, depth + 1);
        }
        lemma_split_at_depth(sub1, rest1, sub2, rest2, depth);
        lemma_flatten_injective(cs1, i1 + 1, cs2, i2 + 1, depth);
        if t1.children.len() > 0 && t2.children.len() > 0 {
            lemma_flatten_injective(t1.children, 0, t2.children, 0, depth + 1);
            assert(t1.children.skip(0) =~= t1.children);
            assert(t2.children.skip(0) =~= t2.children);
        } else if t1.children.len() > 0 {
            assert(sub1.len() > 0);
        } else if t2.children.len() > 0 {
            assert(sub2.len() > 0);
        } else {
            assert(t1.children =~= t2.children);
        }
        assert(t1 == t2);
        assert(cs1.skip(i1) =~= seq![t1] + cs1.skip(i1 + 1));
        assert(cs2.skip(i2) =~= seq![t2] + cs2.skip(i2 + 1));
    }
}

/// **T3 (uniqueness).** Distinct generation trees have distinct emissions:
/// the preorder sequence of `(depth, name, kind)` determines the tree.
pub(crate) proof fn theorem_tree_emission_injective(r1: Seq<SpecTree>, r2: Seq<SpecTree>)
    requires
        tree_emission(r1) == tree_emission(r2),
    ensures
        r1 == r2,
{
    lemma_flatten_injective(r1, 0, r2, 0, 0);
    assert(r1.skip(0) =~= r1);
    assert(r2.skip(0) =~= r2);
}

// ---------------------------------------------------------------------------
// T4: the wrapped document
// ---------------------------------------------------------------------------

/// `<agentic-navigation-guide>`, the opening line `dump` writes. (Opaque so
/// that only the lemmas that need its scalars see them.)
#[verifier::opaque]
pub(crate) open spec fn opening_marker_text() -> Seq<char> {
    seq![
        '<', 'a', 'g', 'e', 'n', 't', 'i', 'c', '-', 'n', 'a', 'v', 'i', 'g', 'a', 't', 'i', 'o',
        'n', '-', 'g', 'u', 'i', 'd', 'e', '>',
    ]
}

/// `</agentic-navigation-guide>`, the closing line `dump` writes. (Opaque
/// for the same reason.)
#[verifier::opaque]
pub(crate) open spec fn closing_marker_text() -> Seq<char> {
    seq![
        '<', '/', 'a', 'g', 'e', 'n', 't', 'i', 'c', '-', 'n', 'a', 'v', 'i', 'g', 'a', 't', 'i',
        'o', 'n', '-', 'g', 'u', 'i', 'd', 'e', '>',
    ]
}

/// The lines of the document `Dumper::dump_with_wrapper` writes around the
/// body lines `body`, as `str::lines` yields them.
pub(crate) open spec fn document_lines(body: Seq<Seq<char>>) -> Seq<Seq<char>> {
    seq![opening_marker_text()] + body + seq![closing_marker_text()]
}

/// The marker kernel's class of every line (over its UTF-8 bytes, as the
/// parser passes `line.as_bytes()`).
pub(crate) open spec fn line_classes(lines: Seq<Seq<char>>) -> Seq<MarkerClass> {
    Seq::new(lines.len(), |i: int| marker_class(encode_utf8(lines[i])))
}

/// Every scalar of `s` is ASCII.
pub(crate) open spec fn is_ascii_text(s: Seq<char>) -> bool {
    forall|i: int| 0 <= i < s.len() ==> (#[trigger] s[i] as u32) < 0x80
}

/// Neither marker line contains a line feed or a carriage return, and the
/// closing line ends with `>`.
proof fn lemma_marker_texts()
    ensures
        lf_free(opening_marker_text()),
        lf_free(closing_marker_text()),
        cr_free(opening_marker_text()),
        cr_free(closing_marker_text()),
        closing_marker_text().len() > 0,
        closing_marker_text().last() == '>',
{
    reveal(opening_marker_text);
    reveal(closing_marker_text);
    assert(is_ascii_text(opening_marker_text()));
    assert(is_ascii_text(closing_marker_text()));
}

/// The UTF-8 encoding of the opening line is `<agentic-navigation-guide>`.
proof fn lemma_opening_line_bytes()
    ensures
        encode_utf8(opening_marker_text()) == opening_name() + seq![0x3Eu8],
{
    reveal(opening_marker_text);
    let t = opening_marker_text();
    assert(is_ascii_text(t));
    lemma_encode_ascii(t);
    assert(encode_utf8(t) =~= opening_name() + seq![0x3Eu8]);
}

/// The UTF-8 encoding of the closing line is `</agentic-navigation-guide>`.
proof fn lemma_closing_line_bytes()
    ensures
        encode_utf8(closing_marker_text()) == closing_name() + seq![0x3Eu8],
{
    reveal(closing_marker_text);
    let t = closing_marker_text();
    assert(is_ascii_text(t));
    lemma_encode_ascii(t);
    assert(encode_utf8(t) =~= closing_name() + seq![0x3Eu8]);
}

/// A literal followed by `>` contains the literal at 0 and `>` right after.
proof fn lemma_literal_then_gt(name: Seq<u8>)
    ensures
        lit_at(name + seq![0x3Eu8], 0, name),
        lit_at(name + seq![0x3Eu8], 0int + name.len(), seq![0x3Eu8]),
        wsp_run(name + seq![0x3Eu8], 0, 0),
        wsp_run(
            name + seq![0x3Eu8],
            0int + name.len() + 1,
            (name + seq![0x3Eu8]).len() as int,
        ),
{
    let b = name + seq![0x3Eu8];
    assert(b.subrange(0, name.len() as int) =~= name);
    assert(b.subrange(name.len() as int, name.len() as int + 1) =~= seq![0x3Eu8]);
}

/// A line that starts with `<` is not prefixed by a byte-order mark.
proof fn lemma_not_bom_prefixed(b: Seq<u8>)
    requires
        b.len() > 0,
        b[0] == 0x3Cu8,
    ensures
        !bom_prefixed(b, opening_name()),
        !bom_prefixed(b, closing_name()),
{
    lemma_skip_wsp_to(b, 0, 0);
    if lit_at(b, 0, bom_bytes()) {
        lemma_lit_at_first(b, 0, bom_bytes());
    }
}

/// A marker literal at the start of a line, followed by `>` and nothing
/// else, makes the line a candidate for that marker and an exact marker
/// line: `active_opening_line` for the opening literal and `closing_line`
/// for the closing literal.
proof fn lemma_marker_witnesses(name: Seq<u8>)
    ensures
        candidate(name + seq![0x3Eu8], name),
        name == opening_name() ==> active_opening_line(name + seq![0x3Eu8]),
        name == closing_name() ==> closing_line(name + seq![0x3Eu8]),
{
    let b = name + seq![0x3Eu8];
    lemma_literal_then_gt(name);
    assert(wsp_run(b, 0, 0) && lit_at(b, 0, name));
    if name == opening_name() {
        assert(name.len() == 25);
        assert(wsp_run(b, 0, 0) && lit_at(b, 0, opening_name()) && lit_at(b, 0int + 25, seq![b'>'])
            && wsp_run(b, 0int + 26, b.len() as int));
    }
    if name == closing_name() {
        assert(name.len() == 26);
        assert(wsp_run(b, 0, 0) && lit_at(b, 0, closing_name()) && lit_at(b, 0int + 26, seq![b'>'])
            && wsp_run(b, 0int + 27, b.len() as int));
    }
}

/// The classification of a line that is an active opening marker.
proof fn lemma_class_of_opening(b: Seq<u8>)
    requires
        !bom_prefixed(b, opening_name()),
        !bom_prefixed(b, closing_name()),
        candidate(b, opening_name()),
        active_opening_line(b),
    ensures
        marker_class(b) == MarkerClass::Opening,
{
}

/// The classification of a line that is a closing marker.
proof fn lemma_class_of_closing(b: Seq<u8>)
    requires
        !bom_prefixed(b, opening_name()),
        !bom_prefixed(b, closing_name()),
        !candidate(b, opening_name()),
        candidate(b, closing_name()),
        closing_line(b),
    ensures
        marker_class(b) == MarkerClass::Closing,
{
}

/// `<agentic-navigation-guide>` is exactly the active opening marker.
proof fn lemma_opening_bytes_class()
    ensures
        marker_class(opening_name() + seq![0x3Eu8]) == MarkerClass::Opening,
{
    let b = opening_name() + seq![0x3Eu8];
    assert(b[0] == opening_name()[0]);
    lemma_not_bom_prefixed(b);
    lemma_marker_witnesses(opening_name());
    lemma_class_of_opening(b);
}

/// The opening line is exactly the active opening marker.
proof fn lemma_opening_line_class()
    ensures
        marker_class(encode_utf8(opening_marker_text())) == MarkerClass::Opening,
{
    lemma_opening_line_bytes();
    lemma_opening_bytes_class();
}

/// `</agentic-navigation-guide>` is exactly the closing marker.
proof fn lemma_closing_bytes_class()
    ensures
        marker_class(closing_name() + seq![0x3Eu8]) == MarkerClass::Closing,
{
    let b = closing_name() + seq![0x3Eu8];
    assert(closing_name().len() == 26);
    assert(b[0] == closing_name()[0]);
    assert(b[1] == closing_name()[1]);
    lemma_not_bom_prefixed(b);
    assert(!candidate(b, opening_name())) by {
        if candidate(b, opening_name()) {
            let p = choose|p: int| wsp_run(b, 0, p) && lit_at(b, p, opening_name());
            if p > 0 {
                assert(is_wsp(b[0]));
            }
            assert(b.subrange(0, 25)[1] == b[1]);
            assert(opening_name()[1] == 0x61u8);
        }
    }
    lemma_marker_witnesses(closing_name());
    lemma_class_of_closing(b);
}

/// The closing line is exactly the closing marker.
proof fn lemma_closing_line_class()
    ensures
        marker_class(encode_utf8(closing_marker_text())) == MarkerClass::Closing,
{
    lemma_closing_line_bytes();
    lemma_closing_bytes_class();
}

/// **T4.** Wrapping body lines that are all ordinary text in the opening and
/// closing lines of `dump` yields a document whose lines classify as
/// `Opening`, `Ordinary`..., `Closing`, which the envelope fold accepts as
/// one active (not ignored) guide block whose opening marker is line 1,
/// whose closing marker is the last line, with no prologue or epilogue, and
/// whose body (`lines[start + 1..end]` in `Parser::extract_guide_block`) is
/// exactly the given lines.
pub(crate) proof fn theorem_document_envelope(body: Seq<Seq<char>>)
    requires
        forall|i: int|
            0 <= i < body.len() ==> marker_class(encode_utf8(#[trigger] body[i]))
                == MarkerClass::Ordinary,
        body.len() + 2 <= usize::MAX,
    ensures
        ({
            let lines = document_lines(body);
            let cs = line_classes(lines);
            &&& cs.len() == body.len() + 2
            &&& cs[0] == MarkerClass::Opening
            &&& cs[body.len() as int + 1] == MarkerClass::Closing
            &&& forall|i: int| 1 <= i <= body.len() ==> #[trigger] cs[i] == MarkerClass::Ordinary
            &&& locate_envelope(cs) == Ok::<Envelope, EnvelopeError>(
                Envelope {
                    opening_line: 1,
                    closing_line: (body.len() + 2) as usize,
                    ignore: false,
                },
            )
            &&& lines.subrange(1, body.len() as int + 1) == body
        }),
{
    let lines = document_lines(body);
    let cs = line_classes(lines);
    let n = body.len() as int;
    lemma_opening_line_class();
    lemma_closing_line_class();
    assert(lines[0] == opening_marker_text());
    assert(lines[n + 1] == closing_marker_text());
    assert forall|i: int| 1 <= i <= n implies #[trigger] cs[i] == MarkerClass::Ordinary by {
        assert(lines[i] == body[i - 1]);
    }
    assert(single_envelope(cs, 0, n + 1, false));
    theorem_envelope_accepts_exactly(cs);
    assert(lines.subrange(1, n + 1) =~= body);
}

// ---------------------------------------------------------------------------
// The document text and its lines
// ---------------------------------------------------------------------------

/// The text `Dumper::dump` returns for an emission, which is also the
/// `dump --omit-xml-wrapper` fragment: every generated line, line feed
/// included, in emission order.
pub(crate) open spec fn body_text(es: Seq<Emitted>, indent: nat) -> Seq<char>
    decreases es.len(),
{
    if es.len() == 0 {
        Seq::empty()
    } else {
        let e = es.last();
        body_text(es.drop_last(), indent) + entry_line_spec(
            e.depth,
            indent,
            serialized(e.name),
            e.is_dir,
        )
    }
}

/// The text `Dumper::dump_with_wrapper` returns:
/// `format!("<agentic-navigation-guide>\n{content}</agentic-navigation-guide>")`
/// with `content` the [`body_text`].
pub(crate) open spec fn document_text(es: Seq<Emitted>, indent: nat) -> Seq<char> {
    opening_marker_text() + seq!['\n'] + body_text(es, indent) + closing_marker_text()
}

/// The pieces of `t` between line feeds, as `str::split('\n')` yields them:
/// always at least one, possibly empty.
pub(crate) open spec fn split_lf(t: Seq<char>) -> Seq<Seq<char>>
    decreases t.len(),
{
    if t.len() == 0 {
        seq![Seq::<char>::empty()]
    } else {
        let p = split_lf(t.drop_last());
        if t.last() == '\n' {
            p.push(Seq::empty())
        } else {
            p.update(p.len() - 1, p.last().push(t.last()))
        }
    }
}

/// `str::lines` on text without a carriage return, as the standard library
/// documents it: the text is split at every line feed, which is dropped; a
/// final line feed ends the last line instead of starting an empty one; and
/// empty text has no lines.
pub(crate) open spec fn lines_spec(t: Seq<char>) -> Seq<Seq<char>> {
    if t.len() == 0 {
        Seq::empty()
    } else if t.last() == '\n' {
        split_lf(t).drop_last()
    } else {
        split_lf(t)
    }
}

/// `lines.join("\n")`.
pub(crate) open spec fn join_lf(lines: Seq<Seq<char>>) -> Seq<char>
    decreases lines.len(),
{
    if lines.len() == 0 {
        Seq::empty()
    } else if lines.len() == 1 {
        lines[0]
    } else {
        join_lf(lines.drop_last()) + seq!['\n'] + lines.last()
    }
}

proof fn lemma_split_lf_nonempty(t: Seq<char>)
    ensures
        split_lf(t).len() >= 1,
    decreases t.len(),
{
    if t.len() > 0 {
        lemma_split_lf_nonempty(t.drop_last());
    }
}

/// Text without a line feed is one piece.
proof fn lemma_split_lf_free(b: Seq<char>)
    requires
        lf_free(b),
    ensures
        split_lf(b) == seq![b],
    decreases b.len(),
{
    if b.len() == 0 {
        assert(b =~= Seq::<char>::empty());
    } else {
        lemma_split_lf_free(b.drop_last());
        assert(b.last() == b[b.len() - 1]);
        assert(b.drop_last().push(b.last()) =~= b);
        assert(seq![b.drop_last()].update(0, b.drop_last().push(b.last())) =~= seq![b]);
    }
}

/// A line feed separates the pieces of the text before it from those of the
/// text after it.
proof fn lemma_split_lf_append(a: Seq<char>, b: Seq<char>)
    ensures
        split_lf(a + seq!['\n'] + b) == split_lf(a) + split_lf(b),
    decreases b.len(),
{
    let t = a + seq!['\n'] + b;
    if b.len() == 0 {
        assert(t.drop_last() =~= a);
        assert(t.last() == '\n');
        assert(split_lf(a).push(Seq::<char>::empty()) =~= split_lf(a) + seq![Seq::<char>::empty()]);
    } else {
        assert(t.drop_last() =~= a + seq!['\n'] + b.drop_last());
        assert(t.last() == b.last());
        lemma_split_lf_append(a, b.drop_last());
        lemma_split_lf_nonempty(b.drop_last());
        let x = split_lf(a);
        let y = split_lf(b.drop_last());
        if b.last() == '\n' {
            assert((x + y).push(Seq::<char>::empty()) =~= x + y.push(Seq::<char>::empty()));
        } else {
            assert((x + y).last() == y.last());
            assert((x + y).update((x + y).len() - 1, y.last().push(b.last())) =~= x + y.update(
                y.len() - 1,
                y.last().push(b.last()),
            ));
        }
    }
}

/// Joining line-feed-free lines with line feeds and splitting the result at
/// line feeds gives back the lines.
proof fn lemma_split_join(lines: Seq<Seq<char>>)
    requires
        lines.len() > 0,
        forall|i: int| 0 <= i < lines.len() ==> lf_free(#[trigger] lines[i]),
    ensures
        split_lf(join_lf(lines)) == lines,
    decreases lines.len(),
{
    if lines.len() == 1 {
        lemma_split_lf_free(lines[0]);
        assert(lines =~= seq![lines[0]]);
    } else {
        let p = lines.drop_last();
        assert forall|i: int| 0 <= i < p.len() implies lf_free(#[trigger] p[i]) by {
            assert(p[i] == lines[i]);
        }
        lemma_split_join(p);
        lemma_split_lf_append(join_lf(p), lines.last());
        lemma_split_lf_free(lines.last());
        assert(p + seq![lines.last()] =~= lines);
    }
}

/// A join of lines starts with the first line and, when the last line is
/// nonempty, ends with the last scalar of the last line.
proof fn lemma_join_ends(lines: Seq<Seq<char>>)
    requires
        lines.len() > 0,
    ensures
        join_lf(lines).len() >= lines[0].len(),
        forall|k: int| 0 <= k < lines[0].len() ==> #[trigger] join_lf(lines)[k] == lines[0][k],
        lines.last().len() > 0 ==> join_lf(lines).len() > 0 && join_lf(lines).last()
            == lines.last().last(),
    decreases lines.len(),
{
    if lines.len() > 1 {
        let p = lines.drop_last();
        lemma_join_ends(p);
        assert(p[0] == lines[0]);
        let j = join_lf(lines);
        assert(j == join_lf(p) + seq!['\n'] + lines.last());
        assert forall|k: int| 0 <= k < lines[0].len() implies #[trigger] j[k] == lines[0][k] by {
            assert(j[k] == join_lf(p)[k]);
        }
    }
}

/// The dumped document, split at line feeds before its closing text `t`,
/// is the opening line, the body lines, and `t`.
proof fn lemma_document_split(es: Seq<Emitted>, indent: nat, t: Seq<char>)
    requires
        valid_emission(es),
        lf_free(t),
    ensures
        split_lf(opening_marker_text() + seq!['\n'] + body_text(es, indent) + t) == seq![
            opening_marker_text(),
        ] + body_lines(es, indent) + seq![t],
    decreases es.len(),
{
    let o = opening_marker_text();
    if es.len() == 0 {
        assert(o + seq!['\n'] + body_text(es, indent) + t =~= o + seq!['\n'] + t);
        lemma_split_lf_append(o, t);
        lemma_marker_texts();
        lemma_split_lf_free(o);
        lemma_split_lf_free(t);
        assert(body_lines(es, indent) =~= Seq::<Seq<char>>::empty());
        assert(seq![o] + seq![t] =~= seq![o] + body_lines(es, indent) + seq![t]);
    } else {
        let p = es.drop_last();
        let e = es.last();
        let l = emitted_line(e.depth, indent, e.name, e.is_dir);
        lemma_last_line(es, indent);
        lemma_document_split(p, indent, l);
        assert(o + seq!['\n'] + body_text(es, indent) + t =~= (o + seq!['\n'] + body_text(p, indent)
            + l) + seq!['\n'] + t);
        lemma_split_lf_append(o + seq!['\n'] + body_text(p, indent) + l, t);
        lemma_split_lf_free(t);
        assert(seq![o] + body_lines(p, indent) + seq![l] + seq![t] =~= seq![o] + body_lines(
            es,
            indent,
        ) + seq![t]);
    }
}

/// The line emitted for the last entry of a nonempty valid emission is
/// free of line feeds and carriage returns, and the emission without that
/// entry is valid.
proof fn lemma_last_line(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
        es.len() > 0,
    ensures
        valid_emission(es.drop_last()),
        body_text(es, indent) == body_text(es.drop_last(), indent) + emitted_line(
            es.last().depth,
            indent,
            es.last().name,
            es.last().is_dir,
        ).push('\n'),
        body_lines(es, indent) == body_lines(es.drop_last(), indent).push(
            emitted_line(es.last().depth, indent, es.last().name, es.last().is_dir),
        ),
        lf_free(emitted_line(es.last().depth, indent, es.last().name, es.last().is_dir)),
        cr_free(emitted_line(es.last().depth, indent, es.last().name, es.last().is_dir)),
{
    lemma_emission_prefix(es);
    let e = es.last();
    assert(e == es[es.len() - 1]);
    assert(!has_forbidden_control(e.name));
    lemma_line_shape(e.name, e.is_dir, e.depth, indent);
    assert(body_lines(es, indent) =~= body_lines(es.drop_last(), indent).push(
        emitted_line(e.depth, indent, e.name, e.is_dir),
    ));
}

/// A scalar other than CR never encodes to a byte 0x0D.
proof fn lemma_scalar_no_cr(c: char)
    requires
        c != '\r',
    ensures
        forall|j: int| 0 <= j < encode_scalar(c as u32).len() ==> #[trigger] encode_scalar(
            c as u32,
        )[j] != 0x0Du8,
{
    let v = c as u32;
    if v < 0x80 {
        assert((v & 0x7Fu32) as u8 == v as u8) by (bit_vector)
            requires
                v < 0x80u32,
        ;
        assert(v != 0x0Du32);
        assert(v as u8 != 0x0Du8) by (bit_vector)
            requires
                v < 0x80u32,
                v != 0x0Du32,
        ;
    } else {
        let a = ((v >> 6u32) & 0x1Fu32) as u8;
        let b = ((v >> 12u32) & 0x0Fu32) as u8;
        let d = ((v >> 18u32) & 0x7u32) as u8;
        let x = (v & 0x3Fu32) as u8;
        let y = ((v >> 6u32) & 0x3Fu32) as u8;
        let z = ((v >> 12u32) & 0x3Fu32) as u8;
        assert((0xC0u8 | a) != 0x0Du8) by (bit_vector);
        assert((0xE0u8 | b) != 0x0Du8) by (bit_vector);
        assert((0xF0u8 | d) != 0x0Du8) by (bit_vector);
        assert((0x80u8 | x) != 0x0Du8) by (bit_vector);
        assert((0x80u8 | y) != 0x0Du8) by (bit_vector);
        assert((0x80u8 | z) != 0x0Du8) by (bit_vector);
    }
}

/// Text without a carriage return has no lone carriage return in its UTF-8
/// encoding.
proof fn lemma_encode_no_cr(t: Seq<char>)
    requires
        cr_free(t),
    ensures
        forall|j: int| 0 <= j < encode_utf8(t).len() ==> #[trigger] encode_utf8(t)[j] != 0x0Du8,
        no_lone_cr(encode_utf8(t)),
    decreases t.len(),
{
    if t.len() > 0 {
        let rest = t.drop_first();
        assert forall|i: int| 0 <= i < rest.len() implies #[trigger] rest[i] != '\r' by {
            assert(rest[i] == t[i + 1]);
        }
        lemma_encode_no_cr(rest);
        lemma_scalar_no_cr(t[0]);
        let h = encode_scalar(t[0] as u32);
        let b = encode_utf8(t);
        assert(b == h + encode_utf8(rest));
        assert forall|j: int| 0 <= j < b.len() implies #[trigger] b[j] != 0x0Du8 by {
            if j < h.len() {
                assert(b[j] == h[j]);
            } else {
                assert(b[j] == encode_utf8(rest)[j - h.len()]);
            }
        }
    }
    let b = encode_utf8(t);
    assert forall|i: int| 0 <= i < b.len() implies !#[trigger] is_lone_cr(b, i) by {
        assert(b[i] != 0x0Du8);
    }
}

/// No scalar of the dumped body text is a carriage return.
proof fn lemma_body_text_no_cr(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
    ensures
        cr_free(body_text(es, indent)),
    decreases es.len(),
{
    if es.len() > 0 {
        let p = es.drop_last();
        let e = es.last();
        lemma_last_line(es, indent);
        lemma_body_text_no_cr(p, indent);
        let l = emitted_line(e.depth, indent, e.name, e.is_dir);
        let line = l.push('\n');
        assert forall|k: int| 0 <= k < line.len() implies #[trigger] line[k] != '\r' by {
            if k < l.len() {
                assert(line[k] == l[k]);
            }
        }
        let bt = body_text(es, indent);
        assert(bt == body_text(p, indent) + line);
        assert forall|i: int| 0 <= i < bt.len() implies #[trigger] bt[i] != '\r' by {
            if i < body_text(p, indent).len() {
                assert(bt[i] == body_text(p, indent)[i]);
            } else {
                assert(bt[i] == line[i - body_text(p, indent).len()]);
            }
        }
    }
}

/// **Document text.** The text `dump_with_wrapper` returns for a valid
/// emission has no carriage return, so the parser's line-ending check
/// (`first_lone_cr_line`, specified by `no_lone_cr`) accepts it and
/// `str::lines` reduces to [`lines_spec`]; its lines are exactly
/// [`document_lines`] of the body lines. The body the parser re-joins with
/// line feeds (`lines[start + 1..end].join("\n")`) splits back into the same
/// body lines, and for a nonempty emission it is not blank, so the parser's
/// empty-block check passes.
pub(crate) proof fn theorem_document_text_lines(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
    ensures
        cr_free(document_text(es, indent)),
        no_lone_cr(encode_utf8(document_text(es, indent))),
        lines_spec(document_text(es, indent)) == document_lines(body_lines(es, indent)),
        es.len() > 0 ==> lines_spec(join_lf(body_lines(es, indent))) == body_lines(es, indent),
        es.len() > 0 ==> !all_white_space(join_lf(body_lines(es, indent))),
{
    let o = opening_marker_text();
    let c = closing_marker_text();
    let d = document_text(es, indent);
    let body = body_lines(es, indent);
    lemma_body_text_no_cr(es, indent);
    lemma_marker_texts();
    let bt = body_text(es, indent);
    assert(d == o + seq!['\n'] + bt + c);
    assert forall|i: int| 0 <= i < d.len() implies #[trigger] d[i] != '\r' by {
        if i < o.len() {
            assert(d[i] == o[i]);
        } else if i == o.len() {
            assert(d[i] == '\n');
        } else if i < o.len() + 1 + bt.len() {
            assert(d[i] == bt[i - o.len() - 1]);
        } else {
            assert(d[i] == c[i - o.len() - 1 - bt.len()]);
        }
    }
    lemma_encode_no_cr(d);
    lemma_document_split(es, indent, c);
    assert(d.last() == c.last());
    assert(d.last() != '\n');
    if es.len() > 0 {
        assert forall|i: int| 0 <= i < body.len() implies lf_free(#[trigger] body[i]) by {
            let e = es[i];
            assert(!has_forbidden_control(e.name));
            lemma_line_shape(e.name, e.is_dir, e.depth, indent);
        }
        lemma_split_join(body);
        lemma_join_ends(body);
        let last = es[es.len() - 1];
        assert(!has_forbidden_control(last.name));
        lemma_line_shape(last.name, last.is_dir, last.depth, indent);
        assert(body.last() == body[body.len() - 1]);
        assert(body.last().last() != '\n');
        let first = es[0];
        assert(!has_forbidden_control(first.name));
        lemma_line_shape(first.name, first.is_dir, first.depth, indent);
        let w0 = (first.depth * indent) as int;
        assert(join_lf(body)[w0] == body[0][w0]);
        assert(!is_white_space(join_lf(body)[w0]));
    }
}

// ---------------------------------------------------------------------------
// The whole textual round trip
// ---------------------------------------------------------------------------

/// **Round trip.** For every nonempty generation tree and every supported
/// indent size, the document `dump` writes:
///
/// - has no carriage return, so the parser's lone-CR check accepts it and
///   `str::lines` splits it as [`lines_spec`];
/// - splits into the opening line, the emitted body lines, and the closing
///   line, which the marker and envelope kernels accept as one active guide
///   block whose body is exactly the emitted lines;
/// - has a body that, re-joined with line feeds by the parser, splits back
///   into the same lines and is not blank;
/// - has a body the parser's per-line loop accepts, recovering line by line
///   the depth, name, and kind of each tree node in `format_tree` order;
///
/// and those recovered items determine the tree: every well-formed tree
/// whose body (at any supported indent size) parses to the same items is
/// the same tree.
pub(crate) proof fn theorem_dump_round_trip(roots: Seq<SpecTree>, indent: nat)
    requires
        well_formed_tree(roots),
        roots.len() > 0,
        1 <= indent <= 16,
        tree_emission(roots).len() + 2 <= usize::MAX,
    ensures
        ({
            let es = tree_emission(roots);
            let body = body_lines(es, indent);
            let text = document_text(es, indent);
            let lines = lines_spec(text);
            &&& cr_free(text)
            &&& no_lone_cr(encode_utf8(text))
            &&& lines == document_lines(body)
            &&& locate_envelope(line_classes(lines)) == Ok::<Envelope, EnvelopeError>(
                Envelope {
                    opening_line: 1,
                    closing_line: (body.len() + 2) as usize,
                    ignore: false,
                },
            )
            &&& lines.subrange(1, body.len() as int + 1) == body
            &&& lines_spec(join_lf(body)) == body
            &&& !all_white_space(join_lf(body))
            &&& parsed_items(body) == Some(expected_items(es))
        }),
        forall|other: Seq<SpecTree>, other_indent: nat|
            well_formed_tree(other) && 1 <= other_indent <= 16 && #[trigger] parsed_items(
                body_lines(tree_emission(other), other_indent),
            ) == parsed_items(body_lines(tree_emission(roots), indent)) ==> other == roots,
{
    let es = tree_emission(roots);
    let body = body_lines(es, indent);
    theorem_tree_round_trip(roots, indent);
    theorem_document_text_lines(es, indent);
    assert forall|i: int| 0 <= i < body.len() implies marker_class(encode_utf8(#[trigger] body[i]))
        == MarkerClass::Ordinary by {
        assert(line_recovers(body[i], emitted_depths(es)[i], es[i].name, es[i].is_dir));
    }
    theorem_document_envelope(body);
    assert forall|other: Seq<SpecTree>, other_indent: nat|
        well_formed_tree(other) && 1 <= other_indent <= 16 && #[trigger] parsed_items(
            body_lines(tree_emission(other), other_indent),
        ) == parsed_items(body_lines(tree_emission(roots), indent)) implies other == roots by {
        let eo = tree_emission(other);
        theorem_tree_round_trip(other, other_indent);
        lemma_expected_items_injective(eo, es);
        theorem_tree_emission_injective(other, roots);
    }
}

/// The expected items of an emission determine it.
proof fn lemma_expected_items_injective(a: Seq<Emitted>, b: Seq<Emitted>)
    requires
        expected_items(a) == expected_items(b),
    ensures
        a == b,
{
    assert(a.len() == b.len()) by {
        assert(expected_items(a).len() == a.len());
        assert(expected_items(b).len() == b.len());
    }
    assert forall|i: int| 0 <= i < a.len() implies a[i] == b[i] by {
        assert(expected_items(a)[i] == expected_items(b)[i]);
        assert(a[i].depth as int == b[i].depth as int);
    }
    assert(a =~= b);
}


// ---------------------------------------------------------------------------
// The document `init` writes
// ---------------------------------------------------------------------------

/// A Markdown line `init` writes around the guide block: no line feed, no
/// carriage return, and ordinary text for the marker kernel.
pub(crate) open spec fn frame_line(l: Seq<char>) -> bool {
    &&& lf_free(l)
    &&& cr_free(l)
    &&& marker_class(encode_utf8(l)) == MarkerClass::Ordinary
}

/// `# Agentic Navigation Guide`, the first line `init` writes. (Opaque: only the
/// lemma that needs its scalars sees them.)
#[verifier::opaque]
pub(crate) open spec fn init_title() -> Seq<char> {
    seq![
        '#', ' ', 'A', 'g', 'e', 'n', 't', 'i', 'c', ' ', 'N', 'a', 'v', 'i', 'g', 'a', 't', 'i',
        'o', 'n', ' ', 'G', 'u', 'i', 'd', 'e',
    ]
}

/// The first sentence of the description `init` writes. (Opaque: only the
/// lemma that needs its scalars sees them.)
#[verifier::opaque]
pub(crate) open spec fn init_intro() -> Seq<char> {
    seq![
        'T', 'h', 'i', 's', ' ', 'n', 'a', 'v', 'i', 'g', 'a', 't', 'i', 'o', 'n', ' ', 'g', 'u',
        'i', 'd', 'e', ' ', 'h', 'e', 'l', 'p', 's', ' ', 'A', 'I', ' ', 'c', 'o', 'd', 'i', 'n',
        'g', ' ', 'a', 's', 's', 'i', 's', 't', 'a', 'n', 't', 's', ' ', 'u', 'n', 'd', 'e', 'r',
        's', 't', 'a', 'n', 'd', ' ', 't', 'h', 'e', ' ', 's', 't', 'r', 'u', 'c', 't', 'u', 'r',
        'e', ' ', 'o', 'f', ' ', 't', 'h', 'i', 's', ' ', 'p', 'r', 'o', 'j', 'e', 'c', 't', '.',
    ]
}

/// The second sentence of the description `init` writes. (Opaque: only the
/// lemma that needs its scalars sees them.)
#[verifier::opaque]
pub(crate) open spec fn init_scope() -> Seq<char> {
    seq![
        'T', 'h', 'e', ' ', 'l', 'i', 's', 't', 'i', 'n', 'g', ' ', 'b', 'e', 'l', 'o', 'w', ' ',
        'm', 'a', 'y', ' ', 'b', 'e', ' ', 'i', 'n', 'c', 'o', 'm', 'p', 'l', 'e', 't', 'e', ' ',
        'a', 'n', 'd', ' ', 'h', 'i', 'g', 'h', 'l', 'i', 'g', 'h', 't', 's', ' ', 'k', 'e', 'y',
        ' ', 'f', 'i', 'l', 'e', 's', ' ', 'a', 'n', 'd', ' ', 'd', 'i', 'r', 'e', 'c', 't', 'o',
        'r', 'i', 'e', 's', '.',
    ]
}

/// The closing note `init` writes after the guide block. (Opaque: only the
/// lemma that needs its scalars sees them.)
#[verifier::opaque]
pub(crate) open spec fn init_note() -> Seq<char> {
    seq![
        'N', 'o', 't', 'e', ':', ' ', 'T', 'h', 'i', 's', ' ', 'g', 'u', 'i', 'd', 'e', ' ', 'w',
        'a', 's', ' ', 'a', 'u', 't', 'o', 'm', 'a', 't', 'i', 'c', 'a', 'l', 'l', 'y', ' ', 'g',
        'e', 'n', 'e', 'r', 'a', 't', 'e', 'd', ' ', 'a', 'n', 'd', ' ', 'm', 'a', 'y', ' ', 'n',
        'e', 'e', 'd', ' ', 'm', 'a', 'n', 'u', 'a', 'l', ' ', 'a', 'd', 'j', 'u', 's', 't', 'm',
        'e', 'n', 't', 's', '.',
    ]
}

/// The Markdown lines `init` writes before the guide block.
pub(crate) open spec fn init_prologue_lines() -> Seq<Seq<char>> {
    seq![init_title(), Seq::<char>::empty(), init_intro(), init_scope(), Seq::<char>::empty()]
}

/// The Markdown lines `init` writes after the guide block.
pub(crate) open spec fn init_epilogue_lines() -> Seq<Seq<char>> {
    seq![Seq::<char>::empty(), init_note()]
}

/// The text `init` writes (the raw-string `format!` in `cli/init.rs`): the
/// prologue lines joined by line feeds, a line feed, the
/// `dump_with_wrapper` [`document_text`], a line feed, the epilogue lines
/// joined by line feeds, and a final line feed.
pub(crate) open spec fn init_text(es: Seq<Emitted>, indent: nat) -> Seq<char> {
    join_lf(init_prologue_lines()) + seq!['\n'] + document_text(es, indent) + seq!['\n'] + join_lf(
        init_epilogue_lines(),
    ) + seq!['\n']
}

/// The empty line is ordinary text.
proof fn lemma_empty_line_ordinary()
    ensures
        frame_line(Seq::<char>::empty()),
{
    let b = encode_utf8(Seq::<char>::empty());
    assert(b.len() == 0);
    assert(skip_wsp(b, 0) == 0);
    assert(!candidate(b, opening_name())) by {
        if candidate(b, opening_name()) {
            let p = choose|p: int| wsp_run(b, 0, p) && lit_at(b, p, opening_name());
            assert(p + opening_name().len() <= b.len());
        }
    }
    assert(!candidate(b, closing_name())) by {
        if candidate(b, closing_name()) {
            let p = choose|p: int| wsp_run(b, 0, p) && lit_at(b, p, closing_name());
            assert(p + closing_name().len() <= b.len());
        }
    }
}

/// A line whose first scalar is ASCII and is neither a space, a tab, nor
/// `<` is ordinary text.
proof fn lemma_first_char_ordinary(l: Seq<char>)
    requires
        l.len() > 0,
        (l[0] as u32) < 0x80,
        l[0] != ' ',
        l[0] != '\t',
        l[0] != '<',
    ensures
        marker_class(encode_utf8(l)) == MarkerClass::Ordinary,
{
    let head = l.take(1);
    let tail = l.skip(1);
    assert(l =~= head + tail);
    encode_utf8_concat(head, tail);
    assert(head[0] == l[0]);
    lemma_encode_ascii(head);
    let b = encode_utf8(l);
    assert(b[0] == encode_utf8(head)[0]);
    let v = l[0] as u32;
    assert(b[0] == v as u8);
    assert(v != 0x20u32 && v != 0x09u32 && v != 0x3Cu32);
    assert(v as u8 != 0x20u8 && v as u8 != 0x09u8 && v as u8 != 0x3Cu8 && v as u8 != 0xEFu8)
        by (bit_vector)
        requires
            v < 0x80u32,
            v != 0x20u32,
            v != 0x09u32,
            v != 0x3Cu32,
    ;
    lemma_ordinary_bytes(b, 0);
}

/// Every line `init` writes around the guide block is a frame line.
proof fn lemma_init_frame_lines()
    ensures
        forall|i: int|
            0 <= i < init_prologue_lines().len() ==> frame_line(
                #[trigger] init_prologue_lines()[i],
            ),
        forall|i: int|
            0 <= i < init_epilogue_lines().len() ==> frame_line(
                #[trigger] init_epilogue_lines()[i],
            ),
{
    lemma_empty_line_ordinary();
    lemma_init_text_line(init_title());
    lemma_init_text_line(init_intro());
    lemma_init_text_line(init_scope());
    lemma_init_text_line(init_note());
    let pre = init_prologue_lines();
    let post = init_epilogue_lines();
    assert(pre[0] == init_title() && pre[2] == init_intro() && pre[3] == init_scope());
    assert(pre[1] == Seq::<char>::empty() && pre[4] == Seq::<char>::empty());
    assert(post[0] == Seq::<char>::empty() && post[1] == init_note());
}

/// Each prose line `init` writes is a frame line.
proof fn lemma_init_text_line(l: Seq<char>)
    requires
        l == init_title() || l == init_intro() || l == init_scope() || l == init_note(),
    ensures
        frame_line(l),
{
    reveal(init_title);
    reveal(init_intro);
    reveal(init_scope);
    reveal(init_note);
    assert(is_ascii_text(l));
    assert(lf_free(l));
    assert(cr_free(l));
    lemma_first_char_ordinary(l);
}

/// Joining carriage-return-free lines with line feeds gives
/// carriage-return-free text.
proof fn lemma_join_cr_free(lines: Seq<Seq<char>>)
    requires
        forall|i: int| 0 <= i < lines.len() ==> cr_free(#[trigger] lines[i]),
    ensures
        cr_free(join_lf(lines)),
    decreases lines.len(),
{
    if lines.len() > 1 {
        let p = lines.drop_last();
        assert forall|i: int| 0 <= i < p.len() implies cr_free(#[trigger] p[i]) by {
            assert(p[i] == lines[i]);
        }
        lemma_join_cr_free(p);
        let j = join_lf(lines);
        let a = join_lf(p);
        let l = lines.last();
        assert(cr_free(l));
        assert(j == a + seq!['\n'] + l);
        assert forall|i: int| 0 <= i < j.len() implies #[trigger] j[i] != '\r' by {
            if i < a.len() {
                assert(j[i] == a[i]);
            } else if i > a.len() {
                assert(j[i] == l[i - a.len() - 1]);
            }
        }
    } else if lines.len() == 1 {
        assert(cr_free(lines[0]));
    }
}

/// **Init text.** The text `init` writes for a valid emission has no
/// carriage return, so the parser's lone-CR check accepts it and
/// `str::lines` splits it as [`lines_spec`]: into `init`'s five prologue
/// lines, the lines of the `dump_with_wrapper` document, and `init`'s two
/// epilogue lines, every one of the prologue and epilogue lines being
/// ordinary text.
pub(crate) proof fn theorem_init_text_lines(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
    ensures
        cr_free(init_text(es, indent)),
        no_lone_cr(encode_utf8(init_text(es, indent))),
        lines_spec(init_text(es, indent)) == init_prologue_lines() + document_lines(
            body_lines(es, indent),
        ) + init_epilogue_lines(),
        forall|i: int|
            0 <= i < init_prologue_lines().len() ==> frame_line(
                #[trigger] init_prologue_lines()[i],
            ),
        forall|i: int|
            0 <= i < init_epilogue_lines().len() ==> frame_line(
                #[trigger] init_epilogue_lines()[i],
            ),
{
    let pre = init_prologue_lines();
    let post = init_epilogue_lines();
    let d = document_text(es, indent);
    let body = body_lines(es, indent);
    let p = join_lf(pre);
    let e = join_lf(post);
    let t = init_text(es, indent);
    lemma_init_frame_lines();
    theorem_document_text_lines(es, indent);
    lemma_marker_texts();
    // The document text splits into the document lines.
    lemma_document_split(es, indent, closing_marker_text());
    assert(d == opening_marker_text() + seq!['\n'] + body_text(es, indent) + closing_marker_text());
    // The frame lines split back out of their joins.
    assert forall|i: int| 0 <= i < pre.len() implies lf_free(#[trigger] pre[i]) by {
        assert(frame_line(pre[i]));
    }
    assert forall|i: int| 0 <= i < post.len() implies lf_free(#[trigger] post[i]) by {
        assert(frame_line(post[i]));
    }
    lemma_split_join(pre);
    lemma_split_join(post);
    // t == p + "\n" + (d + "\n" + (e + "\n" + empty)).
    let tail2 = e + seq!['\n'] + Seq::<char>::empty();
    let tail1 = d + seq!['\n'] + tail2;
    assert(t =~= p + seq!['\n'] + tail1);
    lemma_split_lf_append(p, tail1);
    lemma_split_lf_append(d, tail2);
    lemma_split_lf_append(e, Seq::<char>::empty());
    assert(split_lf(t) =~= pre + document_lines(body) + post + seq![Seq::<char>::empty()]);
    assert(t.last() == '\n');
    assert(lines_spec(t) =~= pre + document_lines(body) + post);
    // No carriage return anywhere.
    assert forall|i: int| 0 <= i < pre.len() implies cr_free(#[trigger] pre[i]) by {
        assert(frame_line(pre[i]));
    }
    assert forall|i: int| 0 <= i < post.len() implies cr_free(#[trigger] post[i]) by {
        assert(frame_line(post[i]));
    }
    lemma_join_cr_free(pre);
    lemma_join_cr_free(post);
    assert forall|i: int| 0 <= i < t.len() implies #[trigger] t[i] != '\r' by {
        let a = p.len() as int;
        let b = d.len() as int;
        if i < a {
            assert(t[i] == p[i]);
        } else if i == a {
            assert(t[i] == '\n');
        } else if i < a + 1 + b {
            assert(t[i] == d[i - a - 1]);
        } else if i == a + 1 + b {
            assert(t[i] == '\n');
        } else if i < a + 2 + b + e.len() {
            assert(t[i] == e[i - a - 2 - b]);
        } else {
            assert(t[i] == '\n');
        }
    }
    lemma_encode_no_cr(t);
}

/// Wrapping a guide block in ordinary prologue and epilogue lines moves its
/// marker lines and leaves its body and ignore flag unchanged: the
/// envelope fold accepts it with the opening marker right after the
/// prologue and the closing marker right before the epilogue.
pub(crate) proof fn theorem_framed_document_envelope(
    pre: Seq<Seq<char>>,
    body: Seq<Seq<char>>,
    post: Seq<Seq<char>>,
)
    requires
        forall|i: int|
            0 <= i < pre.len() ==> marker_class(encode_utf8(#[trigger] pre[i]))
                == MarkerClass::Ordinary,
        forall|i: int|
            0 <= i < body.len() ==> marker_class(encode_utf8(#[trigger] body[i]))
                == MarkerClass::Ordinary,
        forall|i: int|
            0 <= i < post.len() ==> marker_class(encode_utf8(#[trigger] post[i]))
                == MarkerClass::Ordinary,
        pre.len() + body.len() + post.len() + 2 <= usize::MAX,
    ensures
        ({
            let lines = pre + document_lines(body) + post;
            &&& locate_envelope(line_classes(lines)) == Ok::<Envelope, EnvelopeError>(
                Envelope {
                    opening_line: (pre.len() + 1) as usize,
                    closing_line: (pre.len() + body.len() + 2) as usize,
                    ignore: false,
                },
            )
            &&& lines.subrange(pre.len() + 1int, pre.len() + body.len() + 1int) == body
            &&& lines.take(pre.len() as int) == pre
            &&& lines.skip(pre.len() + body.len() + 2int) == post
        }),
{
    let lines = pre + document_lines(body) + post;
    let cs = line_classes(lines);
    let a = pre.len() as int;
    let n = body.len() as int;
    lemma_opening_line_class();
    lemma_closing_line_class();
    assert(lines[a] == opening_marker_text());
    assert(lines[a + n + 1] == closing_marker_text());
    assert forall|i: int| 0 <= i < cs.len() && i != a && i != a + n + 1 implies #[trigger] cs[i]
        == MarkerClass::Ordinary by {
        if i < a {
            assert(lines[i] == pre[i]);
        } else if i <= a + n {
            assert(lines[i] == body[i - a - 1]);
        } else {
            assert(lines[i] == post[i - a - n - 2]);
        }
    }
    assert(single_envelope(cs, a, a + n + 1, false));
    theorem_envelope_accepts_exactly(cs);
    assert(lines.subrange(a + 1, a + n + 1) =~= body);
    assert(lines.take(a) =~= pre);
    assert(lines.skip(a + n + 2) =~= post);
}

/// Every emitted body line is ordinary text for the marker kernel.
proof fn lemma_body_lines_ordinary(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
    ensures
        forall|i: int|
            0 <= i < body_lines(es, indent).len() ==> marker_class(
                encode_utf8(#[trigger] body_lines(es, indent)[i]),
            ) == MarkerClass::Ordinary,
{
    assert forall|i: int| 0 <= i < body_lines(es, indent).len() implies marker_class(
        encode_utf8(#[trigger] body_lines(es, indent)[i]),
    ) == MarkerClass::Ordinary by {
        let e = es[i];
        assert(!has_forbidden_control(e.name));
        lemma_line_layout(e.name, e.is_dir, e.depth, indent);
    }
}

/// The document half of the init round trip: the text `init` writes for a
/// valid emission has no carriage return, splits into its prologue, the
/// document lines, and its epilogue, and is accepted by the marker and
/// envelope kernels with the guide block on lines 6 through
/// `body.len() + 7` and exactly the body lines between them.
proof fn lemma_init_document(es: Seq<Emitted>, indent: nat)
    requires
        valid_emission(es),
        es.len() + 9 <= usize::MAX,
    ensures
        ({
            let body = body_lines(es, indent);
            let text = init_text(es, indent);
            let lines = lines_spec(text);
            &&& cr_free(text)
            &&& no_lone_cr(encode_utf8(text))
            &&& lines == init_prologue_lines() + document_lines(body) + init_epilogue_lines()
            &&& locate_envelope(line_classes(lines)) == Ok::<Envelope, EnvelopeError>(
                Envelope {
                    opening_line: 6,
                    closing_line: (body.len() + 7) as usize,
                    ignore: false,
                },
            )
            &&& lines.subrange(6, body.len() as int + 6) == body
        }),
{
    let body = body_lines(es, indent);
    let pre = init_prologue_lines();
    let post = init_epilogue_lines();
    lemma_body_lines_ordinary(es, indent);
    theorem_init_text_lines(es, indent);
    assert forall|i: int| 0 <= i < pre.len() implies marker_class(encode_utf8(#[trigger] pre[i]))
        == MarkerClass::Ordinary by {
        assert(frame_line(pre[i]));
    }
    assert forall|i: int| 0 <= i < post.len() implies marker_class(encode_utf8(#[trigger] post[i]))
        == MarkerClass::Ordinary by {
        assert(frame_line(post[i]));
    }
    theorem_framed_document_envelope(pre, body, post);
}

/// **Init round trip.** For every nonempty generation tree and every
/// supported indent size, the file `init` writes has no carriage return;
/// it splits into `init`'s prologue lines, the opening line, the emitted
/// body lines, the closing line, and `init`'s epilogue lines; the marker and
/// envelope kernels accept it as one active guide block on lines 6 through
/// `body.len() + 7`, whose body is exactly the emitted lines; and from there
/// the parser recovers the same items as from the `dump` document, which
/// determine the tree (`theorem_dump_round_trip`).
pub(crate) proof fn theorem_init_round_trip(roots: Seq<SpecTree>, indent: nat)
    requires
        well_formed_tree(roots),
        roots.len() > 0,
        1 <= indent <= 16,
        tree_emission(roots).len() + 9 <= usize::MAX,
    ensures
        ({
            let es = tree_emission(roots);
            let body = body_lines(es, indent);
            let text = init_text(es, indent);
            let lines = lines_spec(text);
            &&& cr_free(text)
            &&& no_lone_cr(encode_utf8(text))
            &&& lines == init_prologue_lines() + document_lines(body) + init_epilogue_lines()
            &&& locate_envelope(line_classes(lines)) == Ok::<Envelope, EnvelopeError>(
                Envelope {
                    opening_line: 6,
                    closing_line: (body.len() + 7) as usize,
                    ignore: false,
                },
            )
            &&& lines.subrange(6, body.len() as int + 6) == body
            &&& lines_spec(join_lf(body)) == body
            &&& !all_white_space(join_lf(body))
            &&& parsed_items(body) == Some(expected_items(es))
        }),
{
    let es = tree_emission(roots);
    theorem_tree_emission_valid(roots);
    lemma_init_document(es, indent);
    theorem_parse_recovers_emission(es, indent);
    theorem_document_text_lines(es, indent);
}

// ---------------------------------------------------------------------------
// The hypotheses are satisfiable
// ---------------------------------------------------------------------------

/// A generation tree with a root directory `src` holding the regular file
/// `a#b`, whose canonical form is quoted.
pub(crate) open spec fn example_tree() -> Seq<SpecTree> {
    seq![
        SpecTree {
            name: seq!['s', 'r', 'c'],
            is_dir: true,
            children: seq![SpecTree { name: seq!['a', '#', 'b'], is_dir: false, children: seq![] }],
        },
    ]
}

/// The example is a nonempty well-formed generation tree with two entries,
/// so the round-trip theorems apply to it (their hypotheses are not
/// vacuous).
proof fn lemma_example_tree()
    ensures
        well_formed_tree(example_tree()),
        tree_emission(example_tree()).len() == 2,
        example_tree().len() > 0,
{
    let src = seq!['s', 'r', 'c'];
    let file = seq!['a', '#', 'b'];
    assert(!has_forbidden_control(src)) by {
        if has_forbidden_control(src) {
            let k = choose|k: int| 0 <= k < src.len() && #[trigger] is_forbidden_control(src[k]);
            assert(!is_forbidden_control(src[k]));
        }
    }
    assert(!has_forbidden_control(file)) by {
        if has_forbidden_control(file) {
            let k = choose|k: int| 0 <= k < file.len() && #[trigger] is_forbidden_control(file[k]);
            assert(!is_forbidden_control(file[k]));
        }
    }
    assert(!is_dot_component(src)) by {
        if src == seq!['.'] || src == seq!['.', '.'] {
            assert(src.len() != 1 && src.len() != 2);
        }
    }
    assert(!is_dot_component(file)) by {
        if file == seq!['.'] || file == seq!['.', '.'] {
            assert(file.len() != 1 && file.len() != 2);
        }
    }
    let roots = example_tree();
    let children = roots[0].children;
    let leaf = children[0];
    assert(roots[0].name == src && roots[0].is_dir);
    assert(leaf.name == file && !leaf.is_dir && leaf.children.len() == 0);
    assert(dumpable_name(src, true));
    assert(dumpable_name(file, false));
    assert(well_formed_from(leaf.children, 0, 2));
    assert(well_formed_from(children, 1, 1));
    assert(well_formed_from(children, 0, 1));
    assert(well_formed_from(roots, 1, 0));
    assert(well_formed_from(roots, 0, 0));
    lemma_flatten_depths(children, 1, 1);
    lemma_flatten_depths(roots, 1, 0);
    assert(flatten_from(children, 0, 1).len() == 1);
    assert(tree_emission(roots).len() == 2);
}

/// The example's dumped document round-trips at indent size 2.
proof fn lemma_example_round_trip()
    ensures
        parsed_items(body_lines(tree_emission(example_tree()), 2)) == Some(
            expected_items(tree_emission(example_tree())),
        ),
{
    lemma_example_tree();
    theorem_dump_round_trip(example_tree(), 2);
}

} // verus!
