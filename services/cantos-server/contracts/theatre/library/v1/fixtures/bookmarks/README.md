# Release-bound bookmark fixtures

Original synthetic fixtures for the v1.2 bookmark resource (#299): the `BookmarkWrite` and
`BookmarkDelete` bodies, the accepted/deleted/refused receipts and the required-nullable
`presentation` page. They are separate from the default v1 corpus so that corpus's count and
strict decoding stay unchanged. The Kotlin fixture generator exposes this directory as
`libraryBookmarkFixtures`; Rust schema/DTO conformance tests load it explicitly.

`bookmarks.json` shows an item whose exact release is no longer current (`access_unavailable`,
`presentation:null`): the bookmark is retained, never remapped to another release.

Contract: [Library v1](../../README.md).
