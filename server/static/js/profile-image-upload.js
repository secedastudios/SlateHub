/**
 * Profile Image Upload and Crop Module
 * Handles image upload, preview, and basic crop functionality
 */

class ProfileImageUploader {
    constructor(containerId) {
        this.container = document.getElementById(containerId);
        this.imageFile = null;
        this.cropData = {
            x: 0.5,
            y: 0.5,
            zoom: 1.0
        };

        this.init();
    }

    init() {
        this.render();
        this.attachEventListeners();
    }

    render() {
        this.container.innerHTML = `
            <div id="image-upload-area" class="upload-area" data-state="empty">
                <div class="upload-dropzone" id="dropzone">
                    <svg class="upload-icon" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                        <polyline points="17 8 12 3 7 8"></polyline>
                        <line x1="12" y1="3" x2="12" y2="15"></line>
                    </svg>
                    <p class="upload-text">Drag and drop your image here or</p>
                    <button type="button" class="upload-button" id="browse-button">Browse Files</button>
                    <input type="file" id="file-input" accept="image/jpeg,image/png,image/webp" hidden>
                    <p class="upload-hint">Supports: JPEG, PNG, WebP (Max 10MB)</p>
                </div>

                <div class="crop-container" id="crop-container" style="display: none;">
                    <div class="crop-preview-wrapper">
                        <div class="crop-preview" id="crop-preview">
                            <img id="preview-image" src="" alt="Preview">
                            <div class="crop-overlay">
                                <div class="crop-circle"></div>
                            </div>
                        </div>
                    </div>

                    <div class="crop-controls">
                        <div class="zoom-control">
                            <label for="zoom-slider">Zoom:</label>
                            <input type="range" id="zoom-slider" min="1" max="3" step="0.1" value="1">
                            <span id="zoom-value">1.0x</span>
                        </div>

                        <div class="position-hint">
                            <small>Drag image to reposition</small>
                        </div>
                    </div>

                    <div class="crop-actions">
                        <button type="button" id="cancel-crop" class="btn-secondary">Cancel</button>
                        <button type="button" id="upload-image" class="btn-primary">Upload Image</button>
                    </div>
                </div>

                <div class="upload-progress" id="upload-progress" style="display: none;">
                    <div class="progress-bar">
                        <div class="progress-fill" id="progress-fill"></div>
                    </div>
                    <p class="progress-text">Uploading...</p>
                </div>
            </div>
        `;
    }

    attachEventListeners() {
        const dropzone = document.getElementById('dropzone');
        const fileInput = document.getElementById('file-input');
        const browseButton = document.getElementById('browse-button');
        const zoomSlider = document.getElementById('zoom-slider');
        const cancelButton = document.getElementById('cancel-crop');
        const uploadButton = document.getElementById('upload-image');
        const previewImage = document.getElementById('preview-image');

        // File selection
        browseButton.addEventListener('click', () => fileInput.click());
        fileInput.addEventListener('change', (e) => this.handleFileSelect(e.target.files[0]));

        // Drag and drop
        dropzone.addEventListener('dragover', (e) => {
            e.preventDefault();
            dropzone.classList.add('dragover');
        });

        dropzone.addEventListener('dragleave', () => {
            dropzone.classList.remove('dragover');
        });

        dropzone.addEventListener('drop', (e) => {
            e.preventDefault();
            dropzone.classList.remove('dragover');
            const file = e.dataTransfer.files[0];
            if (file && file.type.startsWith('image/')) {
                this.handleFileSelect(file);
            }
        });

        // Zoom control
        zoomSlider.addEventListener('input', (e) => {
            this.cropData.zoom = parseFloat(e.target.value);
            document.getElementById('zoom-value').textContent = `${this.cropData.zoom.toFixed(1)}x`;
            this.updatePreview();
        });

        // Image dragging for position
        let isDragging = false;
        let startX, startY, startCropX, startCropY;

        previewImage.addEventListener('mousedown', (e) => {
            isDragging = true;
            startX = e.clientX;
            startY = e.clientY;
            startCropX = this.cropData.x;
            startCropY = this.cropData.y;
            previewImage.style.cursor = 'grabbing';
        });

        document.addEventListener('mousemove', (e) => {
            if (!isDragging) return;

            const deltaX = (e.clientX - startX) / previewImage.width;
            const deltaY = (e.clientY - startY) / previewImage.height;

            this.cropData.x = Math.max(0, Math.min(1, startCropX - deltaX * this.cropData.zoom));
            this.cropData.y = Math.max(0, Math.min(1, startCropY - deltaY * this.cropData.zoom));

            this.updatePreview();
        });

        document.addEventListener('mouseup', () => {
            isDragging = false;
            previewImage.style.cursor = 'grab';
        });

        // Touch support for mobile
        previewImage.addEventListener('touchstart', (e) => {
            const touch = e.touches[0];
            startX = touch.clientX;
            startY = touch.clientY;
            startCropX = this.cropData.x;
            startCropY = this.cropData.y;
        });

        previewImage.addEventListener('touchmove', (e) => {
            e.preventDefault();
            const touch = e.touches[0];
            const deltaX = (touch.clientX - startX) / previewImage.width;
            const deltaY = (touch.clientY - startY) / previewImage.height;

            this.cropData.x = Math.max(0, Math.min(1, startCropX - deltaX * this.cropData.zoom));
            this.cropData.y = Math.max(0, Math.min(1, startCropY - deltaY * this.cropData.zoom));

            this.updatePreview();
        });

        // Action buttons
        cancelButton.addEventListener('click', () => this.reset());
        uploadButton.addEventListener('click', () => this.uploadImage());
    }

