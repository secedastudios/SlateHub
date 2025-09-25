/**
 * Organization Logo Upload and Crop Module
 * Handles logo upload, preview, and basic crop functionality
 * Supports SVG in addition to standard image formats
 */

class OrganizationLogoUploader {
    constructor(containerId, orgSlug) {
        this.container = document.getElementById(containerId);
        this.orgSlug = orgSlug;
        this.imageFile = null;
        this.cropData = {
            x: 0.5,
            y: 0.5,
            zoom: 1.0,
        };
        this.isSvg = false;

        this.init();
    }

    init() {
        this.render();
        this.attachEventListeners();
    }

    render() {
        this.container.innerHTML = `
            <div id="logo-upload-area" class="upload-area" data-state="empty">
                <div class="upload-dropzone" id="logo-dropzone">
                    <svg class="upload-icon" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                        <polyline points="17 8 12 3 7 8"></polyline>
                        <line x1="12" y1="3" x2="12" y2="15"></line>
                    </svg>
                    <p class="upload-text">Drag and drop your logo here or</p>
                    <button type="button" class="upload-button" id="logo-browse-button">Browse Files</button>
                    <input type="file" id="logo-file-input" accept="image/jpeg,image/png,image/webp,image/svg+xml" hidden>
                    <p class="upload-hint">Supports: JPEG, PNG, WebP, SVG (Max 10MB)</p>
                </div>

                <div class="crop-container" id="logo-crop-container" style="display: none;">
                    <div class="crop-preview-wrapper">
                        <div class="crop-preview" id="logo-crop-preview">
                            <img id="logo-preview-image" src="" alt="Logo Preview">
                            <div class="crop-overlay" id="logo-crop-overlay">
                                <div class="crop-circle"></div>
                            </div>
                        </div>
                    </div>

                    <div class="crop-controls" id="logo-crop-controls">
                        <div class="zoom-control">
                            <label for="logo-zoom-slider">Zoom:</label>
                            <input type="range" id="logo-zoom-slider" min="1" max="3" step="0.1" value="1">
                            <span id="logo-zoom-value">1.0x</span>
                        </div>

                        <div class="position-hint">
                            <small>Drag image to reposition</small>
                        </div>
                    </div>

                    <div class="crop-actions">
                        <button type="button" id="logo-cancel-crop" class="btn-secondary">Cancel</button>
                        <button type="button" id="logo-upload-image" class="btn-primary">Upload Logo</button>
                    </div>
                </div>

                <div class="upload-progress" id="logo-upload-progress" style="display: none;">
                    <div class="progress-bar">
                        <div class="progress-fill" id="logo-progress-fill"></div>
                    </div>
                    <p class="progress-text">Uploading logo...</p>
                </div>
            </div>
        `;
    }

