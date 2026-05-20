# Docs Site Navigation Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify language switcher visual across landing pages and doc pages, add language-aware nav labels to doc pages, and improve cross-links between all pages.

**Architecture:** 5 files modified — shared CSS (`docs/assets/style.css`) gets `.lang-btn` class extracted from inline `<style>` in `_layouts/doc.html`; JS in `_layouts/doc.html` gets `switchLang()` extended to update nav link labels; both landing pages (`index.html`, `ja/index.html`) get button-style language switcher and expanded footer links; `HOWTO-USE.md` gets cross-reference links at end of both language sections.

**Tech Stack:** HTML, CSS, JavaScript (vanilla), Jekyll/Liquid templates, Markdown

---

### Task 1: Add `.lang-btn` CSS to shared stylesheet

**Files:**
- Modify: `docs/assets/style.css` (append before `.divider` rule at end)

- [ ] **Step 1: Insert `.lang-btn` CSS into `style.css`**

Append the `.lang-btn` class definitions before the `.divider` rule at line 189:

Old:
```css
/* DIVIDER */
.divider { border: none; border-top: 1px solid var(--border); }
```

New:
```css
/* LANGUAGE SWITCHER BUTTON */
.lang-btn {
  display: inline-block;
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

/* DIVIDER */
.divider { border: none; border-top: 1px solid var(--border); }
```

- [ ] **Step 2: Commit**

```bash
git add docs/assets/style.css
git commit -m "docs: add .lang-btn CSS to shared stylesheet"
```

---

### Task 2: Update `_layouts/doc.html` — JS, CSS cleanup, footer

**Files:**
- Modify: `docs/_layouts/doc.html`

Three changes in one file:

- [ ] **Step 1: Remove duplicate `.lang-btn` inline CSS**

Delete lines 109-124 from `_layouts/doc.html` (the entire `.lang-btn` / `.lang-btn.active` / `.lang-btn:hover:not(.active)` block inside `<style>`). These definitions now live in `docs/assets/style.css`.

Old (lines 108-134):
```html
    /* lang switcher buttons */
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
    .lang-btn:hover:not(.active) { background: var(--border); color: var(--text); }
    /* content area inside <details> */
```

New (delete the `.lang-btn` block, keep the comment before and after):
```html
    /* content area inside <details> */
```

- [ ] **Step 2: Extend `switchLang()` with nav label switching**

Replace the existing `switchLang()` function (lines 180-198) with the extended version:

Old:
```javascript
  /* EN/JA language switcher */
  function switchLang(lang) {
    var details = document.querySelectorAll('details');
    details.forEach(function(d) {
      var summary = d.querySelector('summary');
      if (!summary) return;
      var text = summary.textContent || '';
      if (lang === 'en') {
        d.open = text.indexOf('English') !== -1;
      } else {
        d.open = text.indexOf('日本語') !== -1;
      }
    });
    document.getElementById('btn-en').classList.toggle('active', lang === 'en');
    document.getElementById('btn-ja').classList.toggle('active', lang === 'ja');
  }
```

New:
```javascript
  /* EN/JA language switcher */
  function switchLang(lang) {
    var details = document.querySelectorAll('details');
    details.forEach(function(d) {
      var summary = d.querySelector('summary');
      if (!summary) return;
      var text = summary.textContent || '';
      if (lang === 'en') {
        d.open = text.indexOf('English') !== -1;
      } else {
        d.open = text.indexOf('日本語') !== -1;
      }
    });
    /* Update nav link labels */
    var navLinks = document.querySelectorAll('.nav-links a:not(.nav-gh)');
    if (navLinks.length >= 2) {
      if (lang === 'ja') {
        navLinks[0].textContent = '使い方';
        navLinks[1].textContent = 'ターゲット一覧';
      } else {
        navLinks[0].textContent = 'HOW TO USE';
        navLinks[1].textContent = 'Targets';
      }
    }
    document.getElementById('btn-en').classList.toggle('active', lang === 'en');
    document.getElementById('btn-ja').classList.toggle('active', lang === 'ja');
  }
```

Note: `.nav-links a:not(.nav-gh)` selects the HOWTO-USE and SUPPORTED links while excluding the GitHub button link. In the current DOM, `.nav-links` contains:
1. `<a href="/HOWTO-USE">HOW TO USE</a>` → `navLinks[0]`
2. `<a href="/SUPPORTED">Targets</a>` → `navLinks[1]`
3. `<div class="lang-switch">...</div>` (not an `a`)
4. `<a class="nav-gh" href="...">GitHub</a>` (excluded by `:not(.nav-gh)`)

- [ ] **Step 3: Add "Top" link to footer**

In the footer `<p>` block (around line 161), insert a Top link before the GitHub link:

Old:
```html
<footer>
  <p>
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="{{ '/HOWTO-USE' | relative_url }}">Documentation</a> ·
    Apache-2.0 License
  </p>
```

New:
```html
<footer>
  <p>
    <a href="{{ '/' | relative_url }}">Top</a> ·
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="{{ '/HOWTO-USE' | relative_url }}">Documentation</a> ·
    Apache-2.0 License
  </p>
```

