pub(crate) const SCHEMA_VERSION: i64 = 10;

pub(crate) const CALENDAR_SCHEMA: &str = "
    ALTER TABLE worlds ADD COLUMN calendar_json TEXT
        CHECK (calendar_json IS NULL OR json_valid(calendar_json));
";

pub(crate) const VARIANT_SCHEMA: &str = "
    CREATE TABLE variants (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        name TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(name)) > 0),
        head_revision_id TEXT NOT NULL,
        archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
        created_from_revision_id TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, name),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (head_revision_id) REFERENCES revisions(id),
        FOREIGN KEY (created_from_revision_id) REFERENCES revisions(id)
    );

    CREATE TABLE revision_snapshots (
        revision_id TEXT PRIMARY KEY,
        snapshot_json TEXT NOT NULL,
        FOREIGN KEY (revision_id) REFERENCES revisions(id) ON DELETE CASCADE
    );

    ALTER TABLE worlds ADD COLUMN active_variant_id TEXT;
    ALTER TABLE revisions ADD COLUMN variant_id TEXT;
    ALTER TABLE revisions ADD COLUMN source_revision_id TEXT;
    ALTER TABLE change_sets ADD COLUMN variant_id TEXT;
    ALTER TABLE import_batches ADD COLUMN variant_id TEXT;
    DROP INDEX revisions_linear_parent;
    CREATE UNIQUE INDEX revisions_variant_parent
        ON revisions (variant_id, parent_revision_id)
        WHERE parent_revision_id IS NOT NULL;
    CREATE INDEX revisions_variant_id ON revisions (variant_id, created_at_ms);
    CREATE INDEX change_sets_variant_id ON change_sets (variant_id, created_at_ms);
    CREATE INDEX import_batches_variant_id ON import_batches (variant_id, created_at_ms);
";

pub(crate) const VARIANT_INTEGRITY_SCHEMA: &str = "
    CREATE TRIGGER variants_head_world_insert
    BEFORE INSERT ON variants
    WHEN NOT EXISTS (
        SELECT 1 FROM revisions
        WHERE id = NEW.head_revision_id AND world_id = NEW.world_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'variant head must belong to its world');
    END;

    CREATE TRIGGER variants_head_world_update
    BEFORE UPDATE OF head_revision_id, world_id ON variants
    WHEN NOT EXISTS (
        SELECT 1 FROM revisions
        WHERE id = NEW.head_revision_id AND world_id = NEW.world_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'variant head must belong to its world');
    END;

    CREATE TRIGGER worlds_active_variant_update
    BEFORE UPDATE OF active_variant_id ON worlds
    WHEN NEW.active_variant_id IS NULL OR NOT EXISTS (
        SELECT 1 FROM variants
        WHERE id = NEW.active_variant_id AND world_id = NEW.id
    )
    BEGIN
        SELECT RAISE(ABORT, 'active variant must belong to its world');
    END;

    CREATE TRIGGER revisions_variant_insert
    BEFORE INSERT ON revisions
    WHEN NEW.variant_id IS NULL OR NOT EXISTS (
        SELECT 1 FROM variants
        WHERE id = NEW.variant_id AND world_id = NEW.world_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'revision variant must belong to its world');
    END;

    CREATE TRIGGER revisions_variant_update
    BEFORE UPDATE OF variant_id, world_id ON revisions
    WHEN NEW.variant_id IS NULL OR NOT EXISTS (
        SELECT 1 FROM variants
        WHERE id = NEW.variant_id AND world_id = NEW.world_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'revision variant must belong to its world');
    END;

    CREATE TRIGGER revisions_source_insert
    BEFORE INSERT ON revisions
    WHEN NEW.source_revision_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM revisions WHERE id = NEW.source_revision_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'merge source revision does not exist');
    END;

    CREATE TRIGGER revisions_source_update
    BEFORE UPDATE OF source_revision_id ON revisions
    WHEN NEW.source_revision_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM revisions WHERE id = NEW.source_revision_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'merge source revision does not exist');
    END;

    CREATE TRIGGER change_sets_variant_insert
    BEFORE INSERT ON change_sets
    WHEN NEW.variant_id IS NULL OR NOT EXISTS (
        SELECT 1 FROM variants
        WHERE id = NEW.variant_id AND world_id = NEW.world_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'change set variant must belong to its world');
    END;

    CREATE TRIGGER change_sets_variant_update
    BEFORE UPDATE OF variant_id, world_id ON change_sets
    WHEN NEW.variant_id IS NULL OR NOT EXISTS (
        SELECT 1 FROM variants
        WHERE id = NEW.variant_id AND world_id = NEW.world_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'change set variant must belong to its world');
    END;

    CREATE TRIGGER import_batches_variant_insert
    BEFORE INSERT ON import_batches
    WHEN NEW.variant_id IS NULL OR NOT EXISTS (
        SELECT 1 FROM variants
        WHERE id = NEW.variant_id AND world_id = NEW.world_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'import batch variant must belong to its world');
    END;

    CREATE TRIGGER import_batches_variant_update
    BEFORE UPDATE OF variant_id, world_id ON import_batches
    WHEN NEW.variant_id IS NULL OR NOT EXISTS (
        SELECT 1 FROM variants
        WHERE id = NEW.variant_id AND world_id = NEW.world_id
    )
    BEGIN
        SELECT RAISE(ABORT, 'import batch variant must belong to its world');
    END;
