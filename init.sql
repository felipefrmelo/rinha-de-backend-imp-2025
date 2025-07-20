-- Initialize PGMQ extension
CREATE EXTENSION IF NOT EXISTS pgmq;

-- The PGMQ extension will handle queue creation in the application
-- This file ensures the extension is available when the worker starts