use rusqlite::Connection;
use crate::error::Result;

struct Migration {
    version: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "001_initial",
        sql: "
            CREATE TABLE IF NOT EXISTS graph_nodes (
                name TEXT,
                entity_type TEXT NOT NULL,
                observations TEXT NOT NULL,
                PRIMARY KEY (name)
            );
            CREATE TABLE IF NOT EXISTS graph_edges (
                from_name TEXT NOT NULL,
                to_name TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                PRIMARY KEY (from_name, to_name, relation_type)
            );
            CREATE TABLE IF NOT EXISTS semantic_metadata (
                node_id TEXT,
                raw_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                timestamp TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0,
                PRIMARY KEY (node_id)
            );
            CREATE TABLE IF NOT EXISTS semantic_vector_mapping (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id TEXT UNIQUE NOT NULL
            );
            CREATE TABLE IF NOT EXISTS semantic_hnsw_index (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                index_data BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS episodic_logs (
                id TEXT PRIMARY KEY,
                task_description TEXT NOT NULL,
                execution_status TEXT NOT NULL,
                steps_taken TEXT NOT NULL,
                error_message TEXT,
                reflection TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS reflection_memory (
                id TEXT PRIMARY KEY,
                task_description TEXT NOT NULL,
                status TEXT NOT NULL,
                attempt_number INTEGER NOT NULL,
                steps_taken TEXT NOT NULL,
                error_encountered TEXT,
                root_cause TEXT,
                solution_applied TEXT,
                reflection TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tool_performance (
                tool_name TEXT NOT NULL,
                model_name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                average_latency REAL NOT NULL DEFAULT 0.0,
                last_used TEXT NOT NULL,
                PRIMARY KEY (tool_name, model_name, task_type)
            );
            CREATE TABLE IF NOT EXISTS code_elements (
                element_id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                element_type TEXT NOT NULL,
                name TEXT NOT NULL,
                signature TEXT NOT NULL,
                ast_json TEXT,
                parent_id TEXT,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS code_calls (
                caller_id TEXT NOT NULL,
                callee_id TEXT NOT NULL,
                call_site TEXT,
                PRIMARY KEY (caller_id, callee_id)
            );
            CREATE TABLE IF NOT EXISTS repository_evolution (
                file_path TEXT NOT NULL,
                version TEXT NOT NULL,
                commit_hash TEXT,
                author TEXT,
                change_type TEXT NOT NULL,
                summary_of_changes TEXT NOT NULL,
                bug_introduced INTEGER NOT NULL DEFAULT 0,
                bug_fixed INTEGER NOT NULL DEFAULT 0,
                timestamp TEXT NOT NULL,
                PRIMARY KEY (file_path, version)
            );
            CREATE TABLE IF NOT EXISTS codebase_signatures (
                id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                item_name TEXT NOT NULL,
                item_type TEXT NOT NULL,
                signature TEXT NOT NULL,
                dependencies TEXT
            );
            CREATE TABLE IF NOT EXISTS shared_agent_memory (
                memory_key TEXT,
                memory_value TEXT NOT NULL,
                source_agent TEXT NOT NULL,
                target_agents TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0,
                timestamp TEXT NOT NULL,
                PRIMARY KEY (memory_key)
            );
        ",
    },
    Migration {
        version: "002_fts5",
        sql: "
            CREATE VIRTUAL TABLE IF NOT EXISTS semantic_fts USING fts5(
                node_id UNINDEXED,
                raw_text
            );
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_ai AFTER INSERT ON semantic_metadata BEGIN
                INSERT INTO semantic_fts(node_id, raw_text) VALUES (new.node_id, new.raw_text);
            END;
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_ad AFTER DELETE ON semantic_metadata BEGIN
                DELETE FROM semantic_fts WHERE node_id = old.node_id;
            END;
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_au AFTER UPDATE OF raw_text ON semantic_metadata BEGIN
                UPDATE semantic_fts SET raw_text = new.raw_text WHERE node_id = new.node_id;
            END;
            INSERT INTO semantic_fts(node_id, raw_text)
            SELECT node_id, raw_text FROM semantic_metadata
            WHERE NOT EXISTS (SELECT 1 FROM semantic_fts WHERE semantic_fts.node_id = semantic_metadata.node_id);
        ",
    },
    Migration {
        version: "003_temporal",
        sql: "
            -- Temporal schema adjustments for graph_edges
            CREATE TABLE IF NOT EXISTS graph_edges_new (
                from_name TEXT NOT NULL,
                to_name TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                valid_until TEXT,
                superseded_by TEXT,
                confidence REAL NOT NULL DEFAULT 1.0,
                PRIMARY KEY (from_name, to_name, relation_type, valid_from)
            );
            
            INSERT INTO graph_edges_new (from_name, to_name, relation_type)
            SELECT from_name, to_name, relation_type FROM graph_edges;
            
            DROP TABLE graph_edges;
            ALTER TABLE graph_edges_new RENAME TO graph_edges;

            -- Temporal schema adjustments for semantic_metadata
            CREATE TABLE IF NOT EXISTS semantic_metadata_new (
                node_id TEXT,
                raw_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                timestamp TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0,
                valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                valid_until TEXT,
                superseded_by TEXT,
                PRIMARY KEY (node_id, valid_from)
            );
            
            INSERT INTO semantic_metadata_new (node_id, raw_text, embedding, timestamp, importance)
            SELECT node_id, raw_text, embedding, timestamp, importance FROM semantic_metadata;
            
            DROP TABLE semantic_metadata;
            ALTER TABLE semantic_metadata_new RENAME TO semantic_metadata;

            -- Re-create semantic_metadata FTS triggers dropped when dropping semantic_metadata
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_ai AFTER INSERT ON semantic_metadata BEGIN
                INSERT INTO semantic_fts(node_id, raw_text) VALUES (new.node_id, new.raw_text);
            END;
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_ad AFTER DELETE ON semantic_metadata BEGIN
                DELETE FROM semantic_fts WHERE node_id = old.node_id;
            END;
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_au AFTER UPDATE OF raw_text ON semantic_metadata BEGIN
                UPDATE semantic_fts SET raw_text = new.raw_text WHERE node_id = new.node_id;
            END;
        ",
    },
    Migration {
        version: "004_scoping",
        sql: "
            -- Access log and scoping column setup

            -- memory_access_log
            CREATE TABLE IF NOT EXISTS memory_access_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT NOT NULL,
                layer TEXT NOT NULL,
                accessed_at TEXT NOT NULL,
                accessed_by TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_access_log_mem_id ON memory_access_log (memory_id);
            CREATE INDEX IF NOT EXISTS idx_memory_access_log_layer ON memory_access_log (layer);

            -- Scoping for graph_nodes
            CREATE TABLE IF NOT EXISTS graph_nodes_new (
                name TEXT,
                entity_type TEXT NOT NULL,
                observations TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (name, user_id, session_id, agent_id)
            );
            INSERT INTO graph_nodes_new (name, entity_type, observations)
            SELECT name, entity_type, observations FROM graph_nodes;
            DROP TABLE graph_nodes;
            ALTER TABLE graph_nodes_new RENAME TO graph_nodes;
            CREATE INDEX IF NOT EXISTS idx_graph_nodes_scope ON graph_nodes (user_id, session_id, agent_id);

            -- Scoping for graph_edges
            CREATE TABLE IF NOT EXISTS graph_edges_new (
                from_name TEXT NOT NULL,
                to_name TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                valid_until TEXT,
                superseded_by TEXT,
                confidence REAL NOT NULL DEFAULT 1.0,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (from_name, to_name, relation_type, user_id, session_id, agent_id, valid_from)
            );
            INSERT INTO graph_edges_new (from_name, to_name, relation_type, valid_from, valid_until, superseded_by, confidence)
            SELECT from_name, to_name, relation_type, valid_from, valid_until, superseded_by, confidence FROM graph_edges;
            DROP TABLE graph_edges;
            ALTER TABLE graph_edges_new RENAME TO graph_edges;
            CREATE INDEX IF NOT EXISTS idx_graph_edges_scope ON graph_edges (user_id, session_id, agent_id);

            -- Scoping for semantic_metadata
            ALTER TABLE semantic_metadata ADD COLUMN user_id TEXT NOT NULL DEFAULT '*';
            ALTER TABLE semantic_metadata ADD COLUMN session_id TEXT NOT NULL DEFAULT '*';
            ALTER TABLE semantic_metadata ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*';
            CREATE INDEX IF NOT EXISTS idx_semantic_metadata_scope ON semantic_metadata (user_id, session_id, agent_id);

            -- Scoping for episodic_logs
            ALTER TABLE episodic_logs ADD COLUMN user_id TEXT NOT NULL DEFAULT '*';
            ALTER TABLE episodic_logs ADD COLUMN session_id TEXT NOT NULL DEFAULT '*';
            ALTER TABLE episodic_logs ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*';
            CREATE INDEX IF NOT EXISTS idx_episodic_logs_scope ON episodic_logs (user_id, session_id, agent_id);

            -- Scoping for reflection_memory
            ALTER TABLE reflection_memory ADD COLUMN user_id TEXT NOT NULL DEFAULT '*';
            ALTER TABLE reflection_memory ADD COLUMN session_id TEXT NOT NULL DEFAULT '*';
            ALTER TABLE reflection_memory ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*';
            CREATE INDEX IF NOT EXISTS idx_reflection_memory_scope ON reflection_memory (user_id, session_id, agent_id);

            -- Scoping for tool_performance
            CREATE TABLE IF NOT EXISTS tool_performance_new (
                tool_name TEXT NOT NULL,
                model_name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                average_latency REAL NOT NULL DEFAULT 0.0,
                last_used TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (tool_name, model_name, task_type, user_id, session_id, agent_id)
            );
            INSERT INTO tool_performance_new (tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used)
            SELECT tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used FROM tool_performance;
            DROP TABLE tool_performance;
            ALTER TABLE tool_performance_new RENAME TO tool_performance;
            CREATE INDEX IF NOT EXISTS idx_tool_performance_scope ON tool_performance (user_id, session_id, agent_id);

            -- Scoping for code_elements
            ALTER TABLE code_elements ADD COLUMN user_id TEXT NOT NULL DEFAULT '*';
            ALTER TABLE code_elements ADD COLUMN session_id TEXT NOT NULL DEFAULT '*';
            ALTER TABLE code_elements ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*';
            CREATE INDEX IF NOT EXISTS idx_code_elements_scope ON code_elements (user_id, session_id, agent_id);

            -- Scoping for shared_agent_memory
            CREATE TABLE IF NOT EXISTS shared_agent_memory_new (
                memory_key TEXT,
                memory_value TEXT NOT NULL,
                source_agent TEXT NOT NULL,
                target_agents TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0,
                timestamp TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (memory_key, user_id, session_id, agent_id)
            );
            INSERT INTO shared_agent_memory_new (memory_key, memory_value, source_agent, target_agents, importance, timestamp)
            SELECT memory_key, memory_value, source_agent, target_agents, importance, timestamp FROM shared_agent_memory;
            DROP TABLE shared_agent_memory;
            ALTER TABLE shared_agent_memory_new RENAME TO shared_agent_memory;
            CREATE INDEX IF NOT EXISTS idx_shared_agent_memory_scope ON shared_agent_memory (user_id, session_id, agent_id);
        ",
    },
];

pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Create schema_migrations table if not exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
        [],
    )?;

    // Get applied migrations
    let mut stmt = conn.prepare("SELECT version FROM schema_migrations;")?;
    let mut applied_versions = std::collections::HashSet::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let v: String = row.get(0)?;
        applied_versions.insert(v);
    }

    for migration in MIGRATIONS {
        if !applied_versions.contains(migration.version) {
            log::info!("Applying database migration {}...", migration.version);
            
            // Run migration in an unchecked transaction to allow shared reference execution
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(migration.sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));",
                [migration.version],
            )?;
            tx.commit()?;
            log::info!("Successfully applied database migration {}.", migration.version);
        }
    }

    Ok(())
}
