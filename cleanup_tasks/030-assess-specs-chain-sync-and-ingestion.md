# Review Task 030: Chain sync and ingestion specs

## Scope

- `docs/specs/chain_sync_entrypoint.md`
- `docs/specs/ingestion.md`
- `docs/specs/cryo_library_adapter.md`

## Goal

Critically assess whether the ingestion story is cohesive and whether each spec owns a distinct surface without duplication.

## Assessment checklist

- Ownership: which spec defines semantics vs configuration vs implementation notes?
- Duplication: are flow descriptions repeated across these docs and in architecture docs?
- Contracts: do these specs clearly link to the payload contracts they depend on?
- Range semantics: is range terminology consistent and centralized?

## Output

- A duplication map and recommended ownership statements per doc.
- Proposed restructures that reduce headings and repeated narratives.
- A list of any missing specs needed for clarity (only if truly missing).

