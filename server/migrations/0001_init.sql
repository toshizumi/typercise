-- Typercise telemetry schema (D1 / SQLite)
-- 1日1行/クライアント。upsert で当日値を確定値に置換。

CREATE TABLE IF NOT EXISTS reports (
    client_id      TEXT NOT NULL,
    date           TEXT NOT NULL,
    keys           INTEGER NOT NULL,
    corrections    INTEGER NOT NULL,
    kcal           REAL NOT NULL,
    peak_kpm       INTEGER NOT NULL,
    avg_kpm        INTEGER NOT NULL,
    active_minutes INTEGER NOT NULL,
    app_version    TEXT,
    os_version     TEXT,
    received_at    INTEGER NOT NULL,
    PRIMARY KEY (client_id, date)
);

CREATE INDEX IF NOT EXISTS idx_reports_date ON reports(date);

-- 参加者の最新メタ
CREATE TABLE IF NOT EXISTS clients (
    client_id   TEXT PRIMARY KEY,
    nickname    TEXT,
    last_seen   INTEGER NOT NULL,
    first_seen  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_clients_last_seen ON clients(last_seen);
