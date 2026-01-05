---
name: image
description: Add an image to a decision document — rename, move to docs/assets/, insert reference
---
# Add Image to Document

Add an image to a decision document. Handles renaming, moving to docs/assets/, and inserting the markdown reference.

## When to Use

- User pastes or provides an image to include in a document
- User asks to add a screenshot, diagram, or photo to a doc
- User provides an image file path that needs to be moved to the correct location

## Workflow

1. **Identify the image source**: The user may paste an image (saved to a temp path), provide a file path, or provide a URL.

2. **Determine the target document**: Ask which document (e.g. ADR-001, OPP-002) the image belongs to, if not obvious from context.

3. **Name the image file**: Use a descriptive, kebab-case name based on content:
   - `architecture-overview.png`
   - `incident-timeline.png`
   - `deployment-flow.svg`
   - Format: `{doc-id-lowercase}-{description}.{ext}` e.g. `adr-001-architecture-overview.png`

4. **Ensure docs/assets/ exists**:
   ```bash
   mkdir -p docs/assets
   ```

5. **Move/copy the image**:
   ```bash
   cp /path/to/source/image.png docs/assets/adr-001-architecture-overview.png
   ```

6. **Insert markdown reference** in the target document using `dg set`:
   ```bash
   dg set ADR-001 --section "Context and Problem Statement" --append '![Architecture overview](../assets/adr-001-architecture-overview.png)'
   ```
   Or for documents that reference from project root:
   ```markdown
   ![Architecture overview](docs/assets/adr-001-architecture-overview.png)
   ```

7. **Validate**:
   ```bash
   dg validate
   ```

## Image Path Rules

- All local images MUST be stored in `docs/assets/`
- Reference from within docs/ subdirs: `../assets/filename.png`
- Reference from project root: `docs/assets/filename.png`
- External URLs (https://...) are allowed anywhere
- `dg validate` will flag images stored outside docs/assets/ (error C002)

## Tips

- Prefer SVG for diagrams (scalable, small file size)
- Prefer PNG for screenshots (lossless, good for text)
- Keep file sizes reasonable — compress large screenshots
- Use descriptive alt text for accessibility
- Prefix image filenames with the doc ID to avoid naming collisions
