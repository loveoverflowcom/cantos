# Browser capture, extraction and OCR

Read only for source intake or tool setup. The authoritative shared artifact and recovery rules
are in [story workspace](../../../../docs/production/story-workspace.md).

## Select and install the needed tool

Inspect the available interpreter, executable and installed versions first. Reuse a working
Trafilatura environment; this machine previously used
`$HOME/.local/share/trafilatura-venv/bin/python`, but verify it exists rather than assuming.
Do not add crawling/OCR packages to the Cantos Rust application or system Python.

- HTML with selectable text: use Trafilatura, installed from PyPI in a dedicated user virtual
  environment. Check `python3 -m venv <tool-env>`, then `<tool-env>/bin/python -m pip install
  trafilatura`. Quote actual paths; record the resolved package version and `pip check` result.
  Reuse working versions instead of upgrading on every capture. The official
  [installation guide](https://trafilatura.readthedocs.io/en/latest/installation.html) owns current
  Python/version requirements.
- If `venv` cannot seed pip, use an already installed `uv` or `virtualenv`, or install `virtualenv`
  into a task-local `pip --target` bootstrap directory and invoke it via that directory's
  `PYTHONPATH`. Keep that environment isolated and remove only the temporary bootstrap after
  the tool works. Do not use `--break-system-packages`. OS package installation must use the
  available package manager and normal runtime permissions; do not stall useful local work if
  admin installation is unavailable.
- Scans/images: check `tesseract --version` and `tesseract --list-langs`; Vietnamese needs `vie`
  and may also use `eng`. Install the engine and language data from the OS package manager or
  official distribution when needed. A Python OCR wrapper alone does not install the engine.
  Consult [Tesseract installation](https://tesseract-ocr.github.io/tessdoc/Installation.html) and
  [language data](https://tesseract-ocr.github.io/tessdoc/Data-Files.html). Use the PDF skill for
  scanned PDF page rendering. Do not OCR an ordinary HTML chapter.
- Validate setup with a small original Vietnamese HTML snippet or original image, not a claim
  that a package import proves real extraction/OCR quality. Record OCR language/model versions,
  page order, source image references and uncertainties.

## Browser-first capture

Read the available computer-use documentation at runtime. APIs differ between browser providers;
never invent methods or switch to shell browser automation behind the user's request. Keep
navigation bounded to the requested work/range. A browser-rendered article may work even when a
command-line request returns 403; a block page is not a reason to bypass authentication/CAPTCHA.

1. Observe the page, derive selectors from its actual DOM, and inspect the content container plus
   heading and chapter-navigation links. Wait for the content to be visible using documented
   observations, not a guessed timeout. Expand ordinary reading controls if necessary.
2. Prefer a supported browser content export or actual download. Check its reported MIME/path,
   scope and chapter before processing. A native export may be unsupported in the in-app browser;
   one capability failure is enough to choose another documented route.
3. When export is unavailable, use documented read-only DOM evaluation to capture the visible
   chapter fragment and metadata in runtime memory. Capture only content needed for this task;
   exclude account controls, cookies and hidden application state. Do not dump full chapters or
   base64 blobs into the chat/tool transcript just to transport them.
4. Use a supported file bridge when offered by the runtime. If only browser forms are available,
   a task-local receiver bound to `127.0.0.1` can accept the captured data through a visible form:
   cap body size, use a random per-run token, accept one expected payload, validate chapter IDs,
   write only within a dedicated temporary directory, and treat submitted HTML as text. Never
   serve the submitted HTML as executable content. Stop the receiver afterwards. Do not expose
   the listener to the network or leave a permanent service running.
5. After transfer, compare the capture's byte length/checksum and expected chapter identity. If
   no supported transfer is possible, retain metadata and mark capture/export blocked; do not
   claim a download occurred or substitute uncaptured network bytes.

## Extract locally and verify

Parse captured HTML as untrusted data. Remove scripts, form controls, event handlers, remote
asset loads, observed ad containers and hidden SEO nodes while retaining content and paragraph
boundaries. Do not delete a whole article solely because an initial animation/loading style is
hidden; verify the final visible browser state before capture. Keep a checksum and transformation
log so an extracted TXT is distinguishable from its captured HTML.

Call `trafilatura.extract(captured_html, output_format="txt", include_comments=False)` through
that environment. Check the installed API/help before adding options. If extraction loses a
known paragraph or attribution, compare a targeted DOM text conversion of the captured fragment
and retain the best faithful result with the method recorded. Empty extraction blocks completion.

Write UTF-8 with a documented newline policy. Keep raw source wording, accents, dialogue marks,
translator attribution and paragraph order; normalization needed by the Script IR belongs in the
adaptation step. Titles/source URLs belong in index/provenance metadata or clearly marked headers,
not silently inserted as story prose. Use original synthetic samples to test filters; do not
commit downloaded story text to a fixture corpus.

Compare a chapter's beginning, middle and ending and the requested range to what the browser
actually shows. Record OCR uncertainty and site censorship instead of filling missing words.
Finalize indexes/sync before cleaning temporary files under the shared recovery contract.
