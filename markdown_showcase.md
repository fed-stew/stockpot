# Markdown Showcase 🍲

A comprehensive collection of markdown elements to test your scrolling streaming renderer.

---

## Headings

# H1: The Main Course
## H2: Side Dish
### H3: Garnish
#### H4: Seasoning
##### H5: Spice Blend
###### H6: The Finest Details

---

## Text Formatting

This is **bold text** for emphasis. This is *italic text* for style. This is ***bold and italic*** for maximum impact.

~~Strikethrough text~~ shows deleted content.

`Inline code` looks like this in the middle of a sentence.

---

## Lists

### Unordered Lists

- Item one
- Item two
  - Nested item 2.1
  - Nested item 2.2
    - Deep nested 2.2.1
    - Deep nested 2.2.2
- Item three

### Ordered Lists

1. First step
2. Second step
   1. Sub-step 2.1
   2. Sub-step 2.2
3. Third step
   1. Another sub-step
   2. Final sub-step

### Mixed Lists

1. Start with ordered
   - Switch to unordered
   - Another bullet
2. Back to ordered
   - Nested bullet again

### Task Lists

- [x] Completed task
- [ ] Incomplete task
- [x] Another completed task
- [ ] Still working on this

---

## Blockquotes

> This is a simple blockquote.

> This is a blockquote with multiple lines.
> It can span across several lines
> and maintains the formatting nicely.

> Nested blockquotes work too!
>> Like this one
>>> And even deeper!

---

## Code Blocks

### JavaScript
```javascript
function greet(name) {
  console.log(`Hello, ${name}!`);
  return true;
}

const result = greet("World");
```

### Python
```python
def fibonacci(n):
    """Generate Fibonacci sequence up to n terms."""
    a, b = 0, 1
    for _ in range(n):
        yield a
        a, b = b, a + b

for num in fibonacci(10):
    print(num)
```

### Rust
```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5];
    let sum: i32 = numbers.iter().sum();
    println!("Sum: {}", sum);
}
```

### Bash
```bash
#!/bin/bash
echo "Starting the server..."
npm start
```

### JSON
```json
{
  "name": "Stockpot",
  "version": "1.0.0",
  "description": "A Rust-powered code agent",
  "features": ["fast", "efficient", "pleasant"]
}
```

### Plain Text
```
This is plain text without syntax highlighting.
It can contain anything you want.
    Indentation is preserved.
```

---

## Links and Images

[Visit OpenAI](https://openai.com)

[Link with title](https://github.com "GitHub")

[Reference-style link][ref1]

[ref1]: https://example.com

### Image Examples

![Alt text for image](https://via.placeholder.com/200)

[![Clickable image](https://via.placeholder.com/150)](https://example.com)

---

## Horizontal Rules

---

***

___

---

## Tables

| Feature | Support | Status |
|---------|---------|--------|
| Headings | ✅ | Working |
| Lists | ✅ | Working |
| Code | ✅ | Working |
| Tables | ✅ | Working |
| Streaming | ✅ | Excellent |

### Complex Table

| Left | Center | Right |
|:-----|:------:|------:|
| A | B | C |
| 1 | 2 | 3 |
| Long content here | Middle | End |

---

## HTML Snippets

<div style="background-color: #f0f0f0; padding: 10px; border-radius: 5px;">
  This is embedded HTML that renders inline.
</div>

<details>
  <summary>Click to expand</summary>
  Hidden content that appears when expanded!
</details>

---

## Inline Elements

This paragraph contains **bold**, *italic*, `code`, ~~strikethrough~~, and [links](https://example.com) all mixed together.

---

## Definition Lists

Term 1
: Definition 1

Term 2
: Definition 2a
: Definition 2b

---

## Footnotes

Here's a sentence with a footnote[^1].

And another one[^2].

[^1]: This is the first footnote explanation.
[^2]: This is the second footnote with more details.

---

## Emoji Support

- 🍲 Stockpot
- 🚀 Rocket
- ✨ Sparkles
- 🎯 Target
- 📝 Documentation
- 🔧 Tools
- 💻 Code
- 🌟 Star

---

## Special Characters & Entities

Copyright: &copy;
Registered: &reg;
Trademark: &trade;
Euro: &euro;
Bullet: &bull;

---

## Line Breaks and Spacing

This has a  
manual line break.

This paragraph is separated by a blank line above.

---

## Escape Characters

\*Not italic\*
\[Not a link\]
\`Not code\`

---

## Extended Syntax Examples

### Subscript and Superscript

H~2~O is water.
E=mc^2^ is Einstein's equation.

### Highlight

This text has ==highlighting== applied.

### Abbreviations

The HTML specification is maintained by the W3C.

*[HTML]: Hyper Text Markup Language
*[W3C]: World Wide Web Consortium

---

## Mixed Content Section

Let me combine several elements:

> **Important Note:** Always test your markdown renderer thoroughly!

Here's why:

1. **Performance** - Streaming should be smooth
2. **Accuracy** - All elements must render correctly
3. **Accessibility** - Proper semantic HTML

```javascript
// Example from the note above
const testMarkdownRenderer = async () => {
  const elements = ['headings', 'lists', 'code', 'tables'];
  for (const element of elements) {
    console.log(`Testing ${element}...`);
  }
};
```

---

## Final Thoughts

Testing a markdown renderer requires checking:

- ✅ All heading levels
- ✅ Text formatting (bold, italic, strikethrough)
- ✅ Lists (ordered, unordered, nested)
- ✅ Code blocks with syntax highlighting
- ✅ Blockquotes and nesting
- ✅ Links and images
- ✅ Tables with alignment
- ✅ Horizontal rules
- ✅ Inline HTML
- ✅ Special characters

**Happy rendering!** 🍲
