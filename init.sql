-- Initialize PGMQ extension
CREATE EXTENSION IF NOT EXISTS pgmq;

-- The PGMQ extension will handle queue creation in the application
-- This file ensures the extension is available when the worker starts

-- Create table for processed payments
CREATE TABLE IF NOT EXISTS processed_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    correlation_id VARCHAR(255) NOT NULL UNIQUE,
    amount DECIMAL(10,2) NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL,
    processor VARCHAR(50) NOT NULL
);