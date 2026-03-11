-- Migration: 0004_profile_reels
-- Change profile.reels from array<record<media>> to array<object> for video links

-- Remove old field definition and redefine as flexible object array
REMOVE FIELD IF EXISTS profile.reels ON person;
DEFINE FIELD profile.reels ON person TYPE array<object> FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD profile.reels[*].url ON person TYPE string PERMISSIONS FULL;
DEFINE FIELD profile.reels[*].title ON person TYPE string PERMISSIONS FULL;
DEFINE FIELD profile.reels[*].platform ON person TYPE string PERMISSIONS FULL;
DEFINE FIELD profile.reels[*].video_id ON person TYPE string PERMISSIONS FULL;
