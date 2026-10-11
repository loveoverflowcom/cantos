# Synthetic manuscript fixtures

The Vietnamese text in this directory was written for Cantos regression tests. It is original
test material, not a copied manuscript or an AI/provider output. The TXT expectation is authored
independently as an exact extraction oracle; it must never be regenerated from the parser.

These fixtures distinguish lexical source labels from resolved characters, preserve unlabeled
dialogue and ambiguous brackets, and keep an instruction-like sentence and HTML as inert source
text. DOCX positive and hostile archives are built independently by the integration test helper,
with minimal OOXML, stored/deflated entries, and explicit malformed header mutations.
