-- Migration: 0005_profile_photos
-- Add profile.photos array for uploaded profile photos

DEFINE FIELD profile.photos ON person TYPE array<object> FLEXIBLE DEFAULT [] PERMISSIONS FULL;
DEFINE FIELD profile.photos[*].url ON person TYPE string PERMISSIONS FULL;
DEFINE FIELD profile.photos[*].thumbnail_url ON person TYPE string PERMISSIONS FULL;
DEFINE FIELD profile.photos[*].caption ON person TYPE string PERMISSIONS FULL;
