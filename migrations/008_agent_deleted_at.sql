-- Keep agent identities and their match references after owner deletion.
ALTER TABLE agents ADD COLUMN deleted_at timestamptz;
