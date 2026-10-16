# Vietnamese text in Script IR

> **Scope.** Unicode policy for script text: NFC normalization at every entry path, byte versus
> code point versus UTF-16 versus grapheme offsets, equality versus search versus sort, the
> whitespace and invisible-character policy, pronunciation overrides as typed data and
> Vietnamese test vectors. Use when code stores, compares, digests, slices, searches, measures or
> anchors anything to dialogue, character or title text, or when fixtures contain Vietnamese.

Product context: the [Script IR dialogue model](../../../../docs/architecture/script-ir.md#purpose)
(spoken text and optional pronunciation overrides),
[casting/performance guidance](../../../../docs/product/business-rules.md#casting-and-performance)
(pronunciation rules stay inspectable) and the
[Script/editor risks](../../../../docs/assurance/script-editor-acceptance.md#risks--unknowns) (confirm
languages and pronunciation needs with real fixtures). Fonts, diacritic clipping and display
belong to [`cantos-ui-design`](../../cantos-ui-design/references/typography.md); editor input
and selection mapping to [`script-editor.md`](../../cantos-leptos-web/references/script-editor.md).
The policy below is proposed; record it with the first Script IR ADR.

## Contents

1. Three facts that break naive code
2. Normalization policy
3. Test vectors
4. Offsets and lengths
5. Equality, search and sort
6. Whitespace and invisible characters
7. Pronunciation overrides are typed data
8. Tests

## 1. Three facts that break naive code

1. **The same visible text has several byte sequences.** Precomposed (NFC), decomposed (NFD,
   produced by some input-method settings and older macOS file paths) and the mixed form that
   Windows-1258 decoding yields are all common. Unnormalized, each looks like a changed line, a
   new digest and a paid re-render.
2. **One visible letter is 1–3 code points and 1–3 UTF-8 bytes.** `ờ` is one code point (3 bytes)
   in NFC and three code points in NFD. Byte, code point, UTF-16 and grapheme counts all differ.
3. **Normalization does not unify everything.** Tone placement variants (`hòa` / `hoà`,
   `thúy` / `thuý`) are different strings in every normalization form, and `đ` (U+0111) has no
   decomposition, so "strip the diacritics" leaves it untouched.

## 2. Normalization policy

| Rule | Why | Good | Counterexample | Oracle |
|---|---|---|---|---|
| derived text is NFC, enforced by the type (`SpokenText`, `DisplayText`) | one value, one byte sequence, one digest | `SpokenText::new` normalizes and validates | a `String` field normalized "usually" | NFD vectors through every entry path |
| every entry path constructs through that type | one forgotten path reopens false diffs | editor save, extraction, adaptation output, structured import, Narrative Forge adapter, fixture loader, row decoder | the fixture loader building structs directly | a test per entry path |
| stored revisions are validated, not re-normalized, on read | a stored non-NFC revision is corruption; silently fixing it changes its digest | `RevisionCorrupted { id }` | normalize-on-read | tampered-row test |
| preserved source bytes are never normalized | provenance must be byte-exact | checksum of the upload equals the stored object | normalizing the upload before hashing | checksum test |
| NFC, never NFKC | NFKC rewrites meaning: `…` becomes `...`, full-width and superscript forms change | NFC keeps `…` | NFKC "to be safe" | vector for `…` |
| tone placement is preserved | it is the author's orthography; rewriting is an edit | `hoà` stays `hoà` | a normalizer that moves tone marks | vector pair stays distinct |

Record the normalization library (a `unicode-normalization` crate is a candidate) and the
Unicode version it implements. Unicode's normalization stability policy keeps results stable for
assigned characters, but pin and test rather than assume.

Combining-mark order is handled by normalization: `a` + U+0302 + U+0323 and `a` + U+0323 +
U+0302 both normalize to `ậ` (U+1EAD), because canonical ordering sorts marks by combining class.
Never compare or hash before normalizing.

## 3. Test vectors

Use these in fixtures and tests; each NFC/NFD pair must produce identical `SpokenText` bytes,
identical `SpokenContent` bytes, an empty revision diff and the same content digest.

| Text | NFC code points / UTF-8 bytes | NFD code points / UTF-8 bytes |
|---|---|---|
| `Người` | 5 / 8 | 8 / 11 |
| `Người dẫn chuyện` | 16 / 23 | 23 / 30 |
| `Ánh đèn cuối sân khấu` | 21 / 29 | 28 / 36 |
| `Ngày mai, mình có diễn tiếp không?` | 34 / 42 | 42 / 50 |
| `Vọng Đài` | 8 / 12 | 10 / 13 (`Đ` does not decompose) |

```text
"Người"  NFC  U+004E U+0067 U+01B0 U+1EDD U+0069
         NFD  U+004E U+0067 U+0075 U+031B U+006F U+031B U+0300 U+0069
Windows-1258 bytes 4E 67 FD F5 CC 69 decode to
              U+004E U+0067 U+01B0 U+01A1 U+0300 U+0069   (neither NFC nor NFD; NFC → "Người")
```

Tone marks are U+0300 (huyền), U+0301 (sắc), U+0303 (ngã), U+0309 (hỏi) and U+0323 (nặng);
vowel modifiers are U+0302 (circumflex), U+0306 (breve) and U+031B (horn). A property generator
can build syllables from base vowels, these marks and consonants including `đ`, then emit each
character composed or decomposed at random.

Distinct-on-purpose pairs (must stay different): `hòa` / `hoà`, `thúy` / `thuý`, `Đ` (U+0110) /
`Ð` (U+00D0, a look-alike from another alphabet).

## 4. Offsets and lengths

| Unit | Native to | `Người` (NFC) | `Người` (NFD) |
|---|---|---|---|
| UTF-8 bytes | Rust `str`, most storage | 8 | 11 |
| code points | Rust `chars()`, PostgreSQL `length()` on UTF-8 text | 5 | 8 |
| UTF-16 code units | browser DOM and JavaScript, Kotlin/JVM `String` | 5 | 8 |
| extended grapheme clusters | what a reader calls a character | 5 | 5 |

- An offset that crosses a boundary carries its unit in its type and wire name
  (`Utf8ByteRange`, `GraphemeRange`, `"start_utf16"`). A bare `"start": 12` is ambiguous between
  Rust, the browser and Kotlin.
- Convert at the edge. The Leptos editor reports UTF-16 selections; the server stores its own
  unit; conversion lives in one tested function per pair of units.
- Prefer anchors that are not offsets at all (stable IDs, terms with an occurrence index), because
  any edit earlier in a line moves every offset after it.
- `&text[..n]` panics when `n` falls inside a multi-byte character such as `ờ`. Truncate previews
  on grapheme boundaries (a `unicode-segmentation` crate is a candidate).
- User-facing length limits count graphemes; storage and request limits count bytes. Provider
  request limits belong to the pipeline's adapter.

## 5. Equality, search and sort

| Purpose | Form | Example |
|---|---|---|
| equality, diff, digest, `SpokenContent` | NFC plus the whitespace policy, exact code points | `hòa` ≠ `hoà` |
| Studio search | folded: NFD, remove nonspacing marks, map `đ`/`Đ` → `d`, Unicode lowercase | `nguoi dan` finds `Người dẫn chuyện`; `vong dai` finds `Vọng Đài` |
| sorting names for display | Vietnamese collation (an ICU-based collator is a candidate) | `a ă â b c d đ e ê …`, not code point order |
| canonical order inside encodings | stable ID | never by name or folded text |

Folded text is an index, never content: it is not stored as dialogue text, digested or diffed.
Without the explicit `đ` mapping, folding `Vọng Đài` yields `vong đai` and search misses it.
Use Unicode lowercasing; `to_ascii_lowercase` leaves `Ấ` unchanged.

## 6. Whitespace and invisible characters

Proposed `SpokenText` policy. Import applies it with conversion notes; the editor applies it on
save.

| Input | Policy | Reason |
|---|---|---|
| leading and trailing whitespace | trim | invisible, changes bytes |
| runs of `White_Space`, including U+00A0 (common in DOCX), U+3000, tab, U+2028 | collapse to one U+0020 | one visible text, one encoding |
| line breaks inside a line | collapse to U+0020 | a dialogue is one utterance; separate utterances are separate dialogues |
| U+200B, U+FEFF inside text, U+00AD | remove | invisible format characters that only change bytes |
| other format characters (ZWJ, bidirectional controls) | reject with `InvisibleFormatCharacter { code_point }` | no Vietnamese need; a reviewed exception may add one |
| C0 and C1 controls | reject | never valid spoken text |
| punctuation (`…` / `...`, `“”` / `""`, `–` / `-`) | keep exactly | the author's choice; it may affect prosody |

A deliberate pause is not whitespace. If the schema models pauses, they are typed delivery data,
so collapsing whitespace never deletes a performance instruction. Whether display needs line
breaks inside a dialogue is a product question; report it rather than encoding a guess.

## 7. Pronunciation overrides are typed data

Inline markup in spoken text is a counterexample:

```text
"Chào mừng đến <phoneme ph=\"…\">Vọng Đài</phoneme>"    provider syntax inside content
"Hẹn gặp ở [TP.HCM|Thành phố Hồ Chí Minh] nhé."        a private mini-language inside content
```

It leaks provider syntax into the IR, shows up in displays and exports, makes a text fix and a
pronunciation fix indistinguishable in the diff and breaks when the provider changes syntax.
Model the override beside the text instead:

```rust
// Illustrative and proposed.
pub struct PronunciationOverride {
    target: OverrideTarget,
    rendering: Rendering,
}

pub enum OverrideTarget {
    Term { surface: SpokenText, occurrence: Occurrence }, // Occurrence::All | Nth(NonZeroU16)
}

pub enum Rendering {
    Respelling(SpokenText),                          // "TP.HCM" → "Thành phố Hồ Chí Minh"
    Phonemes { alphabet: PhonemeAlphabet, value: String },
    SayAs(SayAs),                                    // Characters | Cardinal | Ordinal | …
}
```

- The validator checks that the surface occurs in the line (NFC comparison) and that an `Nth`
  occurrence exists; an edit that removes the term yields `PronunciationTargetMissing { term }`
  instead of a silently dropped override. Overlapping targets are rejected.
- Line overrides belong to Script IR and enter `SpokenContent`. Adaptation- or character-wide
  pronunciation rules are a separately versioned profile resolved when production inputs freeze;
  where character pronunciation guidance lives is not yet decided in the docs.
- Mapping a `Rendering` to what a provider supports is the pipeline adapter's job
  ([architecture § Layers and contexts](../../../../docs/architecture/overview.md#layers-and-contexts)); an
  unsupported rendering is an explicit finding, never a silent drop.
- Mixed-language words, such as an English name in a `vi-VN` line, use an override. Per-span
  language tags are not modeled for the MVP; report a need rather than inventing one.

## 8. Tests

| Claim | Test | Evidence level |
|---|---|---|
| NFD and Windows-1258 forms never look like edits | vector table through every entry path → equal bytes, digest and empty diff | `example-tested` |
| normalization is total and idempotent | property over generated Vietnamese syllables, random composition | `property-tested` |
| tone placement variants stay distinct | vector pairs → different bytes | `example-tested` |
| whitespace policy | table test per row, exact output or exact rejection variant | `example-tested` |
| search folding | `nguoi dan`, `vong dai` hit; folded text never reaches the digest | `example-tested` |
| offset conversion | UTF-16 ↔ UTF-8 ↔ grapheme for the vectors; grapheme truncation never panics | `example-tested`, `property-tested` |
| pronunciation targets | removed term → `PronunciationTargetMissing`; overlap → rejection | `example-tested` |

Rendered diacritics, line height and font coverage are not proven by any of these; they need the
UI evidence loop in [`cantos-ui-design`](../../cantos-ui-design/SKILL.md).