    attachEventListeners() {
        const dropzone = document.getElementById("logo-dropzone");
        const fileInput = document.getElementById("logo-file-input");
        const browseButton = document.getElementById("logo-browse-button");
        const zoomSlider = document.getElementById("logo-zoom-slider");
        const cancelButton = document.getElementById("logo-cancel-crop");
        const uploadButton = document.getElementById("logo-upload-image");
        const previewImage = document.getElementById("logo-preview-image");

        // File selection
        browseButton.addEventListener("click", () => fileInput.click());
        fileInput.addEventListener("change", (e) =>
            this.handleFileSelect(e.target.files[0]),
        );

        // Drag and drop
        dropzone.addEventListener("dragover", (e) => {
            e.preventDefault();
            dropzone.classList.add("dragover");
        });

        dropzone.addEventListener("dragleave", () => {
            dropzone.classList.remove("dragover");
        });

        dropzone.addEventListener("drop", (e) => {
            e.preventDefault();
            dropzone.classList.remove("dragover");
            const file = e.dataTransfer.files[0];
            if (
                file &&
                (file.type.startsWith("image/") ||
                    file.type === "image/svg+xml")
            ) {
                this.handleFileSelect(file);
            }
        });

        // Zoom control (only for non-SVG)
        zoomSlider.addEventListener("input", (e) => {
            if (!this.isSvg) {
                this.cropData.zoom = parseFloat(e.target.value);
                document.getElementById("logo-zoom-value").textContent =
                    `${this.cropData.zoom.toFixed(1)}x`;
                this.updatePreview();
            }
        });

        // Image dragging for position (only for non-SVG)
        let isDragging = false;
        let startX, startY, startCropX, startCropY;

        previewImage.addEventListener("mousedown", (e) => {
            if (this.isSvg) return;
            isDragging = true;
            startX = e.clientX;
            startY = e.clientY;
            startCropX = this.cropData.x;
            startCropY = this.cropData.y;
            previewImage.style.cursor = "grabbing";
        });

        document.addEventListener("mousemove", (e) => {
            if (!isDragging || this.isSvg) return;

            const deltaX = (e.clientX - startX) / previewImage.width;
            const deltaY = (e.clientY - startY) / previewImage.height;

            this.cropData.x = Math.max(
                0,
                Math.min(1, startCropX - deltaX * this.cropData.zoom),
            );
            this.cropData.y = Math.max(
                0,
                Math.min(1, startCropY - deltaY * this.cropData.zoom),
            );

            this.updatePreview();
        });

        document.addEventListener("mouseup", () => {
            isDragging = false;
            if (!this.isSvg) {
                previewImage.style.cursor = "grab";
            }
        });

        // Touch support for mobile
        previewImage.addEventListener("touchstart", (e) => {
            if (this.isSvg) return;
            const touch = e.touches[0];
            startX = touch.clientX;
            startY = touch.clientY;
            startCropX = this.cropData.x;
            startCropY = this.cropData.y;
        });

        previewImage.addEventListener("touchmove", (e) => {
            if (this.isSvg) return;
            e.preventDefault();
            const touch = e.touches[0];
            const deltaX = (touch.clientX - startX) / previewImage.width;
            const deltaY = (touch.clientY - startY) / previewImage.height;

            this.cropData.x = Math.max(
                0,
                Math.min(1, startCropX - deltaX * this.cropData.zoom),
            );
            this.cropData.y = Math.max(
                0,
                Math.min(1, startCropY - deltaY * this.cropData.zoom),
            );

            this.updatePreview();
        });

        // Action buttons
        cancelButton.addEventListener("click", () => this.reset());
        uploadButton.addEventListener("click", () => this.uploadLogo());
    }

    handleFileSelect(file) {
        if (!file) return;

        // Check if it's an SVG
        this.isSvg = file.type === "image/svg+xml";

        // Validate file type
        const validTypes = [
            "image/jpeg",
            "image/png",
            "image/webp",
            "image/svg+xml",
        ];
        if (!validTypes.includes(file.type)) {
            alert("Please select a valid image file (JPEG, PNG, WebP, or SVG)");
            return;
        }

        // Validate file size (10MB)
        if (file.size > 10 * 1024 * 1024) {
            alert("File size must be less than 10MB");
            return;
        }

        this.imageFile = file;

        // Read and display the image
        const reader = new FileReader();
        reader.onload = (e) => {
            const img = document.getElementById("logo-preview-image");
            img.src = e.target.result;
            img.onload = () => {
                this.showCropInterface();
            };
        };
        reader.readAsDataURL(file);
    }

    showCropInterface() {
        document.getElementById("logo-dropzone").style.display = "none";
        document.getElementById("logo-crop-container").style.display = "block";

        // Hide crop controls for SVG files
        if (this.isSvg) {
            document.getElementById("logo-crop-controls").style.display =
                "none";
            document.getElementById("logo-crop-overlay").style.display = "none";
            document.getElementById("logo-preview-image").style.cursor =
                "default";
        } else {
            document.getElementById("logo-crop-controls").style.display =
                "block";
            document.getElementById("logo-crop-overlay").style.display =
                "block";
            document.getElementById("logo-preview-image").style.cursor = "grab";
            this.updatePreview();
        }
    }

