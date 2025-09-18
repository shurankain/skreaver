//! PostgreSQL transactional operations
//!
//! This module provides transactional wrapper functionality for PostgreSQL operations
//! with proper resource management and atomic commits.

use skreaver_core::error::MemoryError;
use skreaver_core::memory::{MemoryUpdate, MemoryWriter};

/// Transactional wrapper for PostgreSQL operations with proper resource management
pub struct PostgresTransactionalMemory<'a> {
    tx: Option<tokio_postgres::Transaction<'a>>,
    namespace: Option<String>,
    pending_operations: Vec<MemoryUpdate>,
}

impl<'a> PostgresTransactionalMemory<'a> {
    pub fn new(tx: tokio_postgres::Transaction<'a>, namespace: Option<String>) -> Self {
        Self {
            tx: Some(tx),
            namespace,
            pending_operations: Vec::new(),
        }
    }

    pub async fn commit(&mut self) -> Result<(), tokio_postgres::Error> {
        if let Some(tx) = self.tx.take() {
            // Apply all pending operations within the transaction
            for update in &self.pending_operations {
                let namespaced_key = match &self.namespace {
                    Some(ns) => format!("{}:{}", ns, update.key.as_str()),
                    None => update.key.as_str().to_string(),
                };

                let json_value: serde_json::Value = serde_json::from_str(&update.value)
                    .unwrap_or_else(|_| serde_json::Value::String(update.value.clone()));

                tx.execute(
                    r#"
                    INSERT INTO memory_entries (key, value, namespace, updated_at)
                    VALUES ($1, $2, $3, NOW())
                    ON CONFLICT (key) DO UPDATE SET
                        value = EXCLUDED.value,
                        updated_at = NOW()
                    "#,
                    &[
                        &namespaced_key,
                        &json_value,
                        &self.namespace.as_deref().unwrap_or(""),
                    ],
                )
                .await?;
            }

            tx.commit().await
        } else {
            Ok(())
        }
    }

    pub async fn rollback(&mut self) -> Result<(), tokio_postgres::Error> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await
        } else {
            Ok(())
        }
    }
}

impl<'a> MemoryWriter for PostgresTransactionalMemory<'a> {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        // Buffer operations to apply them atomically during commit
        self.pending_operations.push(update);
        Ok(())
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        // Buffer all operations to apply them atomically during commit
        self.pending_operations.extend(updates);
        Ok(())
    }
}