// SlatHub IMDb Import — Content Script
// Runs on imdb.com/name/nm* pages to scrape filmography data

(function () {
  "use strict";

  const personId =
    window.location.pathname.match(/\/name\/(nm\d+)/)?.[1] || null;

  if (!personId) return;

  function scrapeFilmography() {
    const credits = [];

    // IMDb uses data-testid="nm-flmg-*" sections for filmography categories
    const sections = document.querySelectorAll(
      '[data-testid^="nm-flmg-"]'
    );

    for (const section of sections) {
      // Category from section data-testid: "nm-flmg-actor" -> "actor"
      const testId = section.getAttribute("data-testid") || "";
      const category = testId.replace("nm-flmg-", "").replace(/-/g, " ");

      // Each credit row within the section
      const rows = section.querySelectorAll(
        '[class*="ipc-metadata-list-summary-item"]'
      );

      for (const row of rows) {
        // Title link contains /title/ttXXXXXX
        const link = row.querySelector('a[href*="/title/tt"]');
        if (!link) continue;

        const title = link.textContent?.trim() || "";
        if (!title) continue;

        // Year — look for year-related spans
        let year = null;
        const yearEl =
          row.querySelector('[class*="ipc-metadata-list-summary-item__li"]') ||
          row.querySelector("span.ipc-metadata-list-summary-item__li");
        if (yearEl) {
          const match = yearEl.textContent?.match(/(\d{4})/);
          if (match) year = match[1];
        }
        // Fallback: scan all text nodes for a 4-digit year
        if (!year) {
          const allText = row.textContent || "";
          const yearMatch = allText.match(/\b(19|20)\d{2}\b/);
          if (yearMatch) year = yearMatch[0];
        }

        // Role / character name — varies by section
        let role = null;
        // For actors, character is often in a separate span or after "..."
        const charEl = row.querySelector(
          '[data-testid="title-cast-item__character"], [class*="character"]'
        );
        if (charEl) {
          role = charEl.textContent?.trim() || null;
        }
        // Fallback: look for text after the title that looks like a character/role
        if (!role) {
          const spans = row.querySelectorAll("span, li");
          for (const span of spans) {
            const text = span.textContent?.trim();
            if (
              text &&
              text !== title &&
              !text.match(/^\d{4}/) &&
              text.length > 1 &&
              text.length < 100 &&
              !span.querySelector("a")
            ) {
              // Skip year-only and episode count spans
              if (!text.match(/^\d+\s+ep/i) && !text.match(/^[\d–\-]+$/)) {
                role = text;
                break;
              }
            }
          }
        }

        credits.push({
          title,
          year,
          role,
          category: category || "unknown",
        });
      }
    }

    // Deduplicate by title + role
    const seen = new Set();
    const unique = credits.filter((c) => {
      const key = `${c.title}|${c.role || ""}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });

    return unique;
  }

  // Scrape and store results
  const credits = scrapeFilmography();
  const personName =
    document.querySelector('[data-testid="hero__pageTitle"]')?.textContent?.trim() ||
    document.querySelector("h1")?.textContent?.trim() ||
    "Unknown";

  chrome.storage.local.set({
    scrapedCredits: credits,
    imdbPersonId: personId,
    imdbPersonName: personName,
    scrapedAt: Date.now(),
  });

  // Also respond to messages from the popup requesting a fresh scrape
  chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
    if (msg.action === "scrape") {
      const fresh = scrapeFilmography();
      chrome.storage.local.set({
        scrapedCredits: fresh,
        imdbPersonId: personId,
        imdbPersonName: personName,
        scrapedAt: Date.now(),
      });
      sendResponse({ credits: fresh, personId, personName });
    }
  });
})();
