# Native source provenance

The original Microsoft repository returned HTTP404 on Nov18 and Nov19 2026. The exact
v1.28.2 signed commit is retained by ossia/onnxruntime. This is a source mirror, not a
replacement implementation. Git fetch of the exact commit and reconstruction of its signed
Git object both yield 33ca9628233dc8f002435e868d4c2e9f82766ca1; tree
7841f821d01a42a4ae981810242278258898e61f. The raw commit, signed payload and signature
are retained here. `git verify-commit` succeeded locally after importing the GitHub public
key from https://github.com/web-flow.gpg into an isolated verification keyring. Fingerprint:
968479A1AFF927E37D1A566BB5690EEEBB952194. No signing identity was created or replaced.
The trust anchor is GitHub's published public key and the immutable commit, not the mirror
owner's statements. Upstream cached release pages identify v1.28.2 as 33ca962.

Known memory-safety fixes in v1.28.3 require separate native validation. The exact signed
patch release is 0d68ff6b3b72b04aac578decd6c4c45d322bb962 (tree
8a9cff33dc5f4e02297d09157f68096e2ae052d4). Rebuild work keeps sherpa's 1.13.8 API/ABI;
no sherpa2.x substitution is accepted. Source provenance alone is not a security PASS.
