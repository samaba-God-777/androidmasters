-- Fase 5: comprobaciones de cuentas (solo tipos y cantidades) y listas de preparación para transferencia.

CREATE TABLE account_checks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id       INTEGER NOT NULL REFERENCES devices(id),
    created_at      TEXT    NOT NULL,
    author_id       INTEGER REFERENCES users(id),
    summary_json    TEXT    NOT NULL,           -- [{label, count, protects}] — never account names
    google_count    INTEGER NOT NULL,
    protected_count INTEGER NOT NULL
);
CREATE INDEX idx_account_checks_device ON account_checks(device_id);

CREATE TABLE checklists (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kind         TEXT    NOT NULL CHECK (kind IN ('transfer')),
    device_id    INTEGER REFERENCES devices(id),
    device_label TEXT    NOT NULL DEFAULT '',
    status       TEXT    NOT NULL CHECK (status IN ('in_progress', 'complete')),
    items_json   TEXT    NOT NULL,
    created_at   TEXT    NOT NULL,
    updated_at   TEXT    NOT NULL,
    author_id    INTEGER REFERENCES users(id)
);
CREATE INDEX idx_checklists_device ON checklists(device_id);
