-- AndroidMasters schema v1.
-- Principles (SKILL.md): no passwords of device owners, no MFA codes or tokens are ever stored;
-- device serials are stored encrypted plus a keyed fingerprint; the audit log is append-only.

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT    NOT NULL UNIQUE,
    display_name    TEXT    NOT NULL,
    role            TEXT    NOT NULL CHECK (role IN ('admin', 'technician')),
    password_hash   TEXT    NOT NULL,          -- Argon2id PHC string of the technician's app password
    active          INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    failed_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until    TEXT,
    created_at      TEXT    NOT NULL,
    last_login      TEXT
);

CREATE TABLE customers (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT NOT NULL,
    contact      TEXT NOT NULL DEFAULT '',     -- optional phone/e-mail given by the customer
    notes        TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL
);

CREATE TABLE devices (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    fingerprint      TEXT NOT NULL UNIQUE,     -- HMAC-SHA256 of the serial (lookup without clear text)
    serial_masked    TEXT NOT NULL,
    serial_encrypted TEXT NOT NULL,            -- ChaCha20-Poly1305
    manufacturer     TEXT NOT NULL DEFAULT '',
    brand            TEXT NOT NULL DEFAULT '',
    model            TEXT NOT NULL DEFAULT '',
    codename         TEXT NOT NULL DEFAULT '',
    android_version  TEXT NOT NULL DEFAULT '',
    api_level        INTEGER,
    security_patch   TEXT NOT NULL DEFAULT '',
    first_seen       TEXT NOT NULL,
    last_seen        TEXT NOT NULL
);

CREATE TABLE service_orders (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    number           TEXT    NOT NULL UNIQUE,
    customer_id      INTEGER NOT NULL REFERENCES customers(id),
    device_id        INTEGER REFERENCES devices(id),
    request_type     TEXT    NOT NULL CHECK (request_type IN
                     ('diagnose', 'accounts', 'frp', 'reset', 'transfer', 'maintenance')),
    reported_problem TEXT    NOT NULL DEFAULT '',
    status           TEXT    NOT NULL CHECK (status IN
                     ('open', 'in_progress', 'pending_verification', 'completed', 'delivered', 'cancelled')),
    consent_status   TEXT    NOT NULL DEFAULT 'pending' CHECK (consent_status IN ('pending', 'granted', 'denied', 'revoked')),
    consent_by       TEXT    NOT NULL DEFAULT '',
    consent_at       TEXT,
    ownership_method TEXT    NOT NULL DEFAULT '',
    technician_id    INTEGER REFERENCES users(id),
    received_at      TEXT    NOT NULL,
    delivered_at     TEXT,
    created_at       TEXT    NOT NULL,
    updated_at       TEXT    NOT NULL,
    notes            TEXT    NOT NULL DEFAULT ''
);
CREATE INDEX idx_orders_customer ON service_orders(customer_id);
CREATE INDEX idx_orders_device ON service_orders(device_id);
CREATE INDEX idx_orders_status ON service_orders(status);

-- Every entry says what it is: a diagnosis, an instruction for the owner, an executed action or a verification.
CREATE TABLE order_entries (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id          INTEGER NOT NULL REFERENCES service_orders(id),
    created_at        TEXT    NOT NULL,
    kind              TEXT    NOT NULL CHECK (kind IN
                      ('diagnosis', 'instruction', 'action', 'verification', 'consent', 'incident', 'note')),
    title             TEXT    NOT NULL,
    detail            TEXT    NOT NULL DEFAULT '',
    status            TEXT    NOT NULL CHECK (status IN ('ok', 'failed', 'pending_verification', 'info')),
    author_id         INTEGER REFERENCES users(id),
    operation_id      INTEGER,
    verifies_entry_id INTEGER REFERENCES order_entries(id),
    -- An instruction is guidance; it can never be recorded as executed or successful.
    CHECK (kind <> 'instruction' OR status IN ('info', 'pending_verification'))
);
CREATE INDEX idx_entries_order ON order_entries(order_id);

CREATE TABLE operations (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at    TEXT NOT NULL,
    finished_at   TEXT,
    duration_ms   INTEGER,
    device_id     INTEGER REFERENCES devices(id),
    order_id      INTEGER REFERENCES service_orders(id),
    user_id       INTEGER REFERENCES users(id),
    operation     TEXT NOT NULL,
    category      TEXT NOT NULL,
    result        TEXT NOT NULL CHECK (result IN ('pending', 'success', 'failed', 'rejected', 'cancelled', 'pending_verification')),
    error_message TEXT NOT NULL DEFAULT '',
    details_json  TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX idx_operations_started ON operations(started_at);
CREATE INDEX idx_operations_device ON operations(device_id);

CREATE TABLE frp_assessments (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id          INTEGER REFERENCES service_orders(id),
    device_id         INTEGER REFERENCES devices(id),
    created_at        TEXT NOT NULL,
    observations_json TEXT NOT NULL,
    outcome           TEXT NOT NULL,
    author_id         INTEGER REFERENCES users(id)
);

CREATE TABLE procedure_reviews (
    procedure_id TEXT    PRIMARY KEY,
    version      INTEGER NOT NULL,
    reviewed_by  INTEGER NOT NULL REFERENCES users(id),
    reviewed_at  TEXT    NOT NULL,
    note         TEXT    NOT NULL DEFAULT ''
);

CREATE TABLE attachments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id    INTEGER NOT NULL REFERENCES service_orders(id),
    created_at  TEXT    NOT NULL,
    file_name   TEXT    NOT NULL,
    stored_path TEXT    NOT NULL,
    sha256      TEXT    NOT NULL,
    size_bytes  INTEGER NOT NULL,
    description TEXT    NOT NULL DEFAULT '',
    author_id   INTEGER REFERENCES users(id)
);

CREATE TABLE audit_log (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at         TEXT NOT NULL,
    event              TEXT NOT NULL,
    severity           TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    user_id            INTEGER REFERENCES users(id),
    device_fingerprint TEXT NOT NULL DEFAULT '',
    details_json       TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX idx_audit_created ON audit_log(created_at);

-- The audit log is append-only.
CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
