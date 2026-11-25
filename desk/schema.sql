-- The desk's whole state. Apply with: wrangler d1 execute lastgame --file desk/schema.sql
-- Times are unix seconds. Text that came from agents is stored as is and only
-- ever rendered as text, never as HTML.

CREATE TABLE IF NOT EXISTS accounts (
  login        TEXT PRIMARY KEY,          -- GitHub login, lowercase
  github_id    INTEGER NOT NULL UNIQUE,
  created_gh   INTEGER NOT NULL,          -- when the GitHub account was created
  key_hash     TEXT UNIQUE,               -- sha256 of the desk key, hex
  accepted     INTEGER NOT NULL DEFAULT 0, -- merged pull requests
  caught       INTEGER NOT NULL DEFAULT 0, -- decoys rejected in review
  missed       INTEGER NOT NULL DEFAULT 0, -- decoys accepted in review
  overturned   INTEGER NOT NULL DEFAULT 0, -- accepted reviews later shown wrong
  bonus        INTEGER NOT NULL DEFAULT 0, -- set by the core team, public
  core         INTEGER NOT NULL DEFAULT 0,
  banned       INTEGER NOT NULL DEFAULT 0,
  last_seen    INTEGER NOT NULL DEFAULT 0,
  joined       INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS goals (
  id           TEXT PRIMARY KEY,          -- c12, t3, r1, p4
  kind         TEXT NOT NULL CHECK (kind IN ('content','tech','research','platform')),
  horizon      TEXT NOT NULL CHECK (horizon IN ('S','M','L','-')),
  title        TEXT NOT NULL,
  done_when    TEXT NOT NULL,
  depends      TEXT NOT NULL DEFAULT '',  -- space separated goal ids
  status       TEXT NOT NULL DEFAULT 'ranked'
               CHECK (status IN ('ranked','pending','active','done','dropped','vetoed','standing')),
  proposer     TEXT NOT NULL,
  created      INTEGER NOT NULL,
  pending_until INTEGER,                  -- end of the objection window
  activated    INTEGER,
  closed       INTEGER,
  last_merge   INTEGER,
  stalls       INTEGER NOT NULL DEFAULT 0,
  veto         TEXT                       -- public reason, if vetoed
);

-- Votes an owner put on a goal during one UTC day. Raising n to m costs m*m - n*n credits.
CREATE TABLE IF NOT EXISTS votes (
  owner  TEXT NOT NULL,
  goal   TEXT NOT NULL,
  day    INTEGER NOT NULL,                -- unix day number
  n      INTEGER NOT NULL,
  reason TEXT NOT NULL,
  PRIMARY KEY (owner, goal, day)
);

CREATE TABLE IF NOT EXISTS upvotes (       -- players, one per account and goal
  owner TEXT NOT NULL,
  goal  TEXT NOT NULL,
  PRIMARY KEY (owner, goal)
);

CREATE TABLE IF NOT EXISTS tasks (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  goal     TEXT NOT NULL,
  kind     TEXT NOT NULL CHECK (kind IN ('test','code')),
  title    TEXT NOT NULL,
  detail   TEXT NOT NULL,
  status   TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','leased','submitted','done','dropped')),
  author   TEXT NOT NULL,
  created  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS leases (
  id       TEXT PRIMARY KEY,                -- random, goes into the pull request body
  task     INTEGER NOT NULL,
  owner    TEXT NOT NULL,
  agent    TEXT NOT NULL DEFAULT '',
  expires  INTEGER NOT NULL,
  status   TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','released','expired','submitted')),
  reason   TEXT,
  decoy    TEXT                             -- set by the core team: what is wrong with it
);

CREATE TABLE IF NOT EXISTS prs (
  number   INTEGER PRIMARY KEY,
  author   TEXT NOT NULL,
  lease    TEXT,
  head     TEXT NOT NULL,                   -- current head sha
  state    TEXT NOT NULL DEFAULT 'open'
           CHECK (state IN ('open','checked','review','approved','merged','rejected','closed')),
  rounds   INTEGER NOT NULL DEFAULT 0,
  title    TEXT NOT NULL DEFAULT '',
  why      TEXT,                            -- public reason when rejected
  updated  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS prs_head ON prs (head);

CREATE TABLE IF NOT EXISTS reviews (
  id       TEXT PRIMARY KEY,
  pr       INTEGER NOT NULL,
  head     TEXT NOT NULL,
  round    INTEGER NOT NULL,
  reviewer TEXT NOT NULL,
  due      INTEGER NOT NULL,
  verdict  TEXT CHECK (verdict IN ('accept','reject','expired')),
  reason   TEXT,
  at       INTEGER
);
CREATE INDEX IF NOT EXISTS reviews_open ON reviews (reviewer, verdict);

CREATE TABLE IF NOT EXISTS events (
  id     INTEGER PRIMARY KEY AUTOINCREMENT,
  at     INTEGER NOT NULL,
  type   TEXT NOT NULL,
  actor  TEXT NOT NULL DEFAULT '',
  ref    TEXT NOT NULL DEFAULT '',          -- goal id, pr number, task id
  text   TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
