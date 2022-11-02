/**
 * This plugin implementation for PostgreSQL requires the following tables
 */

CREATE TYPE "SlotStatus" AS ENUM (
    'Rooted',
    'Confirmed',
    'Processed'
);

CREATE TABLE monitoring (
    name TEXT PRIMARY KEY,
    last_update TIMESTAMP WITH TIME ZONE,
    last_slot_write TIMESTAMP WITH TIME ZONE,
    last_account_write_write TIMESTAMP WITH TIME ZONE,
    slot_queue BIGINT,
    account_write_queue BIGINT
);

-- The table storing account writes, keeping only the newest write_version per slot
CREATE TABLE account_write (
    pubkey VARCHAR NOT NULL,
    slot BIGINT NOT NULL,
    write_version BIGINT NOT NULL,
    owner VARCHAR NOT NULL,
    is_selected BOOL NOT NULL,
    lamports BIGINT NOT NULL,
    executable BOOL NOT NULL,
    rent_epoch BIGINT NOT NULL,
    data BYTEA,
    PRIMARY KEY (pubkey, slot, owner)
)
    PARTITION BY list (owner);

CREATE TABLE account_write_tokenkeg PARTITION OF account_write
    FOR VALUES IN ('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');

CREATE TABLE account_write_zeta PARTITION OF account_write
    FOR VALUES IN ('ZETAxsqBRek56DhiGXrn75yj2NHU3aYUnxvHXpkf3aD');

CREATE TABLE account_write_stake PARTITION OF account_write
    FOR VALUES IN ('Stake11111111111111111111111111111111111111');

CREATE TABLE account_write_default PARTITION of account_write DEFAULT;

-- Substring index on offset 33 with length 32 (bytes) == Owner field for Tokenkeg
create index owner_idx on account_write_tokenkeg (substring(data,33,32));

-- Substring index on offset 1 with length 32 (bytes) == Mint field for Tokenkeg
create index mint_idx on account_write_tokenkeg (substring(data,1,32));


CREATE INDEX account_write_owner_slot on account_write(owner, slot DESC);
CREATE INDEX account_write_slot_owner on account_write(slot, owner);

-- The table storing slot information
CREATE TABLE slot (
    slot BIGINT PRIMARY KEY,
    parent BIGINT,
    status "SlotStatus" NOT NULL
);
CREATE INDEX ON slot (parent);
CREATE INDEX ON slot (status);

-- Keep all slots for debugging purposes
CREATE TABLE slot_history (
    slot BIGINT PRIMARY KEY,
    parent BIGINT,
    status "SlotStatus" NOT NULL
);
CREATE INDEX ON slot_history (parent);
CREATE INDEX ON slot_history (status);