-- Add signup IP address field to person table
DEFINE FIELD signup_ip ON person TYPE option<string> PERMISSIONS FULL;
