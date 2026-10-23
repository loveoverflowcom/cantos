-- Version 18: caller-owned adaptation contexts and immutable submission/acceptance receipts.
-- Time is INTEGER microseconds, with UTC formatting only at the checked adapter boundary.
CREATE TABLE adaptation_runs (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, owner_id TEXT NOT NULL REFERENCES actors(id), operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
 source_id TEXT NOT NULL, script_id TEXT NOT NULL CHECK(coalesce(script_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), frozen_input TEXT NOT NULL CHECK(json_valid(frozen_input) AND json_type(frozen_input)='object' AND octet_length(frozen_input) BETWEEN 2 AND 6291456), input_digest TEXT NOT NULL CHECK(coalesce(input_digest REGEXP '^[a-f0-9]{64}$',0)),
 status TEXT NOT NULL CHECK(status IN ('awaiting_proposal','succeeded','cancelled','accepted')),
 input_version TEXT NOT NULL CHECK(input_version='c1'),
 recorded_at INTEGER NOT NULL CHECK(recorded_at>=0), updated_at INTEGER NOT NULL CHECK(updated_at>=recorded_at),
 UNIQUE(id,owner_id), UNIQUE(owner_id,operation_id),
 FOREIGN KEY(owner_id,source_id) REFERENCES source_records(owner_id,id)
) STRICT;
CREATE TABLE adaptation_proposals (
 run_id TEXT PRIMARY KEY NOT NULL REFERENCES adaptation_runs(id), proposal TEXT NOT NULL CHECK(json_valid(proposal) AND json_type(proposal)='object' AND octet_length(proposal) BETWEEN 2 AND 4194304), digest TEXT NOT NULL CHECK(coalesce(digest REGEXP '^[a-f0-9]{64}$',0))
) STRICT;
CREATE TABLE adaptation_acceptances (
 run_id TEXT PRIMARY KEY NOT NULL REFERENCES adaptation_runs(id), owner_id TEXT NOT NULL REFERENCES actors(id),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), request TEXT NOT NULL CHECK(json_valid(request) AND json_type(request)='object' AND octet_length(request) BETWEEN 2 AND 4194304), script_id TEXT NOT NULL CHECK(coalesce(script_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), revision INTEGER NOT NULL CHECK(revision>0),
 UNIQUE(owner_id,operation_id), FOREIGN KEY(script_id,revision) REFERENCES script_revisions(script_id,revision)
) STRICT;
CREATE TABLE adaptation_cancellations (
 owner_id TEXT NOT NULL REFERENCES actors(id), operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), run_id TEXT NOT NULL REFERENCES adaptation_runs(id),
 recorded_at INTEGER NOT NULL CHECK(recorded_at>=0), PRIMARY KEY(owner_id,operation_id)
) STRICT;
CREATE TABLE adaptation_submissions (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, run_id TEXT NOT NULL REFERENCES adaptation_runs(id), owner_id TEXT NOT NULL REFERENCES actors(id),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), submission_number INTEGER NOT NULL CHECK(submission_number>0),
 request TEXT NOT NULL CHECK(json_valid(request) AND json_type(request)='object' AND octet_length(request) BETWEEN 2 AND 2097152), receipt TEXT NOT NULL CHECK(json_valid(receipt) AND json_type(receipt)='object' AND octet_length(receipt) BETWEEN 2 AND 4194304), receipt_digest TEXT NOT NULL CHECK(coalesce(receipt_digest REGEXP '^[a-f0-9]{64}$',0)), submitted_at INTEGER NOT NULL CHECK(submitted_at>=0),
 UNIQUE(owner_id,operation_id), UNIQUE(run_id,submission_number),
 FOREIGN KEY(run_id,owner_id) REFERENCES adaptation_runs(id,owner_id)
) STRICT;
CREATE TRIGGER adaptation_runs_guard BEFORE UPDATE ON adaptation_runs
WHEN NEW.id<>OLD.id OR NEW.owner_id<>OLD.owner_id OR NEW.operation_id<>OLD.operation_id
 OR NEW.source_id<>OLD.source_id OR NEW.script_id<>OLD.script_id OR NEW.frozen_input<>OLD.frozen_input
 OR NEW.input_digest<>OLD.input_digest OR NEW.recorded_at<>OLD.recorded_at OR NEW.input_version<>OLD.input_version
 OR NOT ((OLD.status='awaiting_proposal' AND NEW.status IN ('succeeded','cancelled'))
 OR (OLD.status='succeeded' AND NEW.status IN ('accepted','cancelled')))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_transition'); END;
CREATE TRIGGER adaptation_runs_no_delete BEFORE DELETE ON adaptation_runs
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_runs_no_replace BEFORE INSERT ON adaptation_runs
WHEN EXISTS(SELECT 1 FROM adaptation_runs WHERE (id=NEW.id) OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_proposals_no_update BEFORE UPDATE ON adaptation_proposals
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_proposals_no_delete BEFORE DELETE ON adaptation_proposals
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_proposals_no_replace BEFORE INSERT ON adaptation_proposals
WHEN EXISTS(SELECT 1 FROM adaptation_proposals WHERE (run_id=NEW.run_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_acceptances_no_update BEFORE UPDATE ON adaptation_acceptances
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_acceptances_no_delete BEFORE DELETE ON adaptation_acceptances
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_acceptances_no_replace BEFORE INSERT ON adaptation_acceptances
WHEN EXISTS(SELECT 1 FROM adaptation_acceptances WHERE (run_id=NEW.run_id) OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_cancellations_no_update BEFORE UPDATE ON adaptation_cancellations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_cancellations_no_delete BEFORE DELETE ON adaptation_cancellations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_cancellations_no_replace BEFORE INSERT ON adaptation_cancellations
WHEN EXISTS(SELECT 1 FROM adaptation_cancellations WHERE (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_submissions_no_update BEFORE UPDATE ON adaptation_submissions
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_submissions_no_delete BEFORE DELETE ON adaptation_submissions
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
CREATE TRIGGER adaptation_submissions_no_replace BEFORE INSERT ON adaptation_submissions
WHEN EXISTS(SELECT 1 FROM adaptation_submissions WHERE (id=NEW.id) OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id) OR (run_id=NEW.run_id AND submission_number=NEW.submission_number))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.adaptation_receipt'); END;
