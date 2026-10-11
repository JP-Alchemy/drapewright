# Working with an AI assistant

Drapewright includes an [MCP](https://modelcontextprotocol.io) server,
`drapewright-mcp`. An AI assistant such as Claude can use it to open
patterns, look at the pieces, name them, set allowances and cutting, add
notches and grainlines, draw simple pieces, lay them out, and save a
project that you then open in the Drapewright app.

It runs on your computer. It reads and writes only the files the
assistant names, makes no network connections, and changes a project only
through the same commands as the app. Each change is one undo step.

## Setting it up

Build it once (from the repository):

```bash
cargo build --release -p drapewright-mcp
```

The program is then `target/release/drapewright-mcp`. Tell your assistant
where it is:

* **Claude Code**: run this from the repository folder:

  ```bash
  claude mcp add drapewright -- "$PWD/target/release/drapewright-mcp"
  ```

* **Claude Desktop**: Settings → Developer → Edit Config, and add it to
  `claude_desktop_config.json`, then restart Claude:

  ```json
  {
    "mcpServers": {
      "drapewright": { "command": "/full/path/to/target/release/drapewright-mcp" }
    }
  }
  ```

* **Other assistants**: any MCP client can start it as a stdio server
  with no arguments.

## What to ask

For example:

> Import ~/Downloads/sasha-dress.svg, show me the pieces, and tell me what
> each one is. Then call the centre front edge "centre front", set 1 cm
> seam allowance with a 3 cm hem on the skirt pieces, and save it to
> ~/Desktop/sasha.drapewright.

> Open ~/Desktop/sasha.drapewright, add a 70 × 8 cm waistband cut on the
> fold, and lay everything out for 150 cm fabric.

To work on the project in Drapewright, open the saved `.drapewright` file
with **Projects → Restore a backup…**, or **Open pattern**. To give an
assistant a project from the app, save it with **Back up** and give the
assistant that file.

## How it behaves

* **It asks rather than guesses.** When a file holds every size, doesn't
  say how big it is, or may or may not include seam allowance, the import
  stops until the assistant passes your answer. A good assistant asks
  you; the import report tells it exactly what to ask.
* **It shows its work.** `render_preview` returns a picture of the
  pieces, numbered, with folds in blue, hems in orange, the stitching line
  dashed, notches and grainlines, and optionally each edge's id. The
  assistant uses it to see shapes and to check its changes.
* **It never overwrites a file unless asked**, and never drops unsaved
  changes without saying so.
* **Millimetres throughout.** Points are `[x, y]` with y down, in each
  piece's own frame.

## The tools

| Tool | What it does |
|---|---|
| `project_summary` | The open project and each piece's id, size, allowance, cutting and problems |
| `new_project`, `open_project`, `save_project` | Start, open and save `.drapewright` projects |
| `inspect_pattern_file` | What an SVG, DXF or PDF holds, and the questions importing it needs answered |
| `import_pattern` | Adds a file's pieces, with the answers to those questions |
| `get_piece` | One piece in full: edges, notches, grainline, marks, lines, position |
| `render_preview` | A picture of the pieces (PNG) with a legend |
| `update_piece` | Name, seam allowance, cutting instructions |
| `update_edge` | An edge's kind (seam, hem, fold), name, allowance and corner shape |
| `split_edge`, `join_edges` | Add or remove a corner on the outline |
| `set_grainline` | Along the length or width, between two points, or removed |
| `add_notch`, `delete_notch` | Single, double or triple notches |
| `create_piece` | A rectangle, or any closed outline as SVG path data in mm |
| `delete_pieces` | Removes pieces (undo brings them back) |
| `move_pieces`, `rotate_piece`, `arrange_pieces` | Arrange the work table, e.g. in rows as wide as the fabric |
| `undo`, `redo` | One tool call is one step |
| `print_settings` | How the project prints, and its print settings |
| `export_pdf`, `export_svg` | The tiled true-size PDF, or a true-size SVG |

## Not yet

* It works on project files, not on the project open in the app. A live
  link, so the app shows the assistant's changes as they happen, comes
  next.