- [ ] **Step 4: Commit**

```bash
git add docs/_layouts/doc.html
git commit -m "docs: add language-aware nav to doc pages, move .lang-btn css, add Top link"
```

---

### Task 3: Update `docs/index.html` — language switcher + footer

**Files:**
- Modify: `docs/index.html`

Two changes:

- [ ] **Step 1: Replace language switcher markup**

Old (lines 25-28):
```html
    <div class="lang-switch">
      <span class="active">EN</span>
      <a href="ja/">日本語</a>
    </div>
```

New:
```html
    <div class="lang-switch">
      <span class="lang-btn active">EN</span>
      <a class="lang-btn" href="ja/">JA</a>
    </div>
```

- [ ] **Step 2: Expand footer links**

Old (lines 207-213):
```html
  <p>
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="HOWTO-USE">Documentation</a> ·
    Apache-2.0 License
  </p>
  <p style="margin-top:0.5rem">© 2025 sasurahime contributors</p>
```

New:
```html
  <p>
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="HOWTO-USE">How to Use</a> ·
    <a href="SUPPORTED">Supported Targets</a> ·
    <a href="HOWTO-ADD-target">Add a Target</a> ·
    Apache-2.0 License
  </p>
  <p style="margin-top:0.5rem">© 2025 sasurahime contributors</p>
```

- [ ] **Step 3: Commit**

```bash
git add docs/index.html
git commit -m "docs: unify lang switcher and expand footer links on EN landing page"
```

---

### Task 4: Update `docs/ja/index.html` — language switcher + footer

**Files:**
- Modify: `docs/ja/index.html`

Two changes:

- [ ] **Step 1: Replace language switcher markup**

Old (lines 24-27):
```html
    <div class="lang-switch">
      <a href="../">English</a>
      <span class="active">日本語</span>
    </div>
```

New:
```html
    <div class="lang-switch">
      <a class="lang-btn" href="../">EN</a>
      <span class="lang-btn active">JA</span>
    </div>
```

- [ ] **Step 2: Expand footer links with Japanese labels**

Old (lines 199-205):
```html
  <p>
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="../HOWTO-USE">ドキュメント</a> ·
    Apache-2.0 License
  </p>
  <p style="margin-top:0.5rem">© 2025 sasurahime contributors</p>
```

New:
```html
  <p>
    <a href="https://github.com/armaniacs/sasurahime" target="_blank" rel="noopener">GitHub</a> ·
    <a href="../HOWTO-USE">使い方</a> ·
    <a href="../SUPPORTED">対応ターゲット一覧</a> ·
    <a href="../HOWTO-ADD-target">ターゲット追加方法</a> ·
    Apache-2.0 License
  </p>
  <p style="margin-top:0.5rem">© 2025 sasurahime contributors</p>
```

- [ ] **Step 3: Commit**

```bash
git add docs/ja/index.html
git commit -m "docs: unify lang switcher and expand footer links on JA landing page"
```

---

### Task 5: Add cross-reference links to HOWTO-USE.md

**Files:**
- Modify: `docs/HOWTO-USE.md`

- [ ] **Step 1: Add cross-references to English section end**

Append before the English `</details>` closing tag (approximately line 299). Find the line `</details>` that closes the English `<details open markdown="1">` block.

Insert after the last content before `</details>`:
```markdown

---

📖 See also: [Supported Targets](/SUPPORTED) · [How to Add a Target](/HOWTO-ADD-target)
```

- [ ] **Step 2: Add cross-references to Japanese section end**

Append before the Japanese `</details>` closing tag (approximately line 572). Find the line `</details>` that closes the Japanese `<details markdown="1">` block.

Insert after the last content before `</details>`:
```markdown

---

📖 関連ドキュメント: [対応ターゲット一覧](/SUPPORTED) · [ターゲット追加方法](/HOWTO-ADD-target)
```

- [ ] **Step 3: Verify the structure**

The file should end with:
```markdown

</details>

<details markdown="1">
...

</details>
```

Both `</details>` tags should still be present — one closing the English block, one closing the Japanese block.

- [ ] **Step 4: Commit**

```bash
git add docs/HOWTO-USE.md
git commit -m "docs: add cross-reference links to HOWTO-USE"
```

---

### Verification

After all tasks are committed, run:

```bash
# Verify no broken links in modified landing pages
grep -n 'href=' docs/index.html docs/ja/index.html | grep -v 'github.com' | grep -v 'releases/latest'

# Verify .lang-btn CSS is in style.css
grep -c 'lang-btn' docs/assets/style.css

# Verify .lang-btn is NOT in _layouts/doc.html inline style
grep -c 'lang-btn' docs/_layouts/doc.html
# Expected: matches only in JS (switchLang nav link logic), not in <style> block

# Verify HOWTO-USE cross-references
grep -c '対応ターゲット一覧\|See also' docs/HOWTO-USE.md
# Expected: 2 (one EN, one JA)
```
