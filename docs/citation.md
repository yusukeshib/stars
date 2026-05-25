# Citation and release archiving

Use this page together with [`CITATION.cff`](../CITATION.cff) when citing
`stars` outputs, screenshots, or derived software.

## Preferred citation text

> Shibata, Y. *stars: a physically informed cross-platform sky renderer*.
> Version or commit used. GitHub repository:
> <https://github.com/yusukeshib/stars>. Archived release DOI: cite the
> version-specific Zenodo DOI when the release is archived.

For a figure or reproducibility package, also attach the schema-versioned JSON
session, the catalog/data manifest reference when available, and any generated
validation-gallery preset name.

## Zenodo release DOI workflow

The repository includes [`.zenodo.json`](../.zenodo.json) so Zenodo can mint a
DOI for tagged GitHub releases with stable metadata.

Release checklist:

1. Tag the release in GitHub.
2. Let the GitHub-Zenodo integration archive the tag.
3. Copy the minted **version DOI** into the GitHub release notes.
4. If a later release changes citation metadata materially, update
   [`CITATION.cff`](../CITATION.cff), this page, and `.zenodo.json` in the same
   PR.

Do not invent a DOI before Zenodo has minted it. Until the first archive exists,
cite the repository URL plus the exact commit hash.

## Data and source caveats

A citation to `stars` is not, by itself, enough to reproduce a scientific
figure. Record these alongside the citation:

- **Code identity:** release DOI if available, otherwise repository URL and
  commit hash.
- **Scene identity:** JSON session or built-in preset name, including observer,
  UTC/UT1/TT/TDB fields, projection/viewpoint, atmosphere, overlays, and
  correction flags.
- **Catalog identity:** HYG v4.2 today; future large-catalog backends must cite
  the manifest entry named by the session.
- **Model limits:** current solar-system ephemerides are visual VSOP87/ELP-style
  approximations, not final DE440 publication-grade states; see
  [`docs/standards-compliance.md`](standards-compliance.md) and
  [`VALIDATION.md`](../VALIDATION.md).
- **Rendering limits:** screenshots depend on renderer settings, GPU/driver
  behaviour, display characteristics, and the documented tone-reproduction
  assumptions.

When in doubt, cite the archived release, include the session JSON, and mention
which validation preset or command regenerated the image.
