# Markdown Rendering Test

This is a test document to verify markdown rendering capabilities.

## Headers

### Heading 3
#### Heading 4
##### Heading 5

---

## Text Formatting

**Bold text** and *italic text* and ***bold italic***

~~Strikethrough text~~

`inline code`

## Lists

### Unordered List
- Item 1
- Item 2
  - Nested item 2a
  - Nested item 2b
- Item 3

### Ordered List
1. First item
2. Second item
3. Third item

### Task List
- [x] Completed task
- [ ] Incomplete task
- [ ] Another task

## Links and Images

[Link to example.com](https://example.com)

## Blockquotes

> This is a blockquote.
> 
> It can span multiple lines.

> Nested blockquote
>> Double nested

## Code Blocks

```javascript
function greet(name) {
    console.log(`Hello, ${name}!`);
    return true;
}
```

```python
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)
```

## Tables

| Header 1 | Header 2 | Header 3 |
|----------|----------|----------|
| Cell 1   | Cell 2   | Cell 3   |
| Cell 4   | Cell 5   | Cell 6   |

| Left Aligned | Center Aligned | Right Aligned |
|:-------------|:--------------:|--------------:|
| Text         | Text           | Text          |
| More text    | More text      | More text     |

## Horizontal Rules

---

***

___

## HTML Elements

<details>
<summary>Click to expand</summary>
This is hidden content inside a details element.
</details>

## Emojis

🎉 🚀 🔥 💻 🍲

## Math (if supported)

Inline math: $E = mc^2$

Block math:

$$
\sum_{i=1}^{n} i = \frac{n(n+1)}{2}
$$

## Footnotes

Here's a sentence with a footnote[^1].

[^1]: This is the footnote content.

## Definition Lists (if supported)

Term 1
: Definition 1

Term 2
: Definition 2a
: Definition 2b

---

**End of test document**