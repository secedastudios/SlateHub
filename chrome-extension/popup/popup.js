// SlatHub IMDb Import — Popup Script

(function () {
  "use strict";

  // Determine SlatHub base URL — try production first, fall back to localhost
  const SLATEHUB_URLS = ["https://slatehub.com", "http://localhost:3000"];

  const $authRequired = document.getElementById("auth-required");
  const $noData = document.getElementById("no-data");
  const $creditsView = document.getElementById("credits-view");
  const $resultView = document.getElementById("result-view");
  const $loading = document.getElementById("loading");

  function showState(el) {
    [$authRequired, $noData, $creditsView, $resultView, $loading].forEach(
      (e) => (e.style.display = "none")
    );
    el.style.display = "block";
  }

  // --- Auth ---

  async function getAuth() {
    for (const url of SLATEHUB_URLS) {
      try {
        const cookie = await chrome.cookies.get({
          url,
          name: "auth_token",
        });
        if (cookie) {
          return { token: cookie.value, baseUrl: url };
        }
      } catch {
        // continue
      }
    }
    return null;
  }

  // --- Credits rendering ---

  function renderCredits(credits) {
    const $list = document.getElementById("credits-list");
    $list.innerHTML = "";

    // Group by category
    const groups = {};
    for (const c of credits) {
      const cat = c.category || "other";
      if (!groups[cat]) groups[cat] = [];
      groups[cat].push(c);
    }

    // Preferred category order
    const order = [
      "actor",
      "actress",
      "self",
      "director",
      "producer",
      "writer",
      "cinematographer",
      "composer",
      "editor",
    ];
    const sortedKeys = Object.keys(groups).sort((a, b) => {
      const ai = order.indexOf(a);
      const bi = order.indexOf(b);
      if (ai === -1 && bi === -1) return a.localeCompare(b);
      if (ai === -1) return 1;
      if (bi === -1) return -1;
      return ai - bi;
    });

    for (const cat of sortedKeys) {
      const header = document.createElement("div");
      header.className = "credit-category";
      header.textContent = `${cat} (${groups[cat].length})`;
      $list.appendChild(header);

      for (const credit of groups[cat]) {
        const item = document.createElement("label");
        item.className = "credit-item";

        const cb = document.createElement("input");
        cb.type = "checkbox";
        cb.checked = true;
        cb.dataset.index = credits.indexOf(credit);

        const info = document.createElement("div");
        info.className = "credit-info";

        const title = document.createElement("div");
        title.className = "credit-title";
        title.textContent = credit.title;

        const meta = document.createElement("div");
        meta.className = "credit-meta";
        const parts = [];
        if (credit.year) parts.push(credit.year);
        if (credit.role) parts.push(credit.role);
        meta.textContent = parts.join(" \u2022 ");

        info.appendChild(title);
        info.appendChild(meta);
        item.appendChild(cb);
        item.appendChild(info);
        $list.appendChild(item);
      }
    }

    updateImportButton();
  }

  function getSelectedCredits(allCredits) {
    const checkboxes = document.querySelectorAll(
      '#credits-list input[type="checkbox"]'
    );
    const selected = [];
    for (const cb of checkboxes) {
      if (cb.checked) {
        selected.push(allCredits[parseInt(cb.dataset.index)]);
      }
    }
    return selected;
  }

  function updateImportButton() {
    const checked = document.querySelectorAll(
      '#credits-list input[type="checkbox"]:checked'
    ).length;
    const btn = document.getElementById("import-btn");
    btn.textContent = `Import Selected (${checked})`;
    btn.disabled = checked === 0;
  }

  // --- Import ---

  async function importCredits(auth, credits) {
    const resp = await fetch(`${auth.baseUrl}/api/imdb/import`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${auth.token}`,
      },
      body: JSON.stringify({ credits }),
    });

    if (!resp.ok) {
      const text = await resp.text();
      throw new Error(`Import failed (${resp.status}): ${text}`);
    }

    return resp.json();
  }

  // --- Init ---

  async function init() {
    // Check auth
    const auth = await getAuth();
    if (!auth) {
      showState($authRequired);
      return;
    }

    // Update login link to match detected environment
    document.getElementById("login-link").href = `${auth.baseUrl}/login`;
    document.getElementById("profile-link").href = `${auth.baseUrl}/profile`;

    // Get scraped data from storage
    const data = await chrome.storage.local.get([
      "scrapedCredits",
      "imdbPersonId",
      "imdbPersonName",
      "scrapedAt",
    ]);

    if (
      !data.scrapedCredits ||
      data.scrapedCredits.length === 0 ||
      !data.scrapedAt ||
      Date.now() - data.scrapedAt > 5 * 60 * 1000
    ) {
      // Try to scrape from active tab
      try {
        const [tab] = await chrome.tabs.query({
          active: true,
          currentWindow: true,
        });
        if (tab && tab.url && tab.url.match(/imdb\.com\/name\/nm/)) {
          const response = await chrome.tabs.sendMessage(tab.id, {
            action: "scrape",
          });
          if (response && response.credits && response.credits.length > 0) {
            data.scrapedCredits = response.credits;
            data.imdbPersonName = response.personName;
            data.imdbPersonId = response.personId;
          }
        }
      } catch {
        // Content script not available
      }
    }

    if (!data.scrapedCredits || data.scrapedCredits.length === 0) {
      showState($noData);
      return;
    }

    // Show credits
    const credits = data.scrapedCredits;
    document.getElementById("person-name").textContent =
      data.imdbPersonName || "Unknown";
    document.getElementById("credit-count").textContent =
      `${credits.length} credits`;

    renderCredits(credits);
    showState($creditsView);

    // Event listeners
    document.getElementById("select-all").addEventListener("click", () => {
      document
        .querySelectorAll('#credits-list input[type="checkbox"]')
        .forEach((cb) => (cb.checked = true));
      updateImportButton();
    });

    document.getElementById("deselect-all").addEventListener("click", () => {
      document
        .querySelectorAll('#credits-list input[type="checkbox"]')
        .forEach((cb) => (cb.checked = false));
      updateImportButton();
    });

    document
      .getElementById("credits-list")
      .addEventListener("change", updateImportButton);

    document
      .getElementById("import-btn")
      .addEventListener("click", async () => {
        const selected = getSelectedCredits(credits);
        if (selected.length === 0) return;

        showState($loading);
        document.getElementById("loading-text").textContent =
          `Importing ${selected.length} credits...`;

        try {
          const result = await importCredits(auth, selected);

          const $msg = document.getElementById("result-message");
          $msg.innerHTML = `
            <p class="result-success">Import complete!</p>
            <p class="result-detail">
              ${result.imported} imported, ${result.skipped} already existed
              ${result.errors.length > 0 ? `, ${result.errors.length} errors` : ""}
            </p>
          `;

          showState($resultView);

          // Clear stored data
          chrome.storage.local.remove([
            "scrapedCredits",
            "imdbPersonId",
            "imdbPersonName",
            "scrapedAt",
          ]);
        } catch (err) {
          const $msg = document.getElementById("result-message");
          const errP = document.createElement("p");
          errP.style.color = "#dc2626";
          errP.textContent = `Error: ${err.message}`;
          $msg.innerHTML = "";
          $msg.appendChild(errP);
          showState($resultView);
        }
      });
  }

  init();
})();
