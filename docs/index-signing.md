# Signing the published index

What ORNG Catalog has to change so that serving the index stops being the same
thing as authoring it. The code is in this repository; the workflow is not, so
this is the note whoever writes it works from.

## What the signature is for

`index.json` is the trust anchor. Every row carries a sha-256 digest and a
downloaded document is checked against it, which means whoever serves the index
decides what gets installed into a DAW. Today that is GitHub Releases. A mirror
at `orng.tools` would be a second party able to do it, and the mirror exists to
save bandwidth, not to be trusted.

Signing removes both from the trust path. The key lives in one repository secret
that only the publish workflow can read, the application carries the public half,
and anything that rewrites a digest in transit produces a file that verifies
against nothing. A mirror can then be stale, or missing, but never wrong.

The signature is **detached**: a second release asset, `index.json.sig`, beside
the index rather than inside it. An embedded signature would have to be excluded
from what it covers, so what is signed becomes a canonicalisation of the file
rather than the file, and a canonicalisation is a second serializer for two
programs to agree about. Detached keeps the bytes signed, the bytes served and
the bytes parsed identical, and leaves `index.json` unchanged for anything
reading it that has not learned about signatures.

**The application keeps that pair and re-proves it.** A verified index is
written to `~/.orng/catalog/` as the two files exactly as they arrived, and the
next launch reads them back through the same `Index::verified` call the download
goes through. So the cache is not a third party either: a home directory is
writable by anything running as the user, and an index that decides what gets
downloaded into a DAW must not become believable by having been stored locally.
Keeping the parsed index instead would have made that impossible to check, which
is the same argument the detached signature is making one level up.

## Making the key, once

On a machine the maintainer trusts, never in continuous integration:

```
orng-catalog-lint keygen
```

It prints two hex strings, the secret half and the public half. Nothing else
ever needs to be run again; the key pair is for the life of the catalog unless
it is compromised.

## The secret half

Store it as a **repository secret** on `orng-catalog`, under
Settings -> Secrets and variables -> Actions, named:

```
ORNG_CATALOG_SIGNING_KEY
```

Four things about where it is put, each of which is the difference between a
secret and a published file:

- A repository secret, not an organisation secret. An organisation secret is
  readable by every repository it is shared with, and the catalog is the only
  one that publishes an index.
- Reachable only from the publish workflow, and only on the default branch.
  GitHub will hand a secret to any workflow in the repository that asks for it,
  so "only the publish workflow" is a property of what the workflow files say,
  not something the platform enforces. Nothing else should name it.
- Never from anything that runs a contributor's code. Every contribution is a
  fork pull request, and a `pull_request` workflow on a fork gets no secrets.
  That is the property the whole governance model already leans on. Do not
  reach for `pull_request_target` to work around a permission problem; it runs
  the base branch's workflow with the fork's content and with secrets.
- Into the environment of the one step that signs, and nowhere else. The tool
  reads it from `ORNG_CATALOG_SIGNING_KEY` and has no flag that could name a
  file instead, because a path on a command line is recorded in shell history
  and echoed into a workflow log.

Store the public half as a repository **variable**, `ORNG_CATALOG_PUBLIC_KEY`.
It is not secret; it is there so the workflow can state which key it expects and
notice if it ever signs with another one.

## The publish workflow

The index is generated on merge and published as a release asset. Signing is one
step after that, and a check is one step after signing:

```yaml
      - name: Generate the index
        run: orng-catalog-lint index --root . --out index.json --from-git

      - name: Sign it
        env:
          ORNG_CATALOG_SIGNING_KEY: ${{ secrets.ORNG_CATALOG_SIGNING_KEY }}
        run: orng-catalog-lint sign --index index.json --out index.json.sig

      - name: Check what is about to be published
        run: |
          orng-catalog-lint verify \
            --index index.json \
            --signature index.json.sig \
            --public-key ${{ vars.ORNG_CATALOG_PUBLIC_KEY }}

      - name: Publish both
        run: gh release upload "$TAG" index.json index.json.sig --clobber
```

Three things this order is protecting:

- **Sign the file that is published, not a file like it.** `sign` reads the
  index off disk rather than regenerating it, because the published index
  carries the revisions git gave it at that moment. Nothing may rewrite
  `index.json` between the signing step and the upload: not a formatter, not a
  `jq` pass, not a commit hook. A single added newline is a file that no longer
  verifies.
- **Verify before uploading.** The check runs the same call the application
  makes, against the public key the repository says it expects, so a secret that
  was rotated in one place and not the other fails the release rather than every
  user. It exits 1 when the pair does not verify and 2 when it could not run.
- **Upload both together.** A release carrying an index and last week's
  signature is a release nobody can install from.

The two assets are then what the mirror mirrors, byte for byte.

## How the public key reaches the application

Compiled in. ORNG Registry holds the public key as a constant and parses it with
`PublicKey::from_hex` at startup:

```rust
const CATALOG_KEY: &str = "<the public half, 64 hex digits>";
```

Not fetched. A key downloaded beside the index would be chosen by whoever serves
the index, which is the party the signature exists to distrust, and the whole
scheme would reduce to trusting the domain again. The same reasoning rules out
reading it from a config file a user could be talked into editing.

It is published in the catalog's README and in the release notes as well, so the
value compiled into a build can be compared against the one the project states
in public. That is the only cross-check available to somebody who did not build
the application themselves.

## Rotation

A pinned key means rotation needs an application release, and there is no
automation for it. If it is ever needed: publish a build that accepts the new
key, have the workflow publish a second asset signed with it while both builds
are in the field, and drop the old one when the old builds are gone. A
compromised key is not that case. That one is a revocation, which this scheme
does not have, and the answer is a new key and a release that only accepts it.
