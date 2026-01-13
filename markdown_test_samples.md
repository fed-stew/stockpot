# Markdown Renderer Test Samples 🍲

## Basic Text Formatting

### Bold Text
This is **bold text** and this is ***bold italic***.

### Italic Text
This is *italic text* and this is _also italic_.

### Code Inline
Use `let x = 42;` for inline code snippets.

### Mixed Formatting
This is **bold with `code inside`** and *italic with `code`* too.

---

## Headers (All Levels)

# Header Level 1
## Header Level 2
### Header Level 3
#### Header Level 4
##### Header Level 5
###### Header Level 6

---

## Complex Examples

### Example 1: API Documentation
The `fetch()` function is **essential** for *asynchronous* HTTP requests. Use ***careful error handling*** when working with `try/catch` blocks.

### Example 2: Code Comments
- Use `const` for **immutable** variables
- Use `let` for *scoped* variables
- Avoid `var` - it's ***deprecated*** in modern code

### Example 3: Nested Formatting
This paragraph demonstrates **bold with _nested italic_** and *italic with __nested bold__* formatting.

---

## Edge Cases

### Unmatched Delimiters
This has a single * asterisk and a single _ underscore.

### Consecutive Delimiters
**bold1** **bold2** and *italic1* *italic2*

### Empty Delimiters
** ** and __ __ (these should be plain text)

### Special Characters in Code
Code with special chars: `const x = {a: 1, b: 2};`

---

## Real-World Scenarios

### Function Documentation
The `render_markdown()` function accepts **source text** and returns a ***formatted output***. It handles:
- `Bold` text with `**markers**`
- `Italic` text with `*markers*` or `_markers_`
- `Code` with backticks
- Headers with `#` symbols

### Configuration Example
Set `debug: true` for **verbose logging** and `timeout: 5000` for *extended* operations.

---

## Stress Test

Multiple bold: **one** **two** **three**
Multiple italic: *one* *two* *three*
Mixed: **bold** *italic* `code` **bold**

Nested attempts: **bold *italic* bold** and *italic **bold** italic*

---

## Whitespace Handling

Line with trailing spaces    
Next line should render normally

Multiple    spaces    in    middle should collapse

---

## The End

✅ **Rendering complete!** All markdown features have been tested.