";

pub(crate) const LORE_IMPORT_SCHEMA: &str = "
    CREATE TABLE import_batches (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        target_revision_id TEXT NOT NULL,
        status TEXT NOT NULL CHECK (
            status IN ('ready', 'extracting', 'reviewing', 'cancelled')
        ),
        created_at_ms INTEGER NOT NULL,
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (target_revision_id) REFERENCES revisions(id)
    );

    CREATE TABLE import_sources (
        id TEXT PRIMARY KEY,
        batch_id TEXT NOT NULL,
        source_path TEXT NOT NULL,
        file_name TEXT NOT NULL,
        format TEXT NOT NULL CHECK (format IN ('markdown', 'text')),
        content_hash TEXT NOT NULL,
        size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
        content_utf8 TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('ready', 'replaced')),
        UNIQUE (batch_id, source_path),
        FOREIGN KEY (batch_id) REFERENCES import_batches(id) ON DELETE CASCADE
    );

    CREATE TABLE import_chunks (
        id TEXT PRIMARY KEY,
        source_id TEXT NOT NULL,
        source_hash TEXT NOT NULL,
        ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
        byte_start INTEGER NOT NULL CHECK (byte_start >= 0),
        byte_end INTEGER NOT NULL CHECK (byte_end >= byte_start),
        line_start INTEGER NOT NULL CHECK (line_start >= 1),
        line_end INTEGER NOT NULL CHECK (line_end >= line_start),
        heading TEXT,
        content_utf8 TEXT NOT NULL,
        UNIQUE (source_id, source_hash, ordinal),
        FOREIGN KEY (source_id) REFERENCES import_sources(id) ON DELETE CASCADE
    );

    CREATE TABLE import_candidates (
        id TEXT PRIMARY KEY,
        batch_id TEXT NOT NULL,
        source_id TEXT NOT NULL,
        source_hash TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (
            kind IN ('entity', 'relation', 'event', 'claim', 'rule')
        ),
        payload_json TEXT NOT NULL,
        citations_json TEXT NOT NULL,
        technical_confidence REAL NOT NULL CHECK (
            technical_confidence >= 0.0 AND technical_confidence <= 1.0
        ),
        status TEXT NOT NULL CHECK (status IN ('pending', 'selected', 'rejected')),
        identity_decision TEXT CHECK (
            identity_decision IS NULL OR identity_decision IN ('exact', 'ambiguous', 'new')
        ),
        canonical_uri TEXT,
        contradiction_key TEXT,
        FOREIGN KEY (batch_id) REFERENCES import_batches(id) ON DELETE CASCADE,
        FOREIGN KEY (source_id) REFERENCES import_sources(id) ON DELETE CASCADE
    );

    CREATE INDEX import_batches_world_id ON import_batches (world_id, created_at_ms);
    CREATE INDEX import_sources_batch_id ON import_sources (batch_id, id);
    CREATE INDEX import_chunks_source_id ON import_chunks (source_id, source_hash, ordinal);
    CREATE INDEX import_candidates_batch_id ON import_candidates (batch_id, status, id);
