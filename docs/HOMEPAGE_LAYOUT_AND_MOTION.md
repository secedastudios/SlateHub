# Homepage Layout & Motion

Reference for the homepage's width system, its motion layer, and the media
embedded in it. All of it lives in `server/static/css/main.css` (there is no
`pages/home.css`) and `server/templates/index.html`.

Everything here follows `docs/HTML_CSS_GUIDELINES.md`: no presentational
classes, no inline styles, no hardcoded colours — styling hangs off IDs and
`data-` attributes, and colours come from the tokens in `main.css` `:root`.

## Width system

The homepage is full-bleed: `[data-page="home"] #main-content` has `padding: 0`,
so each section owns its own horizontal inset. The hero and the sections below
it are deliberately **not** on a shared container.

### Sections below the hero

One custom property drives all seven:

```css
#home-main {
    --home-section-inset: calc(var(--space-xl) + 7.5%);
}
```

Applied as the horizontal half of `padding` on `#section-why`, `#section-who`,
`#section-orgs`, `#section-business-model`, `#section-roadmap`,
`#section-testimonials` and `#section-final-cta`.

The inset grows the padding rather than adding a `max-width`, because these
sections carry full-bleed background colours and artwork that must still reach
both screen edges while the *content* narrows. **Change this one value to make
the page read wider or narrower.**

A percentage is used rather than `vw` so a visible scrollbar cannot tip the page
into horizontal overflow.

The `@media (max-width: 768px)` rules restate `padding` with `var(--space-lg)`,
so mobile keeps its tighter gutters automatically and is unaffected by this
property.

### Hero

`#section-hero` has no horizontal padding at all. Its left inset lives on
`#hero-content`, and its right column bleeds to the viewport edge on purpose.

```css
padding: var(--space-4xl) 0 var(--space-4xl)
    calc(var(--space-xl) + clamp(0px, (100vw - 900px) * 0.2, 5.5vw));
```

The inset tapers to zero at 900px. This is load-bearing, not decoration: the
left column is only ~45% of the viewport, and a fixed inset squeezed the CTA row
until the second button overflowed. The two halves of the `clamp()` cross over
smoothly, so there is no jump while resizing.

The hero grid uses `minmax(0, 1fr) minmax(0, 1.2fr)`. A bare `fr` track will not
shrink below its content's min-content width, which let an over-wide item push
the whole grid past the viewport.

`#hero-actions` is `flex-wrap: wrap` — both buttons are `white-space: nowrap`
and together need ~470px, more than the column has below roughly 1100px.

### Hero backdrop

Two layers under the copy:

1. A warm gradient on the element itself, transparent until 40% and building to
   a 10% accent wash at the right edge, so the column resolves into the
   photograph instead of stopping dead against it. The colour is derived with
   `color-mix()` from `--color-accent` rather than a literal. *(Older gradients
   elsewhere in `main.css` use raw `rgba()`; they predate the token rule.)*
2. The `who-bg.png` texture on `#hero-content::before` at `opacity: 0.18`,
   masked with `linear-gradient(to right, transparent 0%, #000 75%)` so the
   grain is absent behind the copy and only builds toward the photo. The `#000`
   in a mask is alpha, not a visible colour.

## Motion

### Scroll reveal — the convention

**This is the repo's only scroll-reveal mechanism and its only
`IntersectionObserver`.** Datastar's `data-on-intersect` was not used because it
fires a server request rather than a visual toggle.

To reveal something on scroll, add one attribute:

```html
<div id="my-intro" data-role="section-intro" data-reveal>
```

`server/static/js/scroll-reveal.js` observes every `[data-reveal]` and sets
`data-revealed="true"` the first time it enters the viewport, once — it never
re-hides on scroll back up. All motion is CSS.

Mark the **content groups**, not the `<section>` itself. Fading a whole section
fades its background colour block too, which reads as a flash against the
neighbouring sections. Currently marked: `#section-why`, `#section-orgs` and
`#section-roadmap`.

Cards inside a `[data-role="features-grid"]` get a staggered delay
(60/120/180/240ms) automatically — no extra markup.

#### Two rules that must not be broken

**1. The arming flag and the revealer live in the same file.** The hidden start
state is gated behind `[data-reveal-ready]` on `<html>`, which is set by
`scroll-reveal.js` itself:

```css
[data-reveal-ready] [data-reveal] { opacity: 0; transform: translateY(16px); }
```

If the script fails to load, the attribute is never set, the hidden rule never
matches, and everything renders normally. **Do not set `data-reveal-ready`
anywhere else** — in particular not inline in the page head. Splitting the flag
from the revealer means a broken script leaves content permanently invisible.

