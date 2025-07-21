

-- Tabela ultra-rápida (UNLOGGED)
-- Tabela UNLOGGED + sem PK = máxima velocidade
CREATE UNLOGGED TABLE processed_payments (
    correlation_id TEXT NOT NULL,
    amount NUMERIC(10,2) NOT NULL,
    requested_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processor TEXT NOT NULL
);

-- Apenas índice de unicidade (BTREE suporta unique constraints)
CREATE UNIQUE INDEX idx_correlation_unique 
ON processed_payments USING btree (correlation_id);

-- Índice para requested_at 
CREATE INDEX idx_payments_requested_at 
ON processed_payments USING btree (requested_at DESC);

