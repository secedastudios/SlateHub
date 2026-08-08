/**
 * Scroll reveal — fades elements in as they scroll into view.
 *
 * Usage: put data-reveal on any element. It starts hidden and gets
 * data-revealed="true" the first time it enters the viewport. All the motion
 * lives in CSS (main.css, "SCROLL REVEAL" block); this file only sets
 * attributes.
 *
 * Fail-open by design, and the two halves are deliberately in ONE file:
 *
 *   - The hidden start state in CSS is gated behind [data-reveal-ready] on
 *     <html>, which is set here. If this file fails to load or parse, that
 *     attribute is never set, the hidden rule never matches, and everything
 *     renders normally.
 *   - The flag must therefore NOT be set anywhere else (e.g. inline in the
 *     page head). Splitting them would mean a broken revealer could leave
 *     content hidden forever — the one failure mode worth engineering out.
 *
 * Loaded synchronously from <head> so the flag lands before the body paints;
 * observation itself waits for DOMContentLoaded. The file is tiny and
 * same-origin, so the parser pause is negligible next to a guaranteed
 * flash-free first render.
 *
 * Reduced motion is handled in CSS, not here — a media query keeps responding
 * if the OS setting changes mid-session, which a one-time matchMedia read
 * would not.
 */
(function () {
    "use strict";

    var root = document.documentElement;

    // Old browser: leave the flag unset so nothing is ever hidden.
    if (!("IntersectionObserver" in window)) {
        return;
    }

    // Arm the CSS. Anything below here that throws must undo this.
    root.dataset.revealReady = "true";

    function observeAll() {
        var targets = document.querySelectorAll("[data-reveal]");
        if (!targets.length) {
            return;
        }

        var observer = new IntersectionObserver(
            function (entries) {
                for (var i = 0; i < entries.length; i++) {
                    if (!entries[i].isIntersecting) {
                        continue;
                    }
                    entries[i].target.dataset.revealed = "true";
                    // Reveal once — no re-hiding when scrolling back up.
                    observer.unobserve(entries[i].target);
                }
            },
            {
                // Fire once the element's top edge is a little way into the
                // viewport, so the fade reads as deliberate. The threshold is
                // safe for every current target (all are far shorter than the
                // viewport); prefer lowering it to raising it if taller
                // elements ever get data-reveal.
                rootMargin: "0px 0px -8% 0px",
                threshold: 0.15,
            },
        );

        for (var j = 0; j < targets.length; j++) {
            observer.observe(targets[j]);
        }
    }

    function init() {
        try {
            observeAll();
        } catch (e) {
            // Never leave content stranded behind the hidden rule.
            delete root.dataset.revealReady;
        }
    }

    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", init);
    } else {
        init();
    }
})();