";

pub(crate) const INITIAL_SCHEMA: &str = "
    CREATE TABLE schema_migrations (
        version INTEGER PRIMARY KEY,
        applied_at_ms INTEGER NOT NULL
    );
    CREATE TABLE worlds (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        premise_md TEXT NOT NULL,
        epoch_label TEXT NOT NULL,
        current_revision TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        FOREIGN KEY (current_revision) REFERENCES revisions(id)
            DEFERRABLE INITIALLY DEFERRED
    );
    CREATE TABLE revisions (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        parent_revision_id TEXT,
        created_at_ms INTEGER NOT NULL,
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
            DEFERRABLE INITIALLY DEFERRED,
        FOREIGN KEY (parent_revision_id) REFERENCES revisions(id)
    );
";

pub(crate) const CANON_SCHEMA: &str = "
    CREATE TABLE rules (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (
            kind IN ('constitutive', 'generative', 'institutional', 'authorial')
        ),
        statement_md TEXT NOT NULL,
        scope TEXT NOT NULL,
        severity TEXT NOT NULL CHECK (severity IN ('advisory', 'hard')),
        source TEXT,
        validator_kind TEXT CHECK (
            validator_kind IS NULL OR validator_kind = 'no_resurrection'
        ),
        parameters_json TEXT NOT NULL,
        version INTEGER NOT NULL CHECK (version > 0),
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, id),
        CHECK (severity <> 'hard' OR validator_kind IS NOT NULL),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
    );

    CREATE TABLE entities (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (
            kind IN ('person', 'place', 'faction', 'culture', 'resource', 'concept')
        ),
        name TEXT NOT NULL CHECK (length(trim(name)) > 0),
        slug TEXT NOT NULL CHECK (length(trim(slug)) > 0),
        summary TEXT NOT NULL,
        body_md TEXT NOT NULL,
        attributes_json TEXT NOT NULL,
        version INTEGER NOT NULL CHECK (version > 0),
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, id),
        UNIQUE (world_id, slug),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
    );

    CREATE TABLE entity_aliases (
        world_id TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        alias TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(alias)) > 0),
        UNIQUE (entity_id, alias),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, entity_id) REFERENCES entities(world_id, id)
            ON DELETE CASCADE
    );

    CREATE TABLE relations (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        source_entity_id TEXT NOT NULL,
        target_entity_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (length(trim(kind)) > 0),
        direction TEXT NOT NULL CHECK (direction IN ('directed', 'undirected')),
        valid_from_tick INTEGER,
        valid_to_tick INTEGER,
        certainty TEXT NOT NULL CHECK (
            certainty IN ('certain', 'approximate', 'uncertain', 'approximate_uncertain')
        ),
        source_reference TEXT,
        metadata_json TEXT NOT NULL,
        version INTEGER NOT NULL CHECK (version > 0),
        UNIQUE (world_id, id),
        CHECK (
            valid_from_tick IS NULL
            OR valid_to_tick IS NULL
            OR valid_from_tick <= valid_to_tick
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, source_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, target_entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE goals (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        holder_entity_id TEXT NOT NULL,
        desired_state_md TEXT NOT NULL CHECK (length(trim(desired_state_md)) > 0),
        priority INTEGER NOT NULL,
        status TEXT NOT NULL CHECK (
            status IN ('active', 'achieved', 'abandoned', 'frustrated')
        ),
        valid_from_tick INTEGER,
        valid_to_tick INTEGER,
        visibility TEXT NOT NULL CHECK (visibility IN ('public', 'secret')),
        source TEXT,
        version INTEGER NOT NULL CHECK (version > 0),
        UNIQUE (world_id, id),
        CHECK (
            valid_from_tick IS NULL
            OR valid_to_tick IS NULL
            OR valid_from_tick <= valid_to_tick
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, holder_entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE events (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (length(trim(kind)) > 0),
        summary TEXT NOT NULL,
        body_md TEXT NOT NULL,
        time_kind TEXT NOT NULL CHECK (
            time_kind IN ('unknown', 'instant', 'interval', 'ongoing')
        ),
        start_tick INTEGER,
        end_tick INTEGER,
        time_precision TEXT NOT NULL CHECK (
            time_precision IN ('exact', 'day', 'month', 'year', 'era', 'unknown')
        ),
        certainty TEXT NOT NULL CHECK (
            certainty IN ('certain', 'approximate', 'uncertain', 'approximate_uncertain')
        ),
        location_entity_id TEXT,
        version INTEGER NOT NULL CHECK (version > 0),
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, id),
        CHECK (
            (time_kind = 'unknown' AND start_tick IS NULL AND end_tick IS NULL)
            OR (time_kind = 'instant' AND start_tick IS NOT NULL AND end_tick IS NULL)
            OR (
                time_kind = 'interval'
                AND start_tick IS NOT NULL
                AND end_tick IS NOT NULL
                AND start_tick <= end_tick
            )
            OR (time_kind = 'ongoing' AND start_tick IS NOT NULL AND end_tick IS NULL)
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, location_entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE event_participants (
        world_id TEXT NOT NULL,
        event_id TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        role TEXT NOT NULL CHECK (length(trim(role)) > 0),
        ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
        UNIQUE (event_id, ordinal),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, event_id) REFERENCES events(world_id, id)
            ON DELETE CASCADE,
        FOREIGN KEY (world_id, entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE event_links (
        world_id TEXT NOT NULL,
        source_event_id TEXT NOT NULL,
        target_event_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (
            kind IN ('enables', 'causes', 'motivates', 'prevents', 'terminates', 'reveals')
        ),
        UNIQUE (source_event_id, target_event_id, kind),
        CHECK (source_event_id <> target_event_id),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, source_event_id) REFERENCES events(world_id, id)
            ON DELETE CASCADE,
        FOREIGN KEY (world_id, target_event_id) REFERENCES events(world_id, id)
            ON DELETE CASCADE
    );

    CREATE TABLE event_goals (
        world_id TEXT NOT NULL,
        event_id TEXT NOT NULL,
        goal_id TEXT NOT NULL,
        UNIQUE (event_id, goal_id),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, event_id) REFERENCES events(world_id, id)
            ON DELETE CASCADE,
        FOREIGN KEY (world_id, goal_id) REFERENCES goals(world_id, id)
    );

    CREATE TABLE documents (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        title TEXT NOT NULL,
        kind TEXT NOT NULL,
        author_entity_id TEXT,
        perspective_entity_id TEXT,
        canon_status TEXT NOT NULL CHECK (
            canon_status IN ('canonical', 'non_canonical')
        ),
        body_md TEXT NOT NULL,
        version INTEGER NOT NULL CHECK (version > 0),
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, id),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, author_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, perspective_entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE claims (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        subject_entity_id TEXT NOT NULL,
        content_md TEXT NOT NULL,
        predicate_key TEXT,
        object_kind TEXT CHECK (object_kind IS NULL OR object_kind IN ('entity', 'scalar')),
        object_entity_id TEXT,
        object_scalar TEXT,
        polarity TEXT NOT NULL CHECK (polarity IN ('positive', 'negative')),
        authentication TEXT NOT NULL CHECK (
            authentication IN ('canonical', 'attributed', 'disputed')
        ),
        holder_entity_id TEXT,
        modality TEXT CHECK (
            modality IS NULL
            OR modality IN ('assertion', 'belief', 'hypothesis', 'counterfactual')
        ),
        register TEXT,
        epistemic_basis TEXT,
        source TEXT,
        source_document_id TEXT,
        source_claim_id TEXT,
        holder_confidence REAL CHECK (
            holder_confidence IS NULL
            OR (holder_confidence >= 0.0 AND holder_confidence <= 1.0)
        ),
        valid_from_tick INTEGER,
        valid_to_tick INTEGER,
        registered_revision_id TEXT NOT NULL,
        superseded_revision_id TEXT,
        version INTEGER NOT NULL CHECK (version > 0),
        UNIQUE (world_id, id),
        CHECK (
            (predicate_key IS NULL AND object_kind IS NULL)
            OR (predicate_key IS NOT NULL AND length(trim(predicate_key)) > 0
                AND object_kind IS NOT NULL)
        ),
        CHECK (
            (object_kind IS NULL AND object_entity_id IS NULL AND object_scalar IS NULL)
            OR (object_kind = 'entity' AND object_entity_id IS NOT NULL
                AND object_scalar IS NULL)
            OR (object_kind = 'scalar' AND object_entity_id IS NULL
                AND object_scalar IS NOT NULL)
        ),
        CHECK (
            authentication <> 'canonical'
            OR (holder_entity_id IS NULL AND modality IS NULL)
        ),
        CHECK (
            authentication <> 'attributed'
            OR (holder_entity_id IS NOT NULL AND modality IS NOT NULL)
        ),
        CHECK (source_claim_id IS NULL OR source_claim_id <> id),
        CHECK (
            valid_from_tick IS NULL
            OR valid_to_tick IS NULL
            OR valid_from_tick <= valid_to_tick
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, subject_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, object_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, holder_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, source_document_id) REFERENCES documents(world_id, id),
        FOREIGN KEY (world_id, source_claim_id) REFERENCES claims(world_id, id),
        FOREIGN KEY (registered_revision_id) REFERENCES revisions(id),
        FOREIGN KEY (superseded_revision_id) REFERENCES revisions(id)
    );

    CREATE TABLE content_references (
        world_id TEXT NOT NULL,
        source_type TEXT NOT NULL CHECK (
            source_type IN ('entity', 'relation', 'event', 'claim', 'rule', 'goal', 'document')
        ),
        source_id TEXT NOT NULL,
        target_type TEXT NOT NULL CHECK (
            target_type IN ('entity', 'relation', 'event', 'claim', 'rule', 'goal', 'document')
        ),
        target_id TEXT NOT NULL,
        ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
        UNIQUE (world_id, source_type, source_id, ordinal),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
    );

    CREATE UNIQUE INDEX relations_exact_unique ON relations (
        world_id,
        kind,
        direction,
        CASE
            WHEN direction = 'undirected' AND source_entity_id > target_entity_id
                THEN target_entity_id
            ELSE source_entity_id
        END,
        CASE
            WHEN direction = 'undirected' AND source_entity_id > target_entity_id
                THEN source_entity_id
            ELSE target_entity_id
        END,
        valid_from_tick IS NULL,
        ifnull(valid_from_tick, 0),
        valid_to_tick IS NULL,
        ifnull(valid_to_tick, 0),
        certainty,
        source_reference IS NULL,
        ifnull(source_reference, ''),
        metadata_json
    );

    CREATE INDEX rules_world_id ON rules (world_id);
    CREATE INDEX entities_world_id ON entities (world_id);
    CREATE INDEX entity_aliases_world_id ON entity_aliases (world_id, entity_id);
    CREATE INDEX relations_world_id ON relations (world_id);
    CREATE INDEX relations_source_id ON relations (world_id, source_entity_id);
    CREATE INDEX relations_target_id ON relations (world_id, target_entity_id);
    CREATE INDEX relations_time ON relations (world_id, valid_from_tick, valid_to_tick);
    CREATE INDEX goals_world_id ON goals (world_id);
    CREATE INDEX goals_holder_id ON goals (world_id, holder_entity_id);
    CREATE INDEX goals_time ON goals (world_id, valid_from_tick, valid_to_tick);
    CREATE INDEX events_world_id ON events (world_id);
    CREATE INDEX events_location_id ON events (world_id, location_entity_id);
    CREATE INDEX events_time ON events (world_id, start_tick, end_tick);
    CREATE INDEX event_participants_world_id ON event_participants (world_id, event_id);
    CREATE INDEX event_participants_entity_id ON event_participants (world_id, entity_id);
    CREATE INDEX event_links_world_id ON event_links (world_id, source_event_id);
    CREATE INDEX event_links_target_id ON event_links (world_id, target_event_id);
    CREATE INDEX event_goals_world_id ON event_goals (world_id, event_id);
    CREATE INDEX event_goals_goal_id ON event_goals (world_id, goal_id);
    CREATE INDEX documents_world_id ON documents (world_id);
    CREATE INDEX documents_author_id ON documents (world_id, author_entity_id);
    CREATE INDEX documents_perspective_id ON documents (world_id, perspective_entity_id);
    CREATE INDEX claims_world_id ON claims (world_id);
    CREATE INDEX claims_subject_id ON claims (world_id, subject_entity_id);
    CREATE INDEX claims_object_id ON claims (world_id, object_entity_id);
    CREATE INDEX claims_holder_id ON claims (world_id, holder_entity_id);
    CREATE INDEX claims_source_document_id ON claims (world_id, source_document_id);
    CREATE INDEX claims_source_claim_id ON claims (world_id, source_claim_id);
    CREATE INDEX claims_registered_revision_id ON claims (registered_revision_id);
    CREATE INDEX claims_superseded_revision_id ON claims (superseded_revision_id);
    CREATE INDEX claims_time ON claims (world_id, valid_from_tick, valid_to_tick);
    CREATE INDEX content_references_source_id
        ON content_references (world_id, source_type, source_id);
    CREATE INDEX content_references_target_id
        ON content_references (world_id, target_type, target_id);
";

pub(crate) const CHANGE_SET_SCHEMA: &str = "
    CREATE TABLE change_sets (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('draft', 'committed')),
        base_revision_id TEXT NOT NULL,
        result_revision_id TEXT,
        objective TEXT NOT NULL CHECK (length(trim(objective)) > 0),
        source_refs_json TEXT NOT NULL,
        assumptions_json TEXT NOT NULL,
        deterministic_report_json TEXT,
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        CHECK (
            (kind = 'draft' AND result_revision_id IS NULL)
            OR (kind = 'committed' AND result_revision_id IS NOT NULL)
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (base_revision_id) REFERENCES revisions(id)
            DEFERRABLE INITIALLY DEFERRED,
        FOREIGN KEY (result_revision_id) REFERENCES revisions(id)
            DEFERRABLE INITIALLY DEFERRED,
        UNIQUE (result_revision_id)
    );

    CREATE TABLE change_operations (
        operation_id TEXT NOT NULL,
        change_set_id TEXT NOT NULL,
        ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
        kind TEXT NOT NULL,
        retcon TEXT NOT NULL CHECK (
            retcon IN ('additive', 'reinterpretive', 'replacement')
        ),
        expected_version INTEGER NOT NULL CHECK (expected_version >= 0),
        affected_refs_json TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        PRIMARY KEY (change_set_id, operation_id),
        UNIQUE (change_set_id, ordinal),
        FOREIGN KEY (change_set_id) REFERENCES change_sets(id) ON DELETE CASCADE
    );

    CREATE TABLE decision_points (
        id TEXT PRIMARY KEY,
        change_set_id TEXT NOT NULL,
        ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
        prompt TEXT NOT NULL CHECK (length(trim(prompt)) > 0),
        operation_ids_json TEXT NOT NULL,
        alternatives_json TEXT NOT NULL,
        replacement_target_ref TEXT,
        reason TEXT,
        resolved_alternative TEXT,
        UNIQUE (change_set_id, ordinal),
        FOREIGN KEY (change_set_id) REFERENCES change_sets(id) ON DELETE CASCADE
    );

    CREATE TABLE change_set_waivers (
        change_set_id TEXT NOT NULL,
        operation_id TEXT NOT NULL,
        issue_code TEXT NOT NULL CHECK (length(trim(issue_code)) > 0),
        rationale TEXT NOT NULL CHECK (length(trim(rationale)) > 0),
        created_at_ms INTEGER NOT NULL,
        PRIMARY KEY (change_set_id, operation_id, issue_code),
        FOREIGN KEY (change_set_id, operation_id)
            REFERENCES change_operations(change_set_id, operation_id)
            ON DELETE CASCADE
    );

    CREATE TABLE change_operation_audits (
        change_set_id TEXT NOT NULL,
        operation_id TEXT NOT NULL,
        decision TEXT NOT NULL CHECK (decision IN ('accept', 'edit', 'reject')),
        source TEXT NOT NULL CHECK (length(trim(source)) > 0),
        before_json TEXT,
        after_json TEXT,
        decided_at_ms INTEGER NOT NULL,
        PRIMARY KEY (change_set_id, operation_id),
        FOREIGN KEY (change_set_id, operation_id)
            REFERENCES change_operations(change_set_id, operation_id)
            ON DELETE CASCADE
    );

    CREATE INDEX change_sets_world_id ON change_sets (world_id, created_at_ms);
    CREATE INDEX change_sets_base_revision_id ON change_sets (base_revision_id);
    CREATE INDEX change_operations_change_set_id
        ON change_operations (change_set_id, ordinal);
    CREATE INDEX change_operations_operation_id ON change_operations (operation_id);
    CREATE INDEX decision_points_change_set_id
        ON decision_points (change_set_id, ordinal);
    CREATE INDEX change_set_waivers_change_set_id
        ON change_set_waivers (change_set_id, operation_id);
    CREATE INDEX change_operation_audits_change_set_id
        ON change_operation_audits (change_set_id, operation_id);
";

pub(crate) const REVISION_COMPLETION_SCHEMA: &str = "
    ALTER TABLE revisions ADD COLUMN author TEXT NOT NULL DEFAULT 'system'
        CHECK (length(trim(author)) > 0);
    ALTER TABLE revisions ADD COLUMN summary TEXT NOT NULL DEFAULT 'World created'
        CHECK (length(trim(summary)) > 0);
    ALTER TABLE revisions ADD COLUMN change_set_id TEXT;
    CREATE UNIQUE INDEX revisions_single_root ON revisions (world_id)
        WHERE parent_revision_id IS NULL;
    CREATE UNIQUE INDEX revisions_linear_parent ON revisions (parent_revision_id)
        WHERE parent_revision_id IS NOT NULL;
    CREATE UNIQUE INDEX revisions_change_set_id ON revisions (change_set_id)
        WHERE change_set_id IS NOT NULL;
    CREATE INDEX revisions_world_id ON revisions (world_id, created_at_ms);
";

pub(crate) const UNDO_SCHEMA: &str = "
    CREATE TABLE revision_undos (
        undo_revision_id TEXT PRIMARY KEY,
        undone_revision_id TEXT NOT NULL UNIQUE,
        FOREIGN KEY (undo_revision_id) REFERENCES revisions(id) ON DELETE CASCADE,
        FOREIGN KEY (undone_revision_id) REFERENCES revisions(id)
    );

    CREATE INDEX revision_undos_undone_revision_id
        ON revision_undos (undone_revision_id);
";
