# Issue: Text Selection in GPUI Chat Messages

## Goal
Implement text selection (mark text with mouse, copy with Cmd+C) in chat message bubbles, similar to how Zed's editor handles text selection.

## Current State
- `SelectableText` component exists at `src/gui/components/selectable_text.rs`
- Selection state is tracked (start/end range, drag operations)
- Copy to clipboard works (Cmd+C)
- Double-click word selection works
- Triple-click select-all works
- **Visual selection highlighting does NOT work properly**

## Approaches Tried

### Approach 1: Custom Element with Text Shaping
**File:** `src/gui/components/selectable_text.rs`

Created a custom GPUI `Element` that:
1. Shapes text using `window.text_system().shape_text()`
2. Paints selection rectangles using `window.paint_quad()`
3. Paints text lines using `line.paint()`

**Problem:** Text renders vertically (one character per line). The custom element's `request_layout` doesn't properly communicate size to GPUI's layout system. Setting `style.size.width = gpui::relative(1.)` doesn't work as expected when parent has no definite width.

**Screenshot behavior:** Each character appeared on its own line, suggesting the text shaping width constraint wasn't being respected.

### Approach 2: Split Text into Before/Selected/After Divs
Render selection by splitting content into three parts:
```rust
div()
    .child(before_text)
    .child(div().bg(selection_color).child(selected_text))
    .child(after_text)
```

**Problem:**
1. Causes layout re-renders on every selection change
2. Text doesn't flow inline - each div is a block element
3. Adding `.flex().flex_wrap()` doesn't help because flex items don't break mid-word

### Approach 3: No Visual Highlight (Silent Selection)
Track selection in memory, don't render visual highlight, just use for copy.

**Problem:** User (rightfully) wants visual feedback when selecting text.

### Approach 4: Current State (Custom Element v2)
Back to custom Element approach with:
- Parent div handles mouse events and focus
- Child `SelectableTextElement` handles text rendering
- Selection rectangles painted before text

**Current Problem:** Still not rendering correctly. Need to investigate why.

## Key Files

- `src/gui/components/selectable_text.rs` - Main component
- `src/gui/components/mod.rs` - Exports `SelectableText`, `SelectableCopy`, `SelectableSelectAll`
- `src/gui/app.rs` - Integration:
  - `message_texts: HashMap<String, Entity<SelectableText>>` stores entities per message
  - `create_message_text()` / `update_message_text()` manage lifecycle
  - `render_messages()` renders entities inside message bubbles

## Things to Investigate

### 1. GPUI Layout System
- How does GPUI determine element size when using custom `Element` trait?
- Does `request_layout` need to return actual content size, not just style hints?
- Look at how Zed's `EditorElement` handles this in their codebase

### 2. Text Shaping Width
- `shape_text()` takes `Some(bounds.size.width)` for wrapping
- But `bounds` in `prepaint` might have incorrect width if layout wasn't computed correctly
- Add debug logging to see actual bounds values

### 3. Reference Implementations
Look at Zed's codebase for how they handle:
- `crates/editor/src/element.rs` - EditorElement
- `crates/gpui/src/elements/text.rs` - Built-in text element
- `crates/gpui/src/styled.rs` - How styled text works

### 4. Alternative: Use GPUI's Built-in Text with Styled Runs
GPUI's `TextRun` has a `background_color` field. Could potentially:
1. Create multiple `TextRun`s with different background colors for selection
2. Let GPUI's text system handle the rendering

```rust
let runs = vec![
    TextRun { len: before_len, background_color: None, ... },
    TextRun { len: selected_len, background_color: Some(blue), ... },
    TextRun { len: after_len, background_color: None, ... },
];
window.text_system().shape_text(content, font_size, &runs, wrap_width, None)
```

### 5. Debug Steps
Add logging to understand what's happening:
```rust
fn prepaint(...) {
    eprintln!("bounds: {:?}", bounds);
    eprintln!("bounds.size.width: {:?}", bounds.size.width);
    // ... shape text ...
    eprintln!("lines count: {}", lines.len());
    for (i, line) in lines.iter().enumerate() {
        eprintln!("line {}: len={}", i, line.len());
    }
}
```

## Minimal Reproduction
1. Run `cargo run --bin stockpot-gui --features gui`
2. Send a message to get a response
3. Try to click and drag to select text in a message bubble
4. Observe: text renders incorrectly OR selection highlight doesn't appear

## Success Criteria
- Text renders normally (horizontal, wrapping at container width)
- Click and drag shows blue highlight over selected characters
- Cmd+C copies highlighted text
- No layout jumping/re-rendering artifacts during selection
- Works for both user and assistant messages