    updatePreview() {
        if (this.isSvg) return;

        const img = document.getElementById("logo-preview-image");
        const scale = this.cropData.zoom;
        const translateX = -(this.cropData.x * 100 * (scale - 1));
        const translateY = -(this.cropData.y * 100 * (scale - 1));

        img.style.transform = `scale(${scale}) translate(${translateX}%, ${translateY}%)`;
    }

    async uploadLogo() {
        if (!this.imageFile) return;

        // Show progress
        document.getElementById("logo-crop-container").style.display = "none";
        document.getElementById("logo-upload-progress").style.display = "block";

        const formData = new FormData();
        formData.append("image", this.imageFile);

        // Add crop parameters as query string (only for non-SVG)
        let queryParams = "";
        if (!this.isSvg) {
            const params = new URLSearchParams({
                crop_x: this.cropData.x,
                crop_y: this.cropData.y,
                crop_zoom: this.cropData.zoom,
            });
            queryParams = `?${params}`;
        }

        try {
            const response = await fetch(
                `/api/media/upload/organization-logo/${this.orgSlug}${queryParams}`,
                {
                    method: "POST",
                    body: formData,
                    credentials: "same-origin",
                },
            );

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.error || "Upload failed");
            }

            const result = await response.json();

            // Update the logo on the page
            this.updateOrganizationLogo(result.url);

            // Reset the uploader
            this.reset();

            // Show success message
            this.showSuccess("Organization logo updated successfully!");
        } catch (error) {
            console.error("Upload error:", error);
            alert(`Failed to upload logo: ${error.message}`);
            this.reset();
        }
    }

    updateOrganizationLogo(url) {
        // Update all organization logo elements on the page
        const logoImages = document.querySelectorAll(
            "[data-organization-logo]",
        );
        logoImages.forEach((img) => {
            img.src = url;
        });

        // Update or create the current logo preview if it doesn't exist
        const currentLogoContainer = document.getElementById(
            "current-logo-container",
        );
        if (currentLogoContainer) {
            const existingLogo = currentLogoContainer.querySelector(
                '[data-role="logo-preview"]',
            );
            if (existingLogo) {
                const img = existingLogo.querySelector("img");
                if (img) {
                    img.src = url;
                }
            } else {
                // Create new logo preview
                currentLogoContainer.innerHTML = `
                    <div data-role="logo-preview">
                        <img
                            id="organization-logo-current"
                            src="${url}"
                            alt="Organization logo"
                            data-organization-logo
                        />
                        <p data-role="help-text">Current logo</p>
                    </div>
                `;
            }
        }

        // Update logo in navigation or header if present
        const navLogo = document.querySelector('[data-role="org-logo"]');
        if (navLogo) {
            navLogo.src = url;
        }
    }

    reset() {
        this.imageFile = null;
        this.isSvg = false;
        this.cropData = { x: 0.5, y: 0.5, zoom: 1.0 };

        document.getElementById("logo-dropzone").style.display = "block";
        document.getElementById("logo-crop-container").style.display = "none";
        document.getElementById("logo-upload-progress").style.display = "none";
        document.getElementById("logo-file-input").value = "";
        document.getElementById("logo-zoom-slider").value = "1";
        document.getElementById("logo-zoom-value").textContent = "1.0x";
        document.getElementById("logo-crop-controls").style.display = "block";
        document.getElementById("logo-crop-overlay").style.display = "block";
    }

    showSuccess(message) {
        // Create a simple success notification
        const notification = document.createElement("div");
        notification.className = "upload-notification success";
        notification.textContent = message;
        document.body.appendChild(notification);

        setTimeout(() => {
            notification.classList.add("fade-out");
            setTimeout(() => notification.remove(), 300);
        }, 3000);
    }
}

// Auto-initialize if we're on an organization edit page and have the container
document.addEventListener("DOMContentLoaded", () => {
    const uploadContainer = document.getElementById("organization-logo-upload");
    if (uploadContainer && window.orgSlug) {
        new OrganizationLogoUploader(
            "organization-logo-upload",
            window.orgSlug,
        );
    }
});