    handleFileSelect(file) {
        if (!file) return;

        // Validate file type
        const validTypes = ['image/jpeg', 'image/png', 'image/webp'];
        if (!validTypes.includes(file.type)) {
            alert('Please select a valid image file (JPEG, PNG, or WebP)');
            return;
        }

        // Validate file size (10MB)
        if (file.size > 10 * 1024 * 1024) {
            alert('File size must be less than 10MB');
            return;
        }

        this.imageFile = file;

        // Read and display the image
        const reader = new FileReader();
        reader.onload = (e) => {
            const img = document.getElementById('preview-image');
            img.src = e.target.result;
            img.onload = () => {
                this.showCropInterface();
            };
        };
        reader.readAsDataURL(file);
    }

    showCropInterface() {
        document.getElementById('dropzone').style.display = 'none';
        document.getElementById('crop-container').style.display = 'block';
        this.updatePreview();
    }

    updatePreview() {
        const img = document.getElementById('preview-image');
        const scale = this.cropData.zoom;
        const translateX = -(this.cropData.x * 100 * (scale - 1));
        const translateY = -(this.cropData.y * 100 * (scale - 1));

        img.style.transform = `scale(${scale}) translate(${translateX}%, ${translateY}%)`;
    }

    async uploadImage() {
        if (!this.imageFile) return;

        // Show progress
        document.getElementById('crop-container').style.display = 'none';
        document.getElementById('upload-progress').style.display = 'block';

        const formData = new FormData();
        formData.append('image', this.imageFile);

        // Add crop parameters as query string
        const params = new URLSearchParams({
            crop_x: this.cropData.x,
            crop_y: this.cropData.y,
            crop_zoom: this.cropData.zoom
        });

        try {
            const response = await fetch(`/api/media/upload/profile-image?${params}`, {
                method: 'POST',
                body: formData,
                credentials: 'same-origin'
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.error || 'Upload failed');
            }

            const result = await response.json();

            // Update the profile image on the page
            this.updateProfileImage(result.url);

            // Reset the uploader
            this.reset();

            // Show success message
            this.showSuccess('Profile image updated successfully!');

        } catch (error) {
            console.error('Upload error:', error);
            alert(`Failed to upload image: ${error.message}`);
            this.reset();
        }
    }

    updateProfileImage(url) {
        // Update all profile image elements on the page
        const profileImages = document.querySelectorAll('[data-profile-image]');
        profileImages.forEach(img => {
            img.src = url;
        });

        // Update the main profile image if it exists
        const mainImage = document.querySelector('#public-profile-image, #profile-image');
        if (mainImage) {
            mainImage.src = url;
        }
    }

    reset() {
        this.imageFile = null;
        this.cropData = { x: 0.5, y: 0.5, zoom: 1.0 };

        document.getElementById('dropzone').style.display = 'block';
        document.getElementById('crop-container').style.display = 'none';
        document.getElementById('upload-progress').style.display = 'none';
        document.getElementById('file-input').value = '';
        document.getElementById('zoom-slider').value = '1';
        document.getElementById('zoom-value').textContent = '1.0x';
    }

    showSuccess(message) {
        // Create a simple success notification
        const notification = document.createElement('div');
        notification.className = 'upload-notification success';
        notification.textContent = message;
        document.body.appendChild(notification);

        setTimeout(() => {
            notification.classList.add('fade-out');
            setTimeout(() => notification.remove(), 300);
        }, 3000);
    }
}

// Initialize when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    // Only initialize if we're on a profile edit page
    const uploadContainer = document.getElementById('profile-image-upload');
    if (uploadContainer) {
        new ProfileImageUploader('profile-image-upload');
    }
});

// TODO: Future enhancements
// - Add image rotation controls
// - Implement aspect ratio selection (square, portrait, landscape)
// - Add image filters and effects
// - Implement undo/redo functionality
// - Add keyboard shortcuts for zoom and position
// - Implement pinch-to-zoom on mobile
// - Add image quality/compression settings
// - Show file size and dimensions before upload
// - Add ability to upload from URL
// - Implement client-side image optimization before upload
