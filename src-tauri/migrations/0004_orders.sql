-- Fase 7: órdenes de servicio enlazadas con el trabajo registrado y reportes.

-- Equipos no conectados: marca y modelo escritos por el técnico.
ALTER TABLE service_orders ADD COLUMN device_label TEXT NOT NULL DEFAULT '';

-- Origen de una entrada creada a partir de un registro existente (p. ej. «reset:3:verified»).
-- Evita añadir dos veces el mismo trabajo a la misma orden.
ALTER TABLE order_entries ADD COLUMN source TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_entries_source ON order_entries(order_id, source) WHERE source <> '';
