# Docs Site Navigation Integration Design

**Date:** 2026-05-21
**Status:** Approved for implementation

## Motivation

The sasurahime documentation site consists of a bilingual landing page (EN `index.html` / JA `ja/index.html`) and bilingual Markdown documentation pages (HOWTO-USE, SUPPORTED, HOWTO-ADD-target) hosted via GitHub Pages / Jekyll. While both the landing pages and the doc pages support English and Japanese, their language-switching mechanisms differ, creating an inconsistent user experience.

## Current State

| Aspect | Landing pages (`index.html`, `ja/index.html`) | Doc pages (`_layouts/doc.html` + `.md`) |
|--------|-----------------------------------------------|----------------------------------------|
| Language switching | Separate HTML files, `<a>` link-based navigation | Single file, JS `switchLang()` toggles `<details>` blocks |
| Switcher UI | Text links (`EN` span / `日本語` anchor) | Buttons (`[EN]` / `[JA]` with `.lang-btn` class) |
| Nav labels | Static per-file (EN file = English, JA file = Japanese) | Always English regardless of active language |
| Cross-links | Footer links to HOWTO-USE only | Footer links to HOWTO-USE + Top via logo |

### Verified Good Practices (already implemented per prior review)

- hreflang / canonical / og:locale meta tags on both landing pages
- Shared CSS (`docs/assets/style.css`) with no inline duplication
- Japanese section labels fully translated
- CTA text specific and actionable
- All 3 Markdown docs have full bilingual `<details>` blocks
- Dark theme, responsive grid, logical info flow

## Scope

This design covers **navigation integration only**. No content changes, no restructuring of the Markdown files, no new pages.

## Design

### 1. Language-Aware Doc Navigation

**File:** `docs/_layouts/doc.html`

Extend `switchLang()` to update navigation link labels when the user toggles between EN and JA:

```javascript
function switchLang(lang) {
  // Existing details toggle logic (unchanged)
  var details = document.querySelectorAll('details');
  details.forEach(function(d) {
    var summary = d.querySelector('summary');
    if (!summary) return;
    var text = summary.textContent || '';
    d.open = lang === 'en' ? text.indexOf('English') !== -1
                           : text.indexOf('日本語') !== -1;
  });

  // NEW: Update nav link labels
  var links = document.querySelectorAll('.nav-links a');
  // links[0] = "HOW TO USE / 使い方", links[1] = "Targets / ターゲット一覧"
  if (links.length >= 2) {
    if (lang === 'ja') {
      links[0].textContent = '使い方';
      links[1].textContent = 'ターゲット一覧';
    } else {
      links[0].textContent = 'HOW TO USE';
      links[1].textContent = 'Targets';
    }
  }

  // Button active state (unchanged)
  document.getElementById('btn-en').classList.toggle('active', lang === 'en');
  document.getElementById('btn-ja').classList.toggle('active', lang === 'ja');
}
```

The `href` attributes remain unchanged — they always point to `/HOWTO-USE` and `/SUPPORTED` (Jekyll-generated HTML paths), which serve both languages in a single file via `<details>` blocks.

### 2. Unified Language Switcher Visual

**Files:** `docs/index.html`, `docs/ja/index.html`

Replace the text-link language switcher with button-style `.lang-btn` elements matching the doc page style.

**`docs/index.html` (English landing):**

```html
<div class="lang-switch">
  <span class="lang-btn active">EN</span>
  <a class="lang-btn" href="ja/">JA</a>
</div>
```

**`docs/ja/index.html` (Japanese landing):**

```html
<div class="lang-switch">
  <a class="lang-btn" href="../">EN</a>
  <span class="lang-btn active">JA</span>
</div>
```

The `.lang-btn` CSS class (currently defined only in `_layouts/doc.html`'s inline `<style>`) must be moved to `docs/assets/style.css` so the landing pages can use it. The class definition:

```css
.lang-btn {
  background: var(--bg3);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.8rem;
  font-weight: 600;
  padding: 3px 10px;
}
.lang-btn.active {
  background: var(--accent-dim);
  border-color: var(--accent);
  color: #fff;
}
.lang-btn:hover:not(.active) {
  background: var(--border);
  color: var(--text);
}
```

After moving to `style.css`, remove the duplicate definition from `_layouts/doc.html`'s inline `<style>` block.

### 3. Cross-Link Enhancement

#### 3a. Landing page footer: add all doc links

**`docs/index.html` footer:**

```html
<footer>
  <p>
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="HOWTO-USE">How to Use</a> ·
    <a href="SUPPORTED">Supported Targets</a> ·
    <a href="HOWTO-ADD-target">Add a Target</a> ·
    Apache-2.0 License
  </p>
  <p style="margin-top:0.5rem">© 2025 sasurahime contributors</p>
</footer>
```

**`docs/ja/index.html` footer:**

```html
<footer>
  <p>
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="../HOWTO-USE">使い方</a> ·
    <a href="../SUPPORTED">対応ターゲット一覧</a> ·
    <a href="../HOWTO-ADD-target">ターゲット追加方法</a> ·
    Apache-2.0 License
  </p>
  <p style="margin-top:0.5rem">© 2025 sasurahime contributors</p>
</footer>
```

#### 3b. Doc page footer: add Top link

**`docs/_layouts/doc.html` footer:**

```html
<footer>
  <p>
    <a href="{{ '/' | relative_url }}">Top</a> ·
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="{{ '/HOWTO-USE' | relative_url }}">Documentation</a> ·
    Apache-2.0 License
  </p>
  <p style="margin-top:0.5rem">© 2025 sasurahime contributors</p>
</footer>
```

#### 3c. Doc page content: cross-references in HOWTO-USE

Append to the English section and Japanese section of `docs/HOWTO-USE.md`:

**End of English `<details>` block (before `</details>`):**

```markdown
---

📖 See also: [Supported Targets](/SUPPORTED) · [How to Add a Target](/HOWTO-ADD-target)
```

**End of Japanese `<details>` block (before `</details>`):**

```markdown
---

📖 関連ドキュメント: [対応ターゲット一覧](/SUPPORTED) · [ターゲット追加方法](/HOWTO-ADD-target)
```

## Files Changed

| File | Change |
|------|--------|
| `docs/_layouts/doc.html` | Extend `switchLang()` JS, remove duplicate `.lang-btn` CSS, add Top link to footer |
| `docs/index.html` | Replace lang switcher with `.lang-btn` elements, expand footer links |
| `docs/ja/index.html` | Replace lang switcher with `.lang-btn` elements, expand footer links |
| `docs/assets/style.css` | Add `.lang-btn` CSS class definitions |
| `docs/HOWTO-USE.md` | Add cross-reference links at end of both EN and JA sections |

## Non-Changes (explicitly out of scope)

- No bilingual merge of the landing pages (they remain separate HTML files)
- No content edits to SUPPORTED.md or HOWTO-ADD-target.md (they already have cross-links in nav)
- No restructuring of Markdown frontmatter
- No new pages or sections
- No CSS redesign beyond the `.lang-btn` relocation
