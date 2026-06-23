// Management-workspace sidebar collapse.
//
// The collapsed state lives on <html data-manage-sidebar="collapsed"> so CSS
// can restyle the whole workspace, and in localStorage so it survives
// navigation. The layout template restores the attribute inline before
// first paint; this file only owns the toggle interaction.
(function () {
    const button = document.querySelector(".manage-collapse");
    if (!button) return;

    const COLLAPSED = "collapsed";
    const root = document.documentElement;

    function apply() {
        const collapsed = root.dataset.manageSidebar === COLLAPSED;
        button.setAttribute("aria-expanded", String(!collapsed));
        const label = collapsed ? "Expand sidebar" : "Collapse sidebar";
        button.setAttribute("aria-label", label);
        button.setAttribute("title", label);
    }

    button.addEventListener("click", function () {
        if (root.dataset.manageSidebar === COLLAPSED) {
            delete root.dataset.manageSidebar;
            localStorage.removeItem("manage-sidebar");
        } else {
            root.dataset.manageSidebar = COLLAPSED;
            localStorage.setItem("manage-sidebar", COLLAPSED);
        }
        apply();
    });

    apply();
})();

// Script-upload dropzone: filename feedback + drag-over highlight. The
// mechanics (click-to-browse, native drop onto the stretched file input)
// are pure HTML/CSS; this only narrates state.
(function () {
    const zone = document.querySelector(".script-dropzone");
    if (!zone) return;

    const input = zone.querySelector("input[type=file]");
    const label = zone.querySelector("[data-role=dropzone-label]");

    input.addEventListener("change", function () {
        if (input.files.length > 0) {
            label.textContent = input.files[0].name;
            zone.classList.add("has-file");
        } else {
            zone.classList.remove("has-file");
        }
    });

    ["dragenter", "dragover"].forEach(function (event) {
        zone.addEventListener(event, function () {
            zone.classList.add("is-dragover");
        });
    });
    ["dragleave", "drop"].forEach(function (event) {
        zone.addEventListener(event, function () {
            zone.classList.remove("is-dragover");
        });
    });
})();
