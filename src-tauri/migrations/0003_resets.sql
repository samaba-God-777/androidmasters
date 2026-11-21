-- Fase 6: restablecimientos autorizados. AndroidMasters nunca ejecuta el borrado: lo hace el propietario o el técnico
-- en el propio equipo (Ajustes o Recovery). La aplicación registra autorización, confirmación, acciones de apoyo
-- (abrir Ajustes, reiniciar en Recovery) y la verificación posterior.

CREATE TABLE resets (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id         INTEGER REFERENCES devices(id),
    device_label      TEXT    NOT NULL DEFAULT '',
    checklist_id      INTEGER REFERENCES checklists(id),
    method            TEXT    NOT NULL CHECK (method IN ('settings', 'recovery')),
    status            TEXT    NOT NULL CHECK (status IN ('prepared', 'confirmed', 'pending_verification', 'verified', 'failed', 'cancelled')),
    confirmed_at      TEXT,
    confirmed_by      INTEGER REFERENCES users(id),
    owner_name        TEXT    NOT NULL DEFAULT '',
    actions_json      TEXT    NOT NULL DEFAULT '[]',
    verification_json TEXT    NOT NULL DEFAULT '{}',
    created_at        TEXT    NOT NULL,
    updated_at        TEXT    NOT NULL,
    author_id         INTEGER REFERENCES users(id),
    -- A reset can only be verified or attempted after an explicit confirmation.
    CHECK (status IN ('prepared', 'cancelled') OR confirmed_at IS NOT NULL)
);
CREATE INDEX idx_resets_device ON resets(device_id);