That is also why the script is loaded **synchronously in `<head>`**, not
deferred at the foot: it has to arm the CSS before the body paints, or revealed
content flashes in and back out. Do not add `defer`.

**2. Reduced motion needs its own rule.** The global block in `main.css` only
sets `animation-duration`/`transition-duration` to `0.01ms`. It cannot undo an
`opacity: 0` start state, so the reveal ships an explicit
`@media (prefers-reduced-motion: reduce)` block forcing the final visible state.
Any future opacity-based effect needs the same treatment.

Reduced motion is handled in CSS rather than a `matchMedia` read in JS, so it
keeps responding if the OS setting changes mid-session.

### Hover

All seven homepage CTA pills lift with `transform: scale(1.03)`, echoing the
`translateY(-2px)` that `[data-component="feature-card"]` already used.

`transition` is a single property, so adding `transform` means restating the
colour transitions alongside it. The hero and roadmap buttons declare it inline
on their own ID rules; the remaining four share one grouped rule that must stay
*after* every button rule above it — each selector matches its original at equal
specificity, so source order is what makes it win.

Two other hover treatments: `#hero-tag` sweeps an accent fill in from the left
(`::before` scaled on X, behind the text via `isolation: isolate`), and
`#brand-slogan` lifts from 40% opacity to full accent when `#nav-brand` is
hovered or focused — triggered from the lockup so it responds to the logo too.

No reduced-motion handling is needed for hover; the global rule collapses the
durations, which is correct for a hover state.

## Media

### Hero image

`Collab1.jpg`, with `object-fit: cover` and `object-position: 50% 35%`.

Every face in the frame sits in the upper third. The photo is square while the
column is usually taller than it is wide, so `cover` normally crops the sides and
keeps full height — but on a short, wide window it crops vertically instead, and
a centred crop would start cutting into faces. The 35% bias sacrifices ceiling
instead.

**If the hero image is ever replaced, re-check two things:** this value, and the
mobile position of `[data-component="hero-search"]`. That panel sits at
`top: 60%` on mobile specifically to clear this photo's faces — the previous
value of 35% was framed for the previous image and landed straight across them.

Known limitation: the file is 1024×1024, noticeably smaller than the 2499×1666
image it replaced, so it renders at roughly half the pixel density a high-DPI
display wants in that column. A larger original would be a drop-in improvement.

### "So How Is This Free?" video

`#business-model-inner` is a two-column grid — copy left, player right —
collapsing to one column at 768px with the player below the copy (DOM order).
The player is 80% width and centred on mobile; at full width it dominated the
stacked layout.

The embed uses `youtube-nocookie.com` so a homepage visit does not drop YouTube
tracking cookies on people who never press play, and `loading="lazy"` so it
costs nothing until scrolled near. The 16:9 box is enforced with
`aspect-ratio`, so the iframe cannot cause layout shift as it loads.

The grid uses `minmax(0, 1fr)` for the same reason as the hero.

### Roadmap icons

`#phase-01` and `#phase-02` carry inline SVG (clapperboard, call sheet) at the
same 40px rhythm as the feature-card icons, in `--color-accent`.

Inline SVG rather than the PNG convention used by `#section-why`: `static/icons/`
holds only brand and favicon assets, so matching the PNG pattern would have meant
authoring new binary artwork. Inline SVG takes its colour from `currentColor` and
adds no requests. Both are `aria-hidden="true"` — the `h3` already carries the
meaning.

The icons describe what each phase *delivers*, not its timing: the existing
`[data-role="phase-label"]::before` status dots (filled green / hollow) already
say "now" and "soon".

## Mobile specifics

- `#section-roadmap` is centred on mobile only — pill, heading, icons, labels
  (with their status dots) and CTA all on one axis, with `--space-2xl` gutters.
  The cards are flex columns, so this needs `align-items: center` and
  `justify-content: center`; `text-align` alone does not move them. Desktop keeps
  its left-aligned, divider-separated columns.
- `#why-features` drops from 5 to 3 columns between 769px and 1100px, with the
  divider logic mirrored for 3 columns. It is a **range** query — an open-ended
  `max-width` would leak into the 2-column mobile layout and strip card 4's
  border.

## Known issue, not addressed here

The site header overflows horizontally between roughly 769px and 1000px — the
nav does not collapse until 768px, so the full desktop nav renders in a window
too narrow for it and the page scrolls sideways. It predates this work and is
tracked separately.
